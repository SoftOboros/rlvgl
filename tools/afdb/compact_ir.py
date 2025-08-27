from __future__ import annotations

from typing import Dict, List, Optional

import orjson
import zstandard as zstd


def _intern(strings: List[str], index: Dict[str, int], value: Optional[str]) -> Optional[int]:
    if value is None:
        return None
    if value not in index:
        index[value] = len(strings)
        strings.append(value)
    return index[value]


def build_ir(catalog: Dict) -> Dict:
    strings: List[str] = []
    index: Dict[str, int] = {}

    chip = {
        "name": _intern(strings, index, catalog.get("part")),
        "family": _intern(strings, index, catalog.get("family")),
        "package": _intern(strings, index, catalog.get("package")),
        "pins": [],
    }

    for pin_name, funcs in sorted(catalog.get("pins", {}).items()):
        pin_entry = {"name": _intern(strings, index, pin_name), "functions": []}
        for func in funcs:
            func_entry = {
                "instance": _intern(strings, index, func.get("instance")),
                "signal": _intern(strings, index, func.get("signal")),
            }
            if "IOModes" in func:
                func_entry["IOModes"] = _intern(strings, index, func.get("IOModes"))
            pin_entry["functions"].append(func_entry)
        chip["pins"].append(pin_entry)

    return {"strings": strings, "chips": [chip]}


def encode_ir(catalog: Dict) -> bytes:
    ir = build_ir(catalog)
    return orjson.dumps(ir)


def compress_ir(ir_bytes: bytes, level: int = 19) -> bytes:
    compressor = zstd.ZstdCompressor(level=level)
    return compressor.compress(ir_bytes)


def encode_and_compress(catalog: Dict, level: int = 19) -> bytes:
    return compress_ir(encode_ir(catalog), level=level)
