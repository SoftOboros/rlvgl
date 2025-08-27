"""Build canonical pin contexts from CubeMX `.ioc` files."""

from __future__ import annotations

import re
from pathlib import Path
from typing import Any, Dict, Optional

try:
    from .pin_lut import (
        HAL_PULL,
        HAL_SPEED,
        MODE_TO_MODER,
        OTYPE_TO_BIT,
        PULL_TO_BITS,
        SPEED_TO_BITS,
    )
except ImportError:  # fallback for script invocation
    from pin_lut import (
        HAL_PULL,
        HAL_SPEED,
        MODE_TO_MODER,
        OTYPE_TO_BIT,
        PULL_TO_BITS,
        SPEED_TO_BITS,
    )


_PIN_RE = re.compile(
    r"^(?:Mcu\.)?(?:Pin\.)?([A-Z0-9]+)\.(Signal|Mode|GPIO_PuPd|GPIO_Speed|GPIO_OType|GPIO_Label)=(.+)$"
)


def build_pin_context(ioc_path: Path, mcu_pins: Optional[Dict[str, Dict[str, int]]] = None) -> Dict[str, Dict[str, Any]]:
    """Parse `ioc_path` into canonical per-pin objects.

    Parameters
    ----------
    ioc_path: Path
        Input CubeMX `.ioc` file.
    mcu_pins: Optional mapping of pin -> signal -> AF number.

    Returns
    -------
    dict
        Mapping of pin names to canonical context dictionaries.
    """

    raw: Dict[str, Dict[str, Any]] = {}
    with ioc_path.open() as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            m = _PIN_RE.match(line)
            if not m:
                continue
            pin, key, value = m.groups()
            raw.setdefault(pin, {})[key] = value

    pins: Dict[str, Dict[str, Any]] = {}
    for pin, fields in raw.items():
        if len(pin) < 3 or not pin.startswith("P") or not pin[2:].isdigit():
            # Skip entries that do not follow the expected ``P<port><index>`` pattern.
            continue

        sig_full = fields.get("Signal")
        mode = fields.get("Mode")
        pull = fields.get("GPIO_PuPd")
        speed = fields.get("GPIO_Speed")
        otype = fields.get("GPIO_OType")
        label = fields.get("GPIO_Label")

        af = None
        if sig_full and mcu_pins:
            af = mcu_pins.get(pin, {}).get(sig_full)

        port = pin[1]
        index = int(pin[2:])
        port_index = ord(port) - ord("A")

        class_ = "Raw"
        instance = None
        signal = None
        is_exti = False
        exti_line = None
        exti_port_index = None
        exti_rising = False
        exti_falling = False

        if sig_full:
            if sig_full.startswith("GPIO_"):
                class_ = "GPIO"
                if sig_full.startswith("GPIO_EXTI"):
                    is_exti = True
                    try:
                        exti_line = int(sig_full.split("GPIO_EXTI")[1])
                    except ValueError:
                        exti_line = None
                    if exti_line is not None:
                        exti_port_index = ord(port) - ord("A")
            elif sig_full.startswith("RCC_") or sig_full.startswith("DEBUG_") or sig_full.startswith("SYS_"):
                class_ = "System"
            else:
                class_ = "Peripheral"
            if "_" in sig_full:
                instance, signal = sig_full.split("_", 1)
            else:
                instance = sig_full
                signal = None

        if is_exti and mode:
            if "RISING" in mode:
                exti_rising = True
            if "FALLING" in mode:
                exti_falling = True

        pins[pin] = {
            "name": pin,
            "port": port,
            "index": index,
            "port_index": port_index,
            "class": class_,
            "sig_full": sig_full,
            "instance": instance,
            "signal": signal,
            "af": af,
            "mode": mode,
            "pull": pull,
            "speed": speed,
            "otype": otype,
            "label": label,
            "is_exti": is_exti,
            "exti_line": exti_line,
            "exti_port_index": exti_port_index,
            "exti_rising": exti_rising,
            "exti_falling": exti_falling,
            "moder_bits": MODE_TO_MODER.get(mode),
            "pupd_bits": PULL_TO_BITS.get(pull),
            "speed_bits": SPEED_TO_BITS.get(speed),
            "otype_bit": OTYPE_TO_BIT.get(otype),
            "hal_speed": HAL_SPEED.get(speed),
            "hal_pull": HAL_PULL.get(pull),
        }

    if not pins:
        raise ValueError(f"{ioc_path.name} evaluated to a null context")
    return pins
