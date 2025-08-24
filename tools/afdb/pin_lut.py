"""Lookup tables normalizing CubeMX GPIO strings.

Provides mappings from Cube-generated mode/pull/speed/otype
strings to bitfield values for register configuration as well as
HAL enum names.
"""

MODE_TO_MODER = {
    "GPIO_INPUT": 0b00,
    "GPIO_MODE_INPUT": 0b00,
    "GPIO_OUTPUT": 0b01,
    "GPIO_MODE_OUTPUT_PP": 0b01,
    "GPIO_MODE_OUTPUT_OD": 0b01,
    "GPIO_ANALOG": 0b11,
    "GPIO_ANALOG_ADC_CONTROL": 0b11,
    # Alternate
    "GPIO_AF_PP": 0b10,
    "GPIO_AF_OD": 0b10,
    "GPIO_MODE_AF_PP": 0b10,
    "GPIO_MODE_AF_OD": 0b10,
}

PULL_TO_BITS = {
    "GPIO_NOPULL": 0b00,
    "GPIO_PULLUP": 0b01,
    "GPIO_PULLDOWN": 0b10,
}

SPEED_TO_BITS = {
    "GPIO_SPEED_FREQ_LOW": 0b00,
    "GPIO_SPEED_FREQ_MEDIUM": 0b01,
    "GPIO_SPEED_FREQ_HIGH": 0b10,
    "GPIO_SPEED_FREQ_VERY_HIGH": 0b11,
}

OTYPE_TO_BIT = {
    "GPIO_OType_PP": 0,
    "GPIO_OType_OD": 1,
}

HAL_SPEED = {
    "GPIO_SPEED_FREQ_LOW": "Low",
    "GPIO_SPEED_FREQ_MEDIUM": "Medium",
    "GPIO_SPEED_FREQ_HIGH": "High",
    "GPIO_SPEED_FREQ_VERY_HIGH": "VeryHigh",
}

HAL_PULL = {
    "GPIO_NOPULL": "None",
    "GPIO_PULLUP": "PullUp",
    "GPIO_PULLDOWN": "PullDown",
}
