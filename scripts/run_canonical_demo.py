#!/usr/bin/env python3
from __future__ import annotations

import argparse
import shutil
import subprocess
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
DEMO = ROOT / "demos/model_validation/src/main.sifr"
HARNESS = ROOT / "demos/model_validation"
SNAPSHOT = ROOT / "tests/snapshots/model_validation.stdout"


def run(command: list[str], cwd: Path) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(command, cwd=cwd, capture_output=True, text=True)
    if result.returncode != 0:
        raise SystemExit(
            f"canonical demo command failed: {command!r}\n"
            f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}"
        )
    return result


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--sifr-bin", type=Path, required=True)
    args = parser.parse_args()

    with tempfile.TemporaryDirectory(prefix="canonical-demo-", dir=ROOT / "demos") as raw:
        project = Path(raw)
        shutil.copy2(HARNESS / "Cargo.toml", project / "Cargo.toml")
        shutil.copy2(HARNESS / "Cargo.lock", project / "Cargo.lock")
        shutil.copy2(HARNESS / "sifr.toml", project / "sifr.toml")
        (project / "src").mkdir()
        shutil.copy2(HARNESS / "src/lib.rs", project / "src/lib.rs")
        shutil.copy2(DEMO, project / "src/main.sifr")

        run([str(args.sifr_bin), "fetch", "--locked"], project)
        run([str(args.sifr_bin), "fmt", "--check", "src"], project)
        result = run([str(args.sifr_bin), "run", "--locked"], project)

    expected = SNAPSHOT.read_text(encoding="utf-8")
    if result.stdout != expected:
        raise SystemExit(
            "canonical demo output does not match snapshot\n"
            f"expected: {expected!r}\nactual:   {result.stdout!r}"
        )
    print("canonical demo snapshot passed")


if __name__ == "__main__":
    main()
