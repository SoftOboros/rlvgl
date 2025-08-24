"""Tests for STM32 AFDB utilities."""

from pathlib import Path
import json
import subprocess

import sys
ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))

from tools.afdb.pin_lut import (
    HAL_PULL,
    HAL_SPEED,
    MODE_TO_MODER,
    OTYPE_TO_BIT,
    PULL_TO_BITS,
    SPEED_TO_BITS,
)


def run_scraper(root: Path, out: Path) -> None:
    script = Path(__file__).resolve().parents[1] / "afdb" / "stm32_xml_scraper.py"
    subprocess.run(["python3", str(script), "--root", str(root), "--output", str(out)], check=True)


def test_scraper_skips_undefined_mcus(tmp_path):
    src_root = tmp_path / "src"
    ip_dir = src_root / "ip"
    mcu_dir = src_root / "mcu"
    ip_dir.mkdir(parents=True)
    mcu_dir.mkdir(parents=True)

    (ip_dir / "usart.xml").write_text("<IP Name='USART'><Signal Name='TX'/></IP>")
    (mcu_dir / "stm32f4.xml").write_text(
        "<Mcu Name='STM32F4'><Pin Name='PA0'><Signal Name='USART2_TX' Instance='USART2' AlternateFunction='7'/></Pin></Mcu>"
    )
    (mcu_dir / "stub.xml").write_text("<Mcu Name='STUB'></Mcu>")

    out = tmp_path / "out"
    run_scraper(src_root, out)

    assert (out / "mcu/stm32f4.json").exists()
    assert not (out / "mcu/STUB.json").exists()


def test_scraper_skip_list(tmp_path):
    src_root = tmp_path / "src"
    ip_dir = src_root / "ip"
    mcu_dir = src_root / "mcu"
    ip_dir.mkdir(parents=True)
    mcu_dir.mkdir(parents=True)

    (ip_dir / "usart.xml").write_text("<IP Name='USART'><Signal Name='TX'/></IP>")
    (mcu_dir / "stm32g0.xml").write_text("<Mcu Name='STM32G0'><Pin Name='PA0'/></Mcu>")

    out = tmp_path / "out"
    script = Path(__file__).resolve().parents[1] / "afdb" / "stm32_xml_scraper.py"
    subprocess.run(
        [
            "python3",
            str(script),
            "--root",
            str(src_root),
            "--output",
            str(out),
            "--skip-mcu",
            "STM32G0",
        ],
        check=True,
    )
    assert not (out / "mcu/STM32G0.json").exists()


def test_scraper_skip_file(tmp_path):
    src_root = tmp_path / "src"
    ip_dir = src_root / "ip"
    mcu_dir = src_root / "mcu"
    ip_dir.mkdir(parents=True)
    mcu_dir.mkdir(parents=True)

    (ip_dir / "usart.xml").write_text("<IP Name='USART'><Signal Name='TX'/></IP>")
    (mcu_dir / "stm32l0.xml").write_text("<Mcu Name='STM32L0'><Pin Name='PA0'/></Mcu>")

    skip = tmp_path / "skip.txt"
    skip.write_text("STM32L0\n")

    out = tmp_path / "out"
    script = Path(__file__).resolve().parents[1] / "afdb" / "stm32_xml_scraper.py"
    subprocess.run(
        [
            "python3",
            str(script),
            "--root",
            str(src_root),
            "--output",
            str(out),
            "--skip-list",
            str(skip),
        ],
        check=True,
    )
    assert not (out / "mcu/STM32L0.json").exists()


def test_ioc_board_populates_pins(tmp_path):
    mcu_root = tmp_path / "mcu"
    mcu_root.mkdir()
    mcu_root.joinpath("STM32F4.json").write_text(
        json.dumps(
            {
                "pins": {
                    "PA0": {
                        "name": "PA0",
                        "sigs": {"USART2_TX": {"signal": "USART2_TX", "af": 7}},
                        "position": 0,
                    },
                    "PB12": {"name": "PB12", "sigs": {}, "position": 0},
                },
                "ip": {},
                "data": {},
            },
            indent=2,
            sort_keys=True,
        )
    )
    ioc = tmp_path / "board.ioc"
    ioc.write_text(
        "\n".join(
            [
                "Mcu.Name=STM32F4",
                "PA0.Signal=USART2_TX",
                "PA0.Mode=GPIO_AF_PP",
                "PA0.GPIO_PuPd=GPIO_PULLUP",
                "PA0.GPIO_Speed=GPIO_SPEED_FREQ_VERY_HIGH",
                "PA0.GPIO_OType=GPIO_OType_PP",
                "PB12.Signal=GPIO_EXTI12",
                "PB12.Mode=GPIO_MODE_IT_RISING_FALLING",
            ]
        )
    )
    out = tmp_path / "board.json"
    script = Path(__file__).resolve().parents[1] / "afdb" / "st_ioc_board.py"
    subprocess.run(
        [
            "python3",
            str(script),
            "--ioc",
            str(ioc),
            "--mcu-root",
            str(mcu_root),
            "--board",
            "TestBoard",
            "--output",
            str(out),
        ],
        check=True,
    )
    data = json.loads(out.read_text())
    pa0 = data["pins"]["PA0"]
    assert pa0["af"] == 7
    assert pa0["instance"] == "USART2"
    assert pa0["class"] == "Peripheral"
    assert pa0["moder_bits"] == MODE_TO_MODER["GPIO_AF_PP"]
    assert pa0["pupd_bits"] == PULL_TO_BITS["GPIO_PULLUP"]
    assert pa0["speed_bits"] == SPEED_TO_BITS["GPIO_SPEED_FREQ_VERY_HIGH"]
    assert pa0["otype_bit"] == OTYPE_TO_BIT["GPIO_OType_PP"]
    assert pa0["hal_speed"] == HAL_SPEED["GPIO_SPEED_FREQ_VERY_HIGH"]
    assert pa0["hal_pull"] == HAL_PULL["GPIO_PULLUP"]
    pb12 = data["pins"]["PB12"]
    assert pb12["class"] == "GPIO"
    assert pb12["is_exti"] is True
    assert pb12["exti_line"] == 12
    assert pb12["exti_port_index"] == 1
    assert pb12["port_index"] == 1
    assert pb12["exti_rising"] is True
    assert pb12["exti_falling"] is True


def test_extract_af_converts_ioc(tmp_path):
    mcu_root = tmp_path / "mcu"
    mcu_root.mkdir()
    mcu_root.joinpath("STM32F4.json").write_text(
        json.dumps(
            {
                "pins": {
                    "PA0": {
                        "name": "PA0",
                        "sigs": {"USART2_TX": {"signal": "USART2_TX", "af": 7}},
                        "position": 0,
                    }
                },
                "ip": {},
                "data": {},
            },
            indent=2,
            sort_keys=True,
        )
    )
    ioc = tmp_path / "board.ioc"
    ioc.write_text("Mcu.Name=STM32F4\nPA0.Signal=USART2_TX\n")
    out = tmp_path / "out.json"
    script = Path(__file__).resolve().parents[1] / "afdb" / "st_extract_af.py"
    subprocess.run(
        [
            "python3",
            str(script),
            "--input",
            str(ioc),
            "--output",
            str(out),
            "--mcu-root",
            str(mcu_root),
        ],
        check=True,
    )
    data = json.loads(out.read_text())
    assert data["PA0"]["USART2_TX"] == 7


def test_ioc_board_skips_missing_mcu(tmp_path):
    ioc = tmp_path / "board.ioc"
    ioc.write_text("Mcu.Name=STM32ZZ\nPA0.Signal=USART_TX\n")
    out = tmp_path / "board.json"
    script = Path(__file__).resolve().parents[1] / "afdb" / "st_ioc_board.py"
    subprocess.run(
        [
            "python3",
            str(script),
            "--ioc",
            str(ioc),
            "--mcu-root",
            str(tmp_path / "mcu"),
            "--board",
            "Test",
            "--output",
            str(out),
        ],
        check=True,
    )
    assert not out.exists()


def test_ioc_board_renders_templates(tmp_path):
    mcu_root = tmp_path / "mcu"
    mcu_root.mkdir()
    mcu_root.joinpath("STM32F4.json").write_text(
        json.dumps(
            {
                "pins": {
                    "PA0": {
                        "name": "PA0",
                        "sigs": {"USART2_TX": {"signal": "USART2_TX", "af": 7}},
                        "position": 0,
                    }
                },
                "ip": {},
                "data": {},
            },
            indent=2,
            sort_keys=True,
        )
    )
    ioc = tmp_path / "board.ioc"
    ioc.write_text(
        "\n".join(
            [
                "Mcu.Name=STM32F4",
                "PA0.Signal=USART2_TX",
                "PA0.Mode=GPIO_AF_PP",
                "PA0.GPIO_PuPd=GPIO_NOPULL",
                "PA0.GPIO_Speed=GPIO_SPEED_FREQ_HIGH",
            ]
        )
    )
    board_out = tmp_path / "board.json"
    hal_out = tmp_path / "hal.rs"
    pac_out = tmp_path / "pac.rs"
    script = Path(__file__).resolve().parents[1] / "afdb" / "st_ioc_board.py"
    subprocess.run(
        [
            "python3",
            str(script),
            "--ioc",
            str(ioc),
            "--mcu-root",
            str(mcu_root),
            "--board",
            "Demo",
            "--output",
            str(board_out),
            "--hal-out",
            str(hal_out),
            "--pac-out",
            str(pac_out),
        ],
        check=True,
    )
    assert "into_alternate" in hal_out.read_text()
    assert "dp.GPIO" in pac_out.read_text()


def test_lookup_tables_basic():
    assert MODE_TO_MODER["GPIO_AF_PP"] == 0b10
    assert PULL_TO_BITS["GPIO_PULLUP"] == 0b01
    assert SPEED_TO_BITS["GPIO_SPEED_FREQ_VERY_HIGH"] == 0b11
    assert OTYPE_TO_BIT["GPIO_OType_OD"] == 1
    assert HAL_SPEED["GPIO_SPEED_FREQ_LOW"] == "Low"
    assert HAL_PULL["GPIO_PULLDOWN"] == "PullDown"
