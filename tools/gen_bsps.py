#!/usr/bin/env python3
"""Generate STM32 board support modules from CubeMX .ioc files."""

import argparse
import subprocess
import tempfile
from pathlib import Path
from jinja2 import Environment, FileSystemLoader


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
    for ioc in args.input.glob("*.ioc"):
        subprocess.run(
            [
                "rlvgl-creator",
                "bsp",
                "from-ioc",
                str(ioc),
                "--emit-hal",
                "--emit-pac",
                "--grouped-writes",
                "--per-peripheral",
                "--with-deinit",
                "--use-label-names",
                "--emit-label-consts",
                "--label-prefix",
                "pin_",
                "--out",
                str(args.output),
            ],
            check=True,
        )

    tmpl_path = Path(__file__).resolve().parents[1] / "src/bin/creator/bsp/templates"
    env = Environment(loader=FileSystemLoader(tmpl_path))
    tmpl = env.get_template("lib.rs.jinja")
    modules = sorted(
        p.stem for p in args.output.glob("*.rs") if p.name not in {"lib.rs", "mod.rs"}
    )
    args.output.joinpath("lib.rs").write_text(tmpl.render(modules=modules))


if __name__ == "__main__":
    main()
