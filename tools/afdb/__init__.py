# AFDB package initialization
from .compact_ir import build_ir, encode_ir, compress_ir, encode_and_compress

__all__ = ["build_ir", "encode_ir", "compress_ir", "encode_and_compress"]
