"""XML utility helpers for AFDB."""
from defusedxml import ElementTree as ET
from typing import Dict


def parse_xml_safe(path: str):
    with open(path, "rb") as f:
        tree = ET.parse(f)
    return tree.getroot()


def qname(elem) -> Dict[str, str]:
    if elem.tag.startswith("{"):
        ns, _, local = elem.tag[1:].partition("}")
        return {"ns": ns, "tag": local}
    return {"ns": "", "tag": elem.tag}
