"""Lossless XML ingestion utilities."""
from .util_xml import parse_xml_safe, qname

def element_to_raw(e) -> dict:
    attrs_order = list(e.attrib.keys())
    node = {
        "qname": qname(e),
        "attrs": dict(e.attrib),
        "attrs_order": attrs_order,
        "text": (e.text or ""),
        "tail": (e.tail or ""),
        "children": [element_to_raw(c) for c in list(e)],
    }
    if hasattr(e, "sourceline") and e.sourceline is not None:
        node["line"] = e.sourceline
    return node

def load_raw_tree(path: str) -> dict:
    root = parse_xml_safe(path)
    return element_to_raw(root)
