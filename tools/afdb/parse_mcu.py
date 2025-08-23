"""Parse STM32 MCU XML into canonical JSON overlay."""
from .ingest_raw import load_raw_tree
from defusedxml import ElementTree as ET
from .util_xml import parse_xml_safe


def parse_mcu(path: str) -> dict:
    root = parse_xml_safe(path)
    out = {"raw_xml_path": path, "raw_tree": None, "mcu": {"meta": {}, "instances": [], "pins": []}}
    out["raw_tree"] = None

    mcu_attrs = dict(root.attrib)
    meta = {
        "Family": mcu_attrs.get("Family"),
        "Line": mcu_attrs.get("Line"),
        "Package": mcu_attrs.get("Package"),
        "RefName": mcu_attrs.get("RefName"),
        "ClockTree": mcu_attrs.get("ClockTree"),
        "DBVersion": mcu_attrs.get("DBVersion"),
        "HasPowerPad": mcu_attrs.get("HasPowerPad") == "true",
        "other_attributes": {k: v for k, v in mcu_attrs.items() if k not in {"Family", "Line", "Package", "RefName", "ClockTree", "DBVersion", "HasPowerPad"}},
    }

    flash_values = []
    for child in list(root):
        tag = child.tag.split("}")[-1]
        if tag == "Core":
            meta["Core"] = (child.text or "").strip()
        elif tag == "Frequency":
            meta["Frequency_MHz"] = float((child.text or "0").strip())
        elif tag == "Ram":
            meta["Ram_KB"] = int((child.text or "0").strip())
        elif tag == "IONb":
            meta["IONb"] = int((child.text or "0").strip())
        elif tag == "Die":
            meta["Die"] = (child.text or "").strip()
        elif tag == "Flash":
            flash_values.append(int((child.text or "0").strip()))
        elif tag == "Voltage":
            meta["Voltage"] = {"min": float(child.attrib.get("Min", "0")), "max": float(child.attrib.get("Max", "0"))}
        elif tag == "Temperature":
            meta["Temperature_C"] = {"min": float(child.attrib.get("Min", "0")), "max": float(child.attrib.get("Max", "0"))}
    if flash_values:
        meta["Flash_KB"] = flash_values

    out["mcu"]["meta"] = meta

    for child in root.findall(".//{*}IP"):
        attrs = dict(child.attrib)
        entry = {
            "instance": attrs.pop("InstanceName", None),
            "type": attrs.pop("Name", None),
            "version": attrs.pop("Version", None),
            "configFile": attrs.pop("ConfigFile", None),
            "clockEnableMode": attrs.pop("ClockEnableMode", None),
            "other_attributes": attrs,
        }
        out["mcu"]["instances"].append(entry)

    for p in root.findall(".//{*}Pin"):
        pattrs = dict(p.attrib)
        pin = {
            "name": pattrs.pop("Name", None),
            "position": pattrs.pop("Position", None),
            "type": pattrs.pop("Type", None),
            "variant": pattrs.pop("Variant", None),
            "signals": [],
            "other_attributes": pattrs,
        }
        for s in p.findall(".//{*}Signal"):
            sattrs = dict(s.attrib)
            pin["signals"].append({"name": sattrs.pop("Name", None), **({"IOModes": sattrs.pop("IOModes")} if "IOModes" in sattrs else {}), "other_attributes": sattrs})
        out["mcu"]["pins"].append(pin)

    return out
