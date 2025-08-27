from pathlib import Path
from tools.afdb.parse_mcu import parse_mcu
from tools.afdb.parse_ip import parse_ip
from tools.afdb.build_catalog import build_catalog, save_catalog


def test_build_catalog():
    mcu = parse_mcu("tests/fixtures/sample_mcu_STM32C011_UFQFPN20.xml")
    ip = parse_ip("tests/fixtures/sample_ip_TIM_Modes.xml")
    cat = build_catalog(mcu, ip)
    pa0 = cat["pins"]["PA0"]
    assert {"instance": "USART1", "signal": "TX"} in pa0
    assert {"instance": "GPIO", "IOModes": "Input,Output,Analog,EXTI"} in pa0
    assert cat["instances"]["ADC1"]["type"] == "ADC"


def test_save_catalog(tmp_path):
    mcu = parse_mcu("tests/fixtures/sample_mcu_STM32C011_UFQFPN20.xml")
    ip = parse_ip("tests/fixtures/sample_ip_TIM_Modes.xml")
    cat = build_catalog(mcu, ip)
    path = Path(save_catalog(cat, tmp_path))
    expect = tmp_path / cat["family"] / f"{cat['part']}.json"
    assert path == expect and path.exists()
