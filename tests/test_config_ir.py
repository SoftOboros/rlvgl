"""Schema validation for the unified Config IR (v0.1).

Validates example IR JSON files under `examples/ir/` against
`schemas/config.schema.json`.
"""

import json
from pathlib import Path

import jsonschema


def ir_examples() -> list[Path]:
    root = Path(__file__).parent.parent
    return sorted((root / "examples" / "ir").glob("*.json"))


def test_config_ir_examples_validate():
    schema = json.loads(Path("schemas/config.schema.json").read_text())
    examples = ir_examples()
    assert examples, "no IR examples found in examples/ir/"
    for ex in examples:
        data = json.loads(ex.read_text())
        jsonschema.validate(data, schema)


def test_config_ir_basic_fields_present():
    ex = Path("examples/ir/stm32h747i_disco_minimal.json")
    data = json.loads(ex.read_text())
    assert data["version"] == "0.1"
    assert data["board"]
    assert data["mcu"].startswith("STM32")
    assert any(p["function"].startswith("USART1_") for p in data["pins"])  # sanity

