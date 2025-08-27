import json
import subprocess
import zstandard as zstd
from pathlib import Path

from tools.pack_chipdb import build_blob

REPO_ROOT = Path(__file__).resolve().parents[2]

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
    blob = zstd.ZstdDecompressor().decompress(out.read_bytes())
    assert blob == expected
