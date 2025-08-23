"""Parse STM32 IP Modes XML into canonical dictionary."""
from typing import Dict, Set
from .util_xml import parse_xml_safe


def parse_ip(path: str) -> dict:
    root = parse_xml_safe(path)
    out = {"raw_xml_path": path, "ip": {}}
    for periph in root.findall(".//{*}Peripheral"):
        attrs = dict(periph.attrib)
        name = attrs.pop("Name", None)
        signals: Set[str] = set()
        for sig in periph.findall(".//{*}Signal"):
            sattrs = dict(sig.attrib)
            sname = sattrs.get("Name", "")
            norm = sname
            if name and norm.startswith(name):
                norm = norm[len(name):]
            norm = norm.lstrip("0123456789_ ")
            signals.add(norm)
        out["ip"][name] = {"signals": sorted(signals), "other": attrs}
    return out
