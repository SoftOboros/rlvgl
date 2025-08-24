#!/usr/bin/env python3
"""Convert a user CubeMX `.ioc` file into a board overlay JSON.

This helper loads canonical MCU pin data produced by `stm32_xml_scraper.py`
and resolves alternate-function numbers when translating the `.ioc` file.
 It writes a JSON structure compatible with `boards/` overlays:

 {
   "board": "<BOARD>",
   "chip": "<MCU>",
   "pins": { "PA0": {"name": "PA0", "sig_full": "USART2_TX", ...}, ... }
 }

Usage:
    st_ioc_board.py --ioc project.ioc --mcu-root afdb/mcu --board MyBoard --output MyBoard.json
"""

import argparse
import json
import sys
from pathlib import Path

from st_extract_af import _detect_mcu
from pin_context import build_pin_context


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
    parser.add_argument("--hal-out", help="Optional path to write HAL init code")
    parser.add_argument("--pac-out", help="Optional path to write PAC init code")
    parser.add_argument(
        "--strict",
        action="store_true",
        help="Error out when MCU JSON is missing instead of skipping",
    )
    args = parser.parse_args()

    ioc_path = Path(args.ioc)
    mcu_root = Path(args.mcu_root)
    out_path = Path(args.output)

    mcu_name = _detect_mcu(ioc_path)
    if not mcu_name:
        raise SystemExit("Unable to determine MCU name from .ioc")
    mcu_path = mcu_root / f"{mcu_name}.json"
    if not mcu_path.exists():
        msg = f"Missing MCU JSON: {mcu_path}"
        if args.strict:
            raise SystemExit(msg)
        print(msg, file=sys.stderr)
        return

    with mcu_path.open() as f:
        mcu_data = json.load(f)
    mcu_pins = {}
    for pin, info in mcu_data.get("pins", {}).items():
        sigs = {name: sig.get("af", 0) for name, sig in info.get("sigs", {}).items()}
        mcu_pins[pin] = sigs

    pins = build_pin_context(ioc_path, mcu_pins)
    board = {"board": args.board, "chip": mcu_name, "pins": pins}
    with out_path.open("w") as f:
        json.dump(board, f, indent=2, sort_keys=True)
    print(f"Wrote board overlay → {out_path}")

    if args.hal_out or args.pac_out:
        from templates import render_hal, render_pac
        if args.hal_out:
            Path(args.hal_out).write_text(render_hal(pins))
        if args.pac_out:
            Path(args.pac_out).write_text(render_pac(pins))


if __name__ == "__main__":
    main()
