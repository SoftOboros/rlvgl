import orjson

from tools.afdb.parse_mcu import parse_mcu
from tools.afdb.parse_ip import parse_ip
from tools.afdb.build_catalog import build_catalog
from tools.afdb.compact_ir import build_ir, encode_and_compress


def _load_catalog():
    mcu = parse_mcu("tests/fixtures/sample_mcu_STM32C011_UFQFPN20.xml")
    ip = parse_ip("tests/fixtures/sample_ip_TIM_Modes.xml")
    return build_catalog(mcu, ip)


def test_build_ir_dedup_strings():
    cat = _load_catalog()
    ir = build_ir(cat)
    strings = ir["strings"]
    pa0_id = strings.index("PA0")
    assert strings.count("PA0") == 1
    assert ir["chips"][0]["pins"][0]["name"] == pa0_id


def test_encode_and_compress_smaller_than_json():
    cat = _load_catalog()
    json_size = len(orjson.dumps(cat))
    blob = encode_and_compress(cat)
    assert len(blob) < json_size
