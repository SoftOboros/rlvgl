import json
import shutil
import subprocess
from pathlib import Path

from tools.pack_chipdb import build_blob

try:
    import zstandard as zstd
except ModuleNotFoundError:
    zstd = None

REPO_ROOT = Path(__file__).resolve().parents[2]


def decompress_blob(blob: bytes) -> bytes:
    if zstd is not None:
        return zstd.ZstdDecompressor().decompress(blob)

    zstd_bin = shutil.which("zstd")
    if zstd_bin is None:
        raise RuntimeError("test_pack_chipdb.py requires zstandard or the zstd CLI")

    proc = subprocess.run(
        [zstd_bin, "-d", "-c"],
        input=blob,
        stdout=subprocess.PIPE,
        check=True,
    )
    return proc.stdout


def test_pack_chipdb_roundtrip(tmp_path):
    src = tmp_path / "src"
    src.mkdir()
    (src / "mcu.json").write_text(json.dumps({"chip": "STM32F407"}))
    out = tmp_path / "db.bin.zst"
    subprocess.run(
        ["python3", str(REPO_ROOT / "tools/pack_chipdb.py"), "--input", str(src), "--output", str(out)],
        check=True,
        cwd=REPO_ROOT,
    )
    expected = build_blob(src)
    blob = decompress_blob(out.read_bytes())
    assert blob == expected
