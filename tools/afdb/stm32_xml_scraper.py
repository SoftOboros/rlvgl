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
        # Some IP definitions namespace the signal tags and use <PinSignal> or <Signal>.
        signals = [
            elem.attrib["Name"]
            for elem in root.findall(".//*")
            if elem.tag.endswith("Signal")
        ]
        db[name] = {"signals": signals}
    return db


def _parse_mcu(mcu_dir: Path) -> dict:
    mcus = {}
    for xml_path in mcu_dir.glob("*.xml"):
        root = ET.parse(xml_path).getroot()
        mcu_name = root.attrib.get("RefName") or xml_path.stem
        data, pins, ip = {}, {}, {}
        for item in root.findall(".//*"):
            item_tag = item.tag.split('}')[-1]
            if item_tag == "Pin":
                pin_name = item.attrib.get("Name")
                sigs = {}
                for sig in item.findall(".//*"):
                    if not sig.tag.endswith("Signal"):
                        continue
                    entry = {}
                    instance = sig.attrib.get("Instance")
                    if instance:
                        entry["instance"] = instance
                    modes = sig.attrib.get("IOModes")
                    if modes:
                        entry["modes"] = modes
                    entry["signal"] = sig.attrib.get("Name")
                    af = sig.attrib.get("AlternateFunction")
                    if af:
                        try:
                            entry["af"] = int(af)
                        except ValueError:
                            pass
                    sigs[entry["signal"]] = entry
                if sigs:
                    pos = item.attrib.get("Position")
                    if pos:
                        try:
                            pos = int(pos)
                        except ValueError:
                            pass
                    pins[pin_name] = {"name": pin_name, "sigs": sigs, "position": pos }
            if item_tag == "IP":
                cf = item.attrib.get("ConfigFile")
                i = item.attrib.get("InstanceName")
                n = item.attrib.get("Name")
                v = item.attrib.get("Version")
                ip[n] = {"name": n, "config": cf, "instance": i, "version": v}
            elif item.text and not item.text.isspace():
                data[item_tag] = item.text
        if pins:
            mcus[mcu_name] = {"pins": pins, "ip": ip, "data": data}
        else:
            print(f"Skipping MCU {mcu_name}: no pin definitions")
    return mcus


def main() -> None:
    parser = argparse.ArgumentParser(description="Scrape STM32 XML pin data")
    parser.add_argument("--root", required=True, help="Root of STM32_open_pin_data repository")
    parser.add_argument("--output", required=True, help="Output directory for JSON IR")
    args = parser.parse_args()

    root = Path(args.root)
    out = Path(args.output)
    out.mkdir(parents=True, exist_ok=True)

    ip_dir = root / "IP"
    if not ip_dir.exists():
        ip_dir = root / "mcu" / "IP"
    ip_db = _parse_ip(ip_dir)
    (out / "ip.json").write_text(json.dumps(ip_db, indent=2, sort_keys=True))

    mcu_dir = root / "mcu"
    if not mcu_dir.exists():
        mcu_dir = root
    mcu_out = out / "mcu"
    mcu_out.mkdir(exist_ok=True)
    for name, data in _parse_mcu(mcu_dir).items():
        (mcu_out / f"{name}.json").write_text(json.dumps(data, indent=2, sort_keys=True))

    print(f"Wrote {len(ip_db)} IPs and MCU databases to {out}")


if __name__ == "__main__":
    main()
