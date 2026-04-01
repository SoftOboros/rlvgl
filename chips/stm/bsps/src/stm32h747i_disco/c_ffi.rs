//! C FFI declarations for the C HAL bindings.
//! Only compiled when the `c_hal` feature is active.

#![cfg(feature = "c_hal")]

unsafe extern "C" {
    // I2C4
    pub fn c_i2c4_enable_gpio_clocks();
    pub fn c_i2c4_configure_pins();
    pub fn c_i2c4_enable_peripherals();
    pub fn c_i2c4_deinit();

    // SPI2
    pub fn c_spi2_enable_gpio_clocks();
    pub fn c_spi2_configure_pins();
    pub fn c_spi2_enable_peripherals();
    pub fn c_spi2_deinit();

    // SPI5
    pub fn c_spi5_enable_gpio_clocks();
    pub fn c_spi5_configure_pins();
    pub fn c_spi5_enable_peripherals();
    pub fn c_spi5_deinit();

    // UART8
    pub fn c_uart8_enable_gpio_clocks();
    pub fn c_uart8_configure_pins();
    pub fn c_uart8_enable_peripherals();
    pub fn c_uart8_deinit();

    // USART1
    pub fn c_usart1_enable_gpio_clocks();
    pub fn c_usart1_configure_pins();
    pub fn c_usart1_enable_peripherals();
    pub fn c_usart1_deinit();
}
