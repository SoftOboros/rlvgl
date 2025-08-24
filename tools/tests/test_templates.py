"""Tests for HAL and PAC template rendering."""

from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))

from tools.afdb.templates import render_hal, render_pac
from tools.afdb.pin_lut import (
    HAL_PULL,
    HAL_SPEED,
    MODE_TO_MODER,
    OTYPE_TO_BIT,
    PULL_TO_BITS,
    SPEED_TO_BITS,
)


def sample_pin():
    return {
        "name": "PA9",
        "port": "A",
        "index": 9,
        "class": "Peripheral",
        "instance": "USART1",
        "signal": "TX",
        "af": 7,
        "mode": "GPIO_AF_PP",
        "pull": "GPIO_PULLUP",
        "speed": "GPIO_SPEED_FREQ_VERY_HIGH",
        "otype": "GPIO_OType_PP",
        "label": None,
        "is_exti": False,
        "exti_line": None,
        "exti_port_index": None,
        "exti_rising": False,
        "exti_falling": False,
        "moder_bits": MODE_TO_MODER["GPIO_AF_PP"],
        "pupd_bits": PULL_TO_BITS["GPIO_PULLUP"],
        "speed_bits": SPEED_TO_BITS["GPIO_SPEED_FREQ_VERY_HIGH"],
        "otype_bit": OTYPE_TO_BIT["GPIO_OType_PP"],
        "hal_speed": HAL_SPEED["GPIO_SPEED_FREQ_VERY_HIGH"],
        "hal_pull": HAL_PULL["GPIO_PULLUP"],
    }


def i2c_pin():
    p = sample_pin().copy()
    p.update(
        {
            "name": "PB8",
            "port": "B",
            "index": 8,
            "instance": "I2C1",
            "signal": "SCL",
            "af": 4,
            "pull": "GPIO_PULLUP",
            "otype": "GPIO_OType_OD",
            "moder_bits": MODE_TO_MODER["GPIO_AF_PP"],
            "pupd_bits": PULL_TO_BITS["GPIO_PULLUP"],
            "speed_bits": SPEED_TO_BITS["GPIO_SPEED_FREQ_VERY_HIGH"],
            "otype_bit": OTYPE_TO_BIT["GPIO_OType_OD"],
            "hal_pull": HAL_PULL["GPIO_PULLUP"],
        }
    )
    return p


def gpio_pin():
    return {
        "name": "PC0",
        "port": "C",
        "index": 0,
        "class": "GPIO",
        "instance": None,
        "signal": None,
        "af": None,
        "mode": "GPIO_OUTPUT",
        "pull": "GPIO_NOPULL",
        "speed": "GPIO_SPEED_FREQ_LOW",
        "otype": "GPIO_OType_PP",
        "label": None,
        "is_exti": False,
        "exti_line": None,
        "exti_port_index": None,
        "exti_rising": False,
        "exti_falling": False,
        "moder_bits": MODE_TO_MODER["GPIO_OUTPUT"],
        "pupd_bits": PULL_TO_BITS["GPIO_NOPULL"],
        "speed_bits": SPEED_TO_BITS["GPIO_SPEED_FREQ_LOW"],
        "otype_bit": OTYPE_TO_BIT["GPIO_OType_PP"],
        "hal_speed": HAL_SPEED["GPIO_SPEED_FREQ_LOW"],
        "hal_pull": HAL_PULL["GPIO_NOPULL"],
    }


def test_render_hal_basic():
    pins = {"PA9": sample_pin(), "PC0": gpio_pin(), "PB8": i2c_pin()}
    code = render_hal(pins)
    assert "gpioa.pa9.into_alternate::<7>()" in code
    assert "gpiob.pb8.into_alternate_open_drain::<4>()" in code
    assert "gpioc.pc0.into_push_pull_output()" in code
    assert "internal_pull_up(true)" in code
    assert "set_open_drain(true)" in code


def test_render_hal_missing_af():
    p = sample_pin()
    p["af"] = None
    code = render_hal({"PA9": p})
    assert "into_alternate::<" not in code
    assert "into_alternate();" in code


def test_render_pac_basic():
    pins = {"PA9": sample_pin(), "PB12": {
        "name": "PB12",
        "port": "B",
        "index": 12,
        "class": "GPIO",
        "instance": None,
        "signal": None,
        "af": None,
        "mode": "GPIO_MODE_IT_RISING_FALLING",
        "pull": None,
        "speed": None,
        "otype": None,
        "label": None,
        "is_exti": True,
        "exti_line": 12,
        "exti_port_index": 1,
        "exti_rising": True,
        "exti_falling": True,
        "moder_bits": None,
        "pupd_bits": None,
        "speed_bits": None,
        "otype_bit": None,
        "hal_speed": None,
        "hal_pull": None,
    }}
    code = render_pac(pins)
    assert "dp.RCC.ahb2enr.modify(|_, w| w.gpioaen().set_bit());" in code
    assert "GPIOA.moder.modify" in code
    assert "GPIOA.afrh.modify" in code
    assert "SYSCFG.exticr[3]" in code
    assert "EXTI.rtsr1" in code and "EXTI.ftsr1" in code
