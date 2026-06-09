//! CTRLMOD pad configuration for AM335x LCD and I2C2 pins.
//!
//! Each pad register at CTRLMOD + 0x800..0xA34 controls one pin's mux mode,
//! pull-up/down, input enable, and slew rate.

use super::am335x::*;

/// Configure all 24 LCD data pins + 4 control signals for Mode 0 (LCD function).
///
/// LCD_DATA[0:23] are output-only (no rxactive), fast slew, no pull.
/// LCD_VSYNC, LCD_HSYNC, LCD_PCLK, LCD_AC_BIAS_EN are also Mode 0 output.
pub unsafe fn configure_lcd_pins() {
    let lcd_out: u32 = PIN_MODE_0 | (1 << 3);

    let lcd_data_pins: [u32; 24] = [
        CONF_LCD_DATA0,
        CONF_LCD_DATA1,
        CONF_LCD_DATA2,
        CONF_LCD_DATA3,
        CONF_LCD_DATA4,
        CONF_LCD_DATA5,
        CONF_LCD_DATA6,
        CONF_LCD_DATA7,
        CONF_LCD_DATA8,
        CONF_LCD_DATA9,
        CONF_LCD_DATA10,
        CONF_LCD_DATA11,
        CONF_LCD_DATA12,
        CONF_LCD_DATA13,
        CONF_LCD_DATA14,
        CONF_LCD_DATA15,
        CONF_LCD_DATA16,
        CONF_LCD_DATA17,
        CONF_LCD_DATA18,
        CONF_LCD_DATA19,
        CONF_LCD_DATA20,
        CONF_LCD_DATA21,
        CONF_LCD_DATA22,
        CONF_LCD_DATA23,
    ];

    unsafe {
        for pin in lcd_data_pins {
            reg_write(pin, lcd_out);
        }
        reg_write(CONF_LCD_VSYNC, lcd_out);
        reg_write(CONF_LCD_HSYNC, lcd_out);
        reg_write(CONF_LCD_PCLK, lcd_out);
        reg_write(CONF_LCD_AC_BIAS_EN, lcd_out);
    }
}

/// Configure I2C2 SDA/SCL pins (P9.19/P9.20) for Mode 3, pull-up, rx active.
pub unsafe fn configure_i2c2_pins() {
    let i2c_cfg: u32 = PIN_MODE_3 | PIN_PULL_UP_EN | PIN_RX_ACTIVE | PIN_SLOW_SLEW;
    unsafe {
        reg_write(CONF_UART1_CTSN, i2c_cfg);
        reg_write(CONF_UART1_RTSN, i2c_cfg);
    }
}
