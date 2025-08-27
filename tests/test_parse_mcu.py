"""Tests for MCU overlay parsing and schema validation."""

import json
from pathlib import Path

import jsonschema
from tools.afdb.parse_mcu import parse_mcu


def test_parse_mcu():
    data = parse_mcu("tests/fixtures/sample_mcu_STM32C011_UFQFPN20.xml")
    meta = data["mcu"]["meta"]
    assert meta["Family"] == "STM32C0"
    assert meta["Flash_KB"] == [16, 32]
    assert len(data["mcu"]["instances"]) == 2
    pa0 = next(p for p in data["mcu"]["pins"] if p["name"] == "PA0")
    assert pa0["position"] == "5"
    assert any(s["name"] == "GPIO" and s["IOModes"] == "Input,Output,Analog,EXTI" for s in pa0["signals"])
    schema = json.loads(Path("schemas/mcu_canonical.schema.json").read_text())
    jsonschema.validate(data, schema)
