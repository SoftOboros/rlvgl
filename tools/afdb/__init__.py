# AFDB package initialization

try:  # pragma: no cover - optional dependency
    from .compact_ir import build_ir, encode_ir, compress_ir, encode_and_compress
    __all__ = ["build_ir", "encode_ir", "compress_ir", "encode_and_compress"]
except Exception:  # handle missing orjson during lightweight imports
    __all__: list[str] = []
