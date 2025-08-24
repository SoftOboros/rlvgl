"""Tests for STM32 AFDB utilities."""

from pathlib import Path
import json
import subprocess


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
    assert data["pins"]["PA0"]["USART2_TX"] == 7


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
