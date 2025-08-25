#!/usr/bin/env python3
"""Generate STM32 board support modules from CubeMX .ioc files."""

import argparse
import subprocess
import tempfile
from pathlib import Path


def main() -> None:
    """Entry point."""
    parser = argparse.ArgumentParser(
        description="Generate STM32 BSP modules from .ioc files"
    )
    parser.add_argument(
        "--input", type=Path, required=True, help="directory containing .ioc files"
    )
    parser.add_argument(
        "--output", type=Path, required=True, help="destination for generated modules"
    )
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    af_db = Path(__file__).resolve().parent.parent / "stm32_af.json"
    for ioc in args.input.glob("*.ioc"):
        module = ioc.stem.replace("-", "_")
        tmp_json = Path(tempfile.mkstemp(suffix=".json")[1])
        subprocess.run(
            [
                "rlvgl-creator",
                "board",
                "from-ioc",
                str(ioc),
                module,
                str(tmp_json),
                "--af",
                str(af_db),
                "--bsp-out",
                str(args.output),
            ],
            check=True,
        )
        tmp_json.unlink(missing_ok=True)


if __name__ == "__main__":
    main()
