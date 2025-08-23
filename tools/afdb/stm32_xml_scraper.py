#!/usr/bin/env python3
"""Scrape STM32_open_pin_data XML into canonical JSON IR.

Parses the `ip/` and `mcu/` directories from the STM32_open_pin_data
submodule and emits JSON files describing peripheral signals and MCU pin
alternate function mappings. The output directory will contain:

  ip.json          – map of peripheral type -> supported signals
  mcu/<part>.json  – per-part pin AF database
"""

import argparse
import json
from pathlib import Path
import xml.etree.ElementTree as ET


def _parse_ip(ip_dir: Path) -> dict:
    db = {}
    for xml_path in ip_dir.glob("*.xml"):
        root = ET.parse(xml_path).getroot()
        name = root.attrib.get("Name") or xml_path.stem
        signals = [sig.attrib["Name"] for sig in root.findall(".//Signal")]
        db[name] = {"signals": signals}
    return db


def _parse_mcu(mcu_dir: Path) -> dict:
    mcus = {}
    for xml_path in mcu_dir.glob("*.xml"):
        root = ET.parse(xml_path).getroot()
        name = root.attrib.get("Name") or root.attrib.get("PartNumber") or xml_path.stem
        pins = {}
        for pin in root.findall(".//Pin"):
            pin_name = pin.attrib.get("Name")
            entries = []
            for sig in pin.findall("Signal"):
                entry = {
                    "instance": sig.attrib.get("Instance"),
                    "signal": sig.attrib.get("Name"),
                }
                af = sig.attrib.get("AlternateFunction")
                if af:
                    try:
                        entry["af"] = int(af)
                    except ValueError:
                        pass
                entries.append(entry)
            if entries:
                pins[pin_name] = entries
        mcus[name] = {"pins": pins}
    return mcus


def main() -> None:
    parser = argparse.ArgumentParser(description="Scrape STM32 XML pin data")
    parser.add_argument("--root", required=True, help="Root of STM32_open_pin_data repository")
    parser.add_argument("--output", required=True, help="Output directory for JSON IR")
    args = parser.parse_args()

    root = Path(args.root)
    out = Path(args.output)
    out.mkdir(parents=True, exist_ok=True)

    ip_db = _parse_ip(root / "ip")
    (out / "ip.json").write_text(json.dumps(ip_db, indent=2, sort_keys=True))

    mcu_out = out / "mcu"
    mcu_out.mkdir(exist_ok=True)
    for name, data in _parse_mcu(root / "mcu").items():
        (mcu_out / f"{name}.json").write_text(json.dumps(data, indent=2, sort_keys=True))

    print(f"Wrote {len(ip_db)} IPs and MCU databases to {out}")


if __name__ == "__main__":
    main()
