"""Render Rust init code from canonical pin contexts."""
from __future__ import annotations

from typing import Any, Dict


def render_hal(pins: Dict[str, Dict[str, Any]]) -> str:
    """Render stm32xx-hal style initialization code.

    Parameters
    ----------
    pins: mapping of pin name (e.g. "PA9") to canonical context
    """
    ports = sorted({ctx["port"] for ctx in pins.values()})
    lines = ["let mut rcc = dp.RCC.constrain();"]
    for port in ports:
        lines.append(f"let mut gpio{port.lower()} = dp.GPIO{port}.split(&mut rcc.ahb2);")

    for name, ctx in sorted(pins.items()):
        var = name.lower()
        portl = ctx["port"].lower()
        cls = ctx.get("class")
        pull = ctx.get("pull")
        speed = ctx.get("hal_speed")
        otype = ctx.get("otype")
        mode = ctx.get("mode")

        if cls in ("Peripheral", "System"):
            af = ctx.get("af")
            if otype == "GPIO_OType_OD":
                if af is None:
                    lines.append(
                        f"let {var} = gpio{portl}.{var}.into_alternate_open_drain();"
                    )
                else:
                    lines.append(
                        f"let {var} = gpio{portl}.{var}.into_alternate_open_drain::<{af}>();"
                    )
                lines.append(f"{var}.set_open_drain(true);")
            else:
                if af is None:
                    lines.append(
                        f"let {var} = gpio{portl}.{var}.into_alternate();"
                    )
                else:
                    lines.append(
                        f"let {var} = gpio{portl}.{var}.into_alternate::<{af}>();"
                    )
        elif cls == "GPIO":
            if mode in ("GPIO_ANALOG", "GPIO_ANALOG_ADC_CONTROL"):
                lines.append(f"let {var} = gpio{portl}.{var}.into_analog();")
            elif mode in ("GPIO_OUTPUT", "GPIO_MODE_OUTPUT_PP", "GPIO_MODE_OUTPUT_OD"):
                if otype == "GPIO_OType_OD":
                    lines.append(
                        f"let {var} = gpio{portl}.{var}.into_open_drain_output();"
                    )
                    lines.append(f"{var}.set_open_drain(true);")
                else:
                    lines.append(
                        f"let {var} = gpio{portl}.{var}.into_push_pull_output();"
                    )
            else:
                if pull == "GPIO_PULLUP":
                    lines.append(f"let {var} = gpio{portl}.{var}.into_pull_up_input();")
                elif pull == "GPIO_PULLDOWN":
                    lines.append(f"let {var} = gpio{portl}.{var}.into_pull_down_input();")
                else:
                    lines.append(f"let {var} = gpio{portl}.{var}.into_floating_input();")
        else:
            continue

        if speed:
            lines.append(f"{var}.set_speed(Speed::{speed});")
        if pull == "GPIO_PULLUP":
            lines.append(f"{var}.internal_pull_up(true);")
        elif pull == "GPIO_PULLDOWN":
            lines.append(f"{var}.internal_pull_down(true);")
    return "\n".join(lines) + "\n"


def render_pac(pins: Dict[str, Dict[str, Any]]) -> str:
    """Render register-level (PAC) initialization code."""
    ports = sorted({ctx["port"] for ctx in pins.values()})
    out = []
    for port in ports:
        out.append(
            f"dp.RCC.ahb2enr.modify(|_, w| w.gpio{port.lower()}en().set_bit());"
        )

    for ctx in pins.values():
        port = ctx["port"]
        idx = ctx["index"]
        prefix = f"dp.GPIO{port}"
        # MODER
        moder = ctx.get("moder_bits") or 0
        out.append(
            f"{prefix}.moder.modify(|r, w| unsafe {{\n    w.bits((r.bits() & !(0b11 << {idx*2})) | ({moder} << {idx*2}))\n}});"
        )
        # OTYPER
        otype = ctx.get("otype_bit") or 0
        out.append(
            f"{prefix}.otyper.modify(|r, w| unsafe {{\n    w.bits((r.bits() & !(1 << {idx})) | ({otype} << {idx}))\n}});"
        )
        # OSPEEDR
        speed = ctx.get("speed_bits") or 0
        out.append(
            f"{prefix}.ospeedr.modify(|r, w| unsafe {{\n    w.bits((r.bits() & !(0b11 << {idx*2})) | ({speed} << {idx*2}))\n}});"
        )
        # PUPDR
        pull = ctx.get("pupd_bits") or 0
        out.append(
            f"{prefix}.pupdr.modify(|r, w| unsafe {{\n    w.bits((r.bits() & !(0b11 << {idx*2})) | ({pull} << {idx*2}))\n}});"
        )
        # AFR
        af = ctx.get("af")
        if af is not None:
            afr = "afrl" if idx < 8 else "afrh"
            shift = (idx % 8) * 4
            out.append(
                f"{prefix}.{afr}.modify(|r, w| unsafe {{\n    w.bits((r.bits() & !(0xF << {shift})) | (({af} & 0xF) << {shift}))\n}});"
            )
        if ctx.get("is_exti"):
            line = ctx["exti_line"]
            port_idx = ctx["exti_port_index"]
            out.append(
                f"dp.SYSCFG.exticr[{line // 4}].modify(|r, w| unsafe {{\n    let mut bits = r.bits() & !(0xF << {(line % 4) * 4});\n    bits |= ({port_idx}u32 & 0xF) << {(line % 4) * 4};\n    w.bits(bits)\n}});"
            )
            rising = (
                f"bits |= 1 << {line};" if ctx.get("exti_rising") else f"bits &= !(1 << {line});"
            )
            out.append(
                f"dp.EXTI.rtsr1.modify(|r, w| unsafe {{\n    let mut bits = r.bits();\n    {rising}\n    w.bits(bits)\n}});"
            )
            falling = (
                f"bits |= 1 << {line};" if ctx.get("exti_falling") else f"bits &= !(1 << {line});"
            )
            out.append(
                f"dp.EXTI.ftsr1.modify(|r, w| unsafe {{\n    let mut bits = r.bits();\n    {falling}\n    w.bits(bits)\n}});"
            )
            out.append(
                f"dp.EXTI.imr1.modify(|r, w| unsafe {{ w.bits(r.bits() | (1 << {line})) }});"
            )
    return "\n".join(out) + "\n"
