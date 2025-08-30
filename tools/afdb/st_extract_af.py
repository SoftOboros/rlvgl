#!/usr/bin/env python3
"""Convert STM32 pin descriptions to a compact JSON database.

This utility accepts either a single `.csv` or `.ioc` file or a directory
containing such files. Directories are searched recursively for supported file
types. The resulting JSON maps pin names to the
signals they can provide and, where available, the alternate-function number.

Usage examples:
  st_extract_af.py --input pins.csv --output af.json
  st_extract_af.py --input boards/ --output build/afjson

Both input types produce a uniform JSON structure:
{
  "<PIN>": { "<SIGNAL>": <AF>, ... },
  ...
}
"""

import argparse
import csv
import json
import re
from pathlib import Path
from typing import Dict, Optional


def _parse_csv(path: Path) -> Dict[str, Dict[str, int]]:
    db: Dict[str, Dict[str, int]] = {}
    with path.open(newline="") as f:
        reader = csv.DictReader(f)
        for row in reader:
            pin = row["pin"].strip()
            signal = row["signal"].strip()
            af = int(row["af"].strip())
            db.setdefault(pin, {})[signal] = af
    return db

_IOC_RE = re.compile(r'^(?:Mcu\.)?(?:Pin\.)?(.+?)\.Signal=(.+)$')
#_IOC_RE = re.compile(r"Pin\.([A-Z0-9]+)\.Signal=(.+)")
_MCU_NAME_RE = re.compile(r"Mcu\.Name=(.+)")

def _detect_mcu(path: Path) -> Optional[str]:
    with path.open() as f:
        for line in f:
            match = _MCU_NAME_RE.match(line.strip())
            if match:
                return match.group(1)
    return None


def _parse_ioc(path: Path, mcu_pins: Optional[Dict[str, Dict[str, int]]] = None) -> Dict[str, Dict[str, int]]:
    db: Dict[str, Dict[str, int]] = {}
    with path.open() as f:
        for line in f:
            match = _IOC_RE.match(line.strip())
            if match:
                pin, signal = match.groups()
                af = 0
                if mcu_pins:
                    af = mcu_pins.get(pin, {}).get(signal, 0)
                db.setdefault(pin, {})[signal] = af
    if not db:
        raise ValueError(f"{path.name} evaluated to a null db")
    return db


def _convert_file(in_path: Path, out_path: Path, mcu_root: Optional[Path]) -> None:
    if in_path.suffix.lower() == ".csv":
        db = _parse_csv(in_path)
    elif in_path.suffix.lower() == ".ioc":
        mcu_pins = None
        if mcu_root:
            mcu_name = _detect_mcu(in_path)
            if mcu_name:
                mcu_path = mcu_root / f"{mcu_name}.json"
                if mcu_path.exists():
                    with mcu_path.open() as f:
                        data = json.load(f)
                    print(f"Loaded: {mcu_path}")
                    mcu_pins = {}
                    for pin, info in data.get("pins", {}).items():
                        sigs = {}
                        # Support either {"sigs": {"NAME": {"af": N}}} or a list of entries
                        if isinstance(info, list):
                            for ent in info:
                                sig_name = ent.get("signal")
                                af = ent.get("af")
                                try:
                                    af_val = int(af) if af is not None else 0
                                except (TypeError, ValueError):
                                    af_val = 0
                                if sig_name:
                                    sigs[sig_name] = af_val
                        else:
                            for sig_name, sig in info.get("sigs", {}).items():
                                af = sig.get("af")
                                try:
                                    af_val = int(af) if af is not None else 0
                                except (TypeError, ValueError):
                                    af_val = 0
                                sigs[sig_name] = af_val
                        mcu_pins[pin] = sigs
        db = _parse_ioc(in_path, mcu_pins)
        #print(db)
    else:
        raise ValueError(f"Unsupported file extension: {in_path.suffix}")

    with out_path.open("w") as f:
        json.dump(db, f, indent=2, sort_keys=True)

    print(f"Wrote AF DB → {out_path}")


def main() -> None:
    parser = argparse.ArgumentParser(description="Build STM32 AF JSON database from CSV or IOC")
    parser.add_argument("--input", required=True, help="Input file or directory of pin descriptions")
    parser.add_argument("--output", required=True, help="Output file or directory for JSON")
    parser.add_argument(
        "--mcu-root",
        help="Directory containing canonical MCU JSON files produced by stm32_xml_scraper.py",
    )
    args = parser.parse_args()

    in_path = Path(args.input)
    out_path = Path(args.output)
    mcu_root = Path(args.mcu_root) if args.mcu_root else None

    if in_path.is_dir():
        out_path.mkdir(parents=True, exist_ok=True)
        for src in in_path.rglob("*"):
            if src.is_file() and src.suffix.lower() in {".csv", ".ioc"}:
                dst = out_path / f"{src.stem}.json"
                _convert_file(src, dst, mcu_root)
    else:
        if out_path.is_dir():
            dst = out_path / f"{in_path.stem}.json"
        else:
            dst = out_path
        _convert_file(in_path, dst, mcu_root)


if __name__ == "__main__":
    main()
