"""Pytest collection plugin for deterministic upstream node evidence."""

from __future__ import annotations

import dataclasses
import datetime as dt
import decimal
import enum
import fractions
import functools
import hashlib
import inspect
import json
import math
import os
import pathlib
import re
import sys
import types
import typing
import uuid
import zoneinfo
from collections.abc import ItemsView, KeysView, Mapping, Sequence, Set, ValuesView
from pathlib import Path
from typing import Any


def _digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _canonical(value: object) -> bytes:
    return json.dumps(
        value, sort_keys=True, separators=(",", ":"), ensure_ascii=True
    ).encode()


def _json_issue(value: object, path: str = "$") -> str:
    if value is None or isinstance(value, (bool, int, float, str)):
        return "none"
    if isinstance(value, list):
        for index, item in enumerate(value):
            issue = _json_issue(item, f"{path}[{index}]")
            if issue != "none":
                return issue
        return "none"
    if isinstance(value, dict):
        for key, item in value.items():
            if not isinstance(key, str):
                return f"{path} has non-string key {type(key).__name__}"
            issue = _json_issue(item, f"{path}.{key}")
            if issue != "none":
                return issue
        return "none"
    return f"{path} contains {type(value).__name__}"


def _source_hash(value: object) -> str:
    module = _safe_attribute(value, "__module__")
    if not (
        module == "tests"
        or module.startswith("tests.")
        or module == "pydantic"
        or module.startswith("pydantic.")
        or module == "pydantic_core"
        or module.startswith("pydantic_core.")
    ):
        return "external-symbol"
    try:
        source = inspect.getsource(value)
    except Exception:
        return "none"
    normalized = inspect.cleandoc(source).replace("\r\n", "\n")
    return _digest(normalized.encode())


def _type_name(value: object) -> str:
    cls = value if isinstance(value, type) else type(value)
    return f"{getattr(cls, '__module__', '')}.{getattr(cls, '__qualname__', cls.__name__)}"


def _safe_attribute(value: object, name: str) -> str:
    try:
        result = getattr(value, name, "")
    except Exception:
        return ""
    return result if isinstance(result, str) else ""


def _fingerprint(value: object, seen: set[int] | None = None) -> object:
    if seen is None:
        seen = set()
    if isinstance(value, str):
        if re.fullmatch(
            r"[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[1-8][0-9a-fA-F]{3}-"
            r"[89abAB][0-9a-fA-F]{3}-[0-9a-fA-F]{12}",
            value,
        ):
            return ["uuid-string-source-value"]
        return ["str", value]
    if value is sys.path:
        return ["runtime-import-path"]
    if value is None or isinstance(value, (bool, int)):
        return [type(value).__name__, value]
    if value is Ellipsis:
        return ["ellipsis"]
    if value is NotImplemented:
        return ["not-implemented"]
    if type(value) is object:
        return ["object-sentinel"]
    if isinstance(value, float):
        if math.isnan(value):
            return ["float", "nan"]
        return ["float", value.hex()]
    if isinstance(value, bytes):
        return ["bytes", value.hex()]
    if isinstance(value, complex):
        return ["complex", value.real.hex(), value.imag.hex()]
    if isinstance(value, decimal.Decimal):
        item = value.as_tuple()
        return ["decimal", item.sign, list(item.digits), item.exponent]
    if isinstance(value, fractions.Fraction):
        return ["fraction", value.numerator, value.denominator]
    if isinstance(value, uuid.UUID):
        return ["uuid-source-value"]
    if isinstance(value, dt.datetime):
        return ["datetime-source-value", "aware" if value.tzinfo else "naive"]
    if isinstance(value, dt.date):
        return ["date-source-value"]
    if isinstance(value, dt.time):
        return ["time", value.isoformat(), value.fold]
    if isinstance(value, dt.timedelta):
        return ["timedelta", value.days, value.seconds, value.microseconds]
    if isinstance(value, zoneinfo.ZoneInfo):
        return ["zoneinfo", value.key]
    if isinstance(value, pathlib.PurePath):
        return ["path", value.as_posix()]
    if isinstance(value, (KeysView, ValuesView, ItemsView)):
        return [
            "mapping-view",
            _type_name(value),
            [_fingerprint(item, seen) for item in value],
        ]
    if isinstance(value, types.ModuleType):
        return ["module", value.__name__]
    if isinstance(value, types.GeneratorType):
        code = value.gi_code
        code_payload = [
            code.co_name,
            code.co_qualname,
            code.co_firstlineno,
            code.co_argcount,
            code.co_posonlyargcount,
            code.co_kwonlyargcount,
            list(code.co_names),
            list(code.co_varnames),
            list(code.co_freevars),
            list(code.co_cellvars),
        ]
        return ["generator-expression", _digest(_canonical(code_payload))]
    if isinstance(value, types.CodeType):
        code_payload = [
            value.co_name,
            value.co_qualname,
            value.co_firstlineno,
            value.co_argcount,
            value.co_posonlyargcount,
            value.co_kwonlyargcount,
            list(value.co_names),
            list(value.co_varnames),
            list(value.co_freevars),
            list(value.co_cellvars),
        ]
        return ["code", _digest(_canonical(code_payload))]
    if isinstance(value, enum.Enum):
        return ["enum", _type_name(value), value.name, _fingerprint(value.value, seen)]
    if isinstance(value, BaseException):
        return [
            "exception",
            _type_name(value),
            [_fingerprint(item, seen) for item in value.args],
        ]
    if isinstance(value, re.Pattern):
        return ["pattern", _fingerprint(value.pattern, seen), value.flags]
    origin = typing.get_origin(value)
    if origin is not None:
        return [
            "typing",
            _fingerprint(origin, seen),
            [_fingerprint(item, seen) for item in typing.get_args(value)],
        ]
    if typing.is_typeddict(value):
        return [
            "typed-dict-symbol",
            _safe_attribute(value, "__module__"),
            _safe_attribute(value, "__qualname__") or _safe_attribute(value, "__name__"),
            _source_hash(value),
        ]
    if type(value).__module__ in {"typing", "typing_extensions"}:
        return ["typing-symbol", _type_name(value), str(value)]
    if isinstance(value, (types.FunctionType, types.BuiltinFunctionType, type)) or inspect.isroutine(value):
        return [
            "symbol",
            _safe_attribute(value, "__module__"),
            _safe_attribute(value, "__qualname__") or _safe_attribute(value, "__name__"),
            getattr(getattr(value, "__objclass__", None), "__qualname__", ""),
            _source_hash(value),
        ]
    if type(value).__module__.startswith(
        ("_pytest.", "pytest_examples.", "dirty_equals.", "six")
    ):
        return [
            "pytest-infrastructure",
            _type_name(value),
            _source_hash(type(value)),
        ]

    identity = id(value)
    if identity in seen:
        raise TypeError(f"cyclic parameter value is not auditable: {_type_name(value)}")
    seen.add(identity)
    try:
        if dataclasses.is_dataclass(value) and not isinstance(value, type):
            fields = [
                [field.name, _fingerprint(getattr(value, field.name), seen)]
                for field in dataclasses.fields(value)
            ]
            return ["dataclass", _type_name(value), fields]
        if isinstance(value, Mapping):
            pairs = [
                [_fingerprint(key, seen), _fingerprint(item, seen)]
                for key, item in value.items()
            ]
            pairs.sort(key=_canonical)
            return ["mapping", _type_name(value), pairs]
        if isinstance(value, Set) and not isinstance(value, (str, bytes)):
            items = [_fingerprint(item, seen) for item in value]
            items.sort(key=_canonical)
            return ["set", _type_name(value), items]
        if isinstance(value, Sequence) and not isinstance(value, (str, bytes)):
            return [
                "sequence",
                _type_name(value),
                [_fingerprint(item, seen) for item in value],
            ]
        if hasattr(value, "_asdict"):
            return ["namedtuple", _type_name(value), _fingerprint(value._asdict(), seen)]
        state: dict[str, object] = {}
        try:
            raw_state = vars(value)
        except TypeError:
            raw_state = {}
        for name, item in sorted(raw_state.items()):
            if not name.startswith("__") and not callable(item):
                state[name] = _fingerprint(item, seen)
        for cls in type(value).__mro__:
            slots = getattr(cls, "__slots__", ())
            if isinstance(slots, str):
                slots = (slots,)
            for name in slots:
                if name in {"__dict__", "__weakref__"} or name in state:
                    continue
                if hasattr(value, name):
                    state[name] = _fingerprint(getattr(value, name), seen)
        source = _source_hash(type(value))
        if state or source != "none":
            return ["object", _type_name(value), source, state]
        if type(value).__module__.startswith("pydantic_core."):
            return ["pydantic-core-extension-singleton", _type_name(value)]
        if type(value).__module__.startswith("tests."):
            return ["source-closure-object", _type_name(value)]
        raise TypeError(
            f"opaque parameter value has no deterministic fingerprint: {_type_name(value)}"
        )
    finally:
        seen.remove(identity)


@functools.lru_cache(maxsize=None)
def _normalized_source_closure(path: Path, root: Path) -> str:
    paths = [path]
    current = path.parent
    while current == root or root in current.parents:
        conftest = current / "conftest.py"
        if conftest.is_file() and conftest != path:
            paths.append(conftest)
        if current == root:
            break
        current = current.parent
    normalized: list[list[str]] = []
    for candidate in sorted(set(paths)):
        data = candidate.read_bytes()
        if candidate.suffix == ".py":
            try:
                data = data.decode("utf-8").replace("\r\n", "\n").encode()
            except UnicodeDecodeError:
                pass
        normalized.append([candidate.relative_to(root).as_posix(), _digest(data)])
    return _digest(_canonical(normalized))


def pytest_collection_modifyitems(session: Any, config: Any, items: list[Any]) -> None:
    output = os.environ.get("PYDANTIC_SIFR_COLLECTION_OUT")
    namespace = os.environ.get("PYDANTIC_SIFR_COLLECTION_NAMESPACE")
    upstream = os.environ.get("PYDANTIC_SIFR_UPSTREAM_ROOT")
    if not output or namespace not in {"api", "core"} or not upstream:
        raise RuntimeError("pydantic-sifr collection environment is incomplete")
    root = Path(upstream).resolve()
    records: list[dict[str, object]] = []
    for item in items:
        path = Path(str(item.path)).resolve()
        relative = path.relative_to(root).as_posix()
        nodeid = item.nodeid
        raw_selector = nodeid.split("::", 1)[1] if "::" in nodeid else ""
        selector = raw_selector.split("[", 1)[0] if "[" in raw_selector else raw_selector
        callspec = getattr(item, "callspec", None)
        parameters: list[list[object]] = []
        if callspec is not None:
            for name in sorted(callspec.params):
                try:
                    value_fingerprint = _fingerprint(callspec.params[name])
                except Exception as error:
                    raise TypeError(
                        f"cannot fingerprint {nodeid} parameter {name}: {error}"
                    ) from error
                parameters.append([name, value_fingerprint])
        source_closure = _normalized_source_closure(path, root)
        try:
            parameter_values_bytes = _canonical(parameters)
        except TypeError as error:
            raise TypeError(
                f"non-JSON fingerprint for {nodeid}: {_json_issue(parameters)}"
            ) from error
        records.append(
            {
                "namespace": namespace,
                "path": relative,
                "selector": selector,
                "_parameters": parameters,
                "_parameterized": callspec is not None,
                "parameter_value_sha256": (
                    _digest(parameter_values_bytes) if callspec is not None else "none"
                ),
                "source_closure_sha256": source_closure,
            }
        )
    records.sort(
        key=lambda item: (
            str(item["path"]),
            str(item["selector"]),
            str(item["parameter_value_sha256"]),
        )
    )
    previous_key: tuple[str, str] | None = None
    ordinal = 0
    for record in records:
        key = (str(record["path"]), str(record["selector"]))
        if key != previous_key:
            ordinal = 0
            previous_key = key
        parameterized = bool(record.pop("_parameterized"))
        parameters = record.pop("_parameters")
        record["collected_ordinal"] = ordinal
        if parameterized:
            payload = {
                "collected_ordinal": ordinal,
                "parameters": parameters,
                "source_closure_sha256": record["source_closure_sha256"],
            }
            record["parameter_identity"] = _digest(_canonical(payload))
        else:
            record["parameter_identity"] = "none"
        ordinal += 1
    Path(output).write_bytes(_canonical(records) + b"\n")
