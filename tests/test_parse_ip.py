"""Tests for IP overlay parsing and schema validation."""

import json
from pathlib import Path

import jsonschema
from tools.afdb.parse_ip import parse_ip


def test_parse_ip():
    data = parse_ip("tests/fixtures/sample_ip_TIM_Modes.xml")
    assert set(data["ip"].keys()) == {"TIM", "USART"}
    assert data["ip"]["TIM"]["signals"] == ["BKIN", "CH1", "CH1N", "CH2", "CH2N", "ETR"]
    assert "TX" in data["ip"]["USART"]["signals"]
    schema = json.loads(Path("schemas/ip_canonical.schema.json").read_text())
    jsonschema.validate(data, schema)
