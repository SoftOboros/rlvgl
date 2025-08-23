import orjson
from tools.afdb.ingest_raw import load_raw_tree


def test_load_raw_tree():
    tree = load_raw_tree("tests/fixtures/sample_mcu_STM32C011_UFQFPN20.xml")
    assert tree["qname"]["tag"] == "Mcu"
    assert "attrs" in tree and tree["attrs"]["Family"] == "STM32C0"
    assert len(tree["children"]) > 0
    orjson.dumps(tree)
