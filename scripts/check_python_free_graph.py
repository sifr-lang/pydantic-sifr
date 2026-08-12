#!/usr/bin/env python3
"""Reject Python binding crates in the production dependency graph."""

from __future__ import annotations

import re
import subprocess

FORBIDDEN = re.compile(r"^(?:cpython|pyo3(?:-[a-z0-9_-]+)?|pythonize)(?:\s|$)")


def forbidden_packages(lines: str) -> list[str]:
    return [line for line in lines.splitlines() if FORBIDDEN.match(line)]


def self_test() -> None:
    seeded = "\n".join(
        [
            "pydantic_sifr_core v0.1.0-beta.1",
            "pyo3 v0.24.0",
            "pyo3-ffi v0.24.0",
            "pythonize v0.24.0",
            "serde v1.0.229",
        ]
    )
    found = forbidden_packages(seeded)
    if found != [
        "pyo3 v0.24.0",
        "pyo3-ffi v0.24.0",
        "pythonize v0.24.0",
    ]:
        raise SystemExit(f"Python-free graph self-test failed: {found}")


def main() -> int:
    self_test()
    graph = subprocess.run(
        [
            "cargo",
            "tree",
            "-p",
            "pydantic_sifr_core",
            "--edges",
            "normal",
            "--prefix",
            "none",
            "--format",
            "{p}",
        ],
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    found = forbidden_packages(graph)
    if found:
        raise SystemExit("production graph contains Python bindings:\n" + "\n".join(found))
    print("Python-free production graph passed; detector self-test passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
