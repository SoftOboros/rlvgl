"""Fuse MCU and IP overlays into a catalog and persist outputs."""
from typing import Dict, Optional
from pathlib import Path
import orjson


def build_catalog(mcu: dict, ip: Optional[dict] = None) -> dict:
    inst_map = {i.get("instance"): i.get("type") for i in mcu.get("mcu", {}).get("instances", []) if i.get("instance")}
    ip_dict = {k: set(v.get("signals", [])) for k, v in (ip or {}).get("ip", {}).items()}
    cat = {
        "part": mcu.get("mcu", {}).get("meta", {}).get("RefName"),
        "family": mcu.get("mcu", {}).get("meta", {}).get("Family"),
        "package": mcu.get("mcu", {}).get("meta", {}).get("Package"),
        "instances": {k: {"type": v} for k, v in inst_map.items()},
        "pins": {},
        "meta": mcu.get("mcu", {}).get("meta", {}),
        "extras": {"raw_xml_path": mcu.get("raw_xml_path")},
    }

    for pin in mcu.get("mcu", {}).get("pins", []):
        entries = []
        for sig in pin.get("signals", []):
            name = sig.get("name") or ""
            iomodes = sig.get("IOModes")
            inst = None
            sig_name = None
            if "_" in name:
                inst_candidate, sig_candidate = name.split("_", 1)
                base_type = inst_map.get(inst_candidate)
                if not base_type:
                    base_type = ''.join(filter(str.isalpha, inst_candidate))
                if base_type in ip_dict and sig_candidate in ip_dict[base_type]:
                    inst, sig_name = inst_candidate, sig_candidate
                else:
                    inst, sig_name = inst_candidate, sig_candidate
            else:
                inst = name
            entry = {}
            if inst:
                entry["instance"] = inst
            if sig_name:
                entry["signal"] = sig_name
            if iomodes:
                entry["IOModes"] = iomodes
            if not sig_name and name != inst:
                entry["name"] = name
            entries.append(entry)
        cat["pins"][pin.get("name")] = entries
    return cat


def save_catalog(cat: dict, root: str) -> str:
    """Write catalog JSON under root/<family>/<part>.json."""
    family = cat.get("family") or "unknown"
    part = cat.get("part") or "unknown"
    path = Path(root) / family / f"{part}.json"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(orjson.dumps(cat, option=orjson.OPT_INDENT_2))
    return str(path)
