from tools.afdb.parse_mcu import parse_mcu
from tools.afdb.build_catalog import build_catalog
from tools.afdb.report import catalog_to_markdown


def test_report_contains_pin_and_functions():
    mcu = parse_mcu("tests/fixtures/sample_mcu_STM32C011_UFQFPN20.xml")
    cat = build_catalog(mcu)
    md = catalog_to_markdown(cat)
    assert "| Pin | Functions |" in md
    assert "| PA0 |" in md
    assert "USART1_TX" in md
    assert "GPIO(IOModes=Input,Output,Analog,EXTI)" in md
