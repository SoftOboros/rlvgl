#!/usr/bin/env python3
"""Convert a user CubeMX `.ioc` file into a board overlay JSON.

This helper loads canonical MCU pin data produced by `stm32_xml_scraper.py`
and resolves alternate-function numbers when translating the `.ioc` file.
It writes a JSON structure compatible with `boards/` overlays:

{
  "board": "<BOARD>",
  "chip": "<MCU>",
  "pins": { "PA0": {"USART2_TX": 7}, ... }
}

Usage:
    st_ioc_board.py --ioc project.ioc --mcu-root afdb/mcu --board MyBoard --output MyBoard.json
"""

import argparse
import json
from pathlib import Path

from st_extract_af import _detect_mcu, _parse_ioc


def main() -> None:
    parser = argparse.ArgumentParser(description="Convert CubeMX .ioc to board overlay JSON")
    parser.add_argument("--ioc", required=True, help="Input CubeMX .ioc file")
    parser.add_argument(
        "--mcu-root",
        required=True,
        help="Directory containing canonical MCU JSON files",
    )
    parser.add_argument("--board", required=True, help="Board name for the output overlay")
    parser.add_argument("--output", required=True, help="Path to write the board JSON")
    args = parser.parse_args()

    ioc_path = Path(args.ioc)
    mcu_root = Path(args.mcu_root)
    out_path = Path(args.output)

    mcu_name = _detect_mcu(ioc_path)
    if not mcu_name:
        raise SystemExit("Unable to determine MCU name from .ioc")
    mcu_path = mcu_root / f"{mcu_name}.json"
    if not mcu_path.exists():
        raise SystemExit(f"Missing MCU JSON: {mcu_path}")

    with mcu_path.open() as f:
        mcu_data = json.load(f)
    mcu_pins = {}
    for pin, info in mcu_data.get("pins", {}).items():
        sigs = {name: sig.get("af", 0) for name, sig in info.get("sigs", {}).items()}
        mcu_pins[pin] = sigs

    pins = _parse_ioc(ioc_path, mcu_pins)
    board = {"board": args.board, "chip": mcu_name, "pins": pins}
    with out_path.open("w") as f:
        json.dump(board, f, indent=2, sort_keys=True)
    print(f"Wrote board overlay → {out_path}")


if __name__ == "__main__":
    main()
