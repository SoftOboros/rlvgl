"""Generate human-readable pin/function reports."""
from typing import Dict, List


def _format_entry(entry: Dict[str, str]) -> str:
    inst = entry.get("instance")
    sig = entry.get("signal")
    iomodes = entry.get("IOModes")
    name = entry.get("name")
    if inst and sig:
        base = f"{inst}_{sig}"
        if iomodes:
            return f"{base}(IOModes={iomodes})"
        return base
    if inst:
        base = inst
        if iomodes:
            return f"{base}(IOModes={iomodes})"
        return base
    if name:
        if iomodes:
            return f"{name}(IOModes={iomodes})"
        return name
    return ""


def catalog_to_markdown(cat: Dict) -> str:
    lines: List[str] = ["| Pin | Functions |", "|-----|-----------|"]
    for pin in sorted(cat.get("pins", {})):
        entries = cat["pins"].get(pin, [])
        funcs = ", ".join(_format_entry(e) for e in entries)
        lines.append(f"| {pin} | {funcs} |")
    return "\n".join(lines) + "\n"
