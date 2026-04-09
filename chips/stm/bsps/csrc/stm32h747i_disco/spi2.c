/* C HAL binding for SPI2 on STM32H747I-DISCO.
 * Pins: PA11 (NSS, AF0), PA12 (SCK, AF0), PC2 (MISO, AF0), PC3 (MOSI, AF0).
 * Push-pull, no pull, very high speed.
 * Equivalent to chips/stm/bsps/src/stm32h747i_disco/pac/spi2.rs.
 */

#include "stm32h747xi.h"

void c_spi2_enable_gpio_clocks(void) {
    /* Enable GPIOA (bit 0) and GPIOC (bit 2) clocks */
    REG32(RCC_AHB4ENR) |= (1u << 0) | (1u << 2);
}

void c_spi2_configure_pins(void) {
    uint32_t tmp;

    /* --- GPIOA: PA11, PA12 --- */

    /* PUPDR: no pull (0b00) */
    tmp = GPIOx(GPIOA_BASE, GPIO_PUPDR);
    tmp &= ~(0x3u << (11 * 2));
    tmp &= ~(0x3u << (12 * 2));
    GPIOx(GPIOA_BASE, GPIO_PUPDR) = tmp;

    /* OTYPER: push-pull (clear bits) */
    tmp = GPIOx(GPIOA_BASE, GPIO_OTYPER);
    tmp &= ~(1u << 11);
    tmp &= ~(1u << 12);
    GPIOx(GPIOA_BASE, GPIO_OTYPER) = tmp;

    /* OSPEEDR: very high speed (0b11) */
    tmp = GPIOx(GPIOA_BASE, GPIO_OSPEEDR);
    tmp &= ~(0x3u << (11 * 2));
    tmp |=  (0x3u << (11 * 2));
    tmp &= ~(0x3u << (12 * 2));
    tmp |=  (0x3u << (12 * 2));
    GPIOx(GPIOA_BASE, GPIO_OSPEEDR) = tmp;

    /* AFRL: no-op */
    tmp = GPIOx(GPIOA_BASE, GPIO_AFRL);
    GPIOx(GPIOA_BASE, GPIO_AFRL) = tmp;

    /* AFRH: AF0 on PA11, PA12 */
    tmp = GPIOx(GPIOA_BASE, GPIO_AFRH);
    tmp &= ~(0xFu << ((11 % 8) * 4));
    tmp |=  (0u   << ((11 % 8) * 4));
    tmp &= ~(0xFu << ((12 % 8) * 4));
    tmp |=  (0u   << ((12 % 8) * 4));
    GPIOx(GPIOA_BASE, GPIO_AFRH) = tmp;

    /* MODER: alternate function (0b10) */
    tmp = GPIOx(GPIOA_BASE, GPIO_MODER);
    tmp &= ~(0x3u << (11 * 2));
    tmp |=  (0x2u << (11 * 2));
    tmp &= ~(0x3u << (12 * 2));
    tmp |=  (0x2u << (12 * 2));
    GPIOx(GPIOA_BASE, GPIO_MODER) = tmp;

    /* --- GPIOC: PC2, PC3 --- */

    /* PUPDR: no pull (0b00) */
    tmp = GPIOx(GPIOC_BASE, GPIO_PUPDR);
    tmp &= ~(0x3u << (2 * 2));
    tmp &= ~(0x3u << (3 * 2));
    GPIOx(GPIOC_BASE, GPIO_PUPDR) = tmp;

    /* OTYPER: push-pull (clear bits) */
    tmp = GPIOx(GPIOC_BASE, GPIO_OTYPER);
    tmp &= ~(1u << 2);
    tmp &= ~(1u << 3);
    GPIOx(GPIOC_BASE, GPIO_OTYPER) = tmp;

    /* OSPEEDR: very high speed (0b11) */
    tmp = GPIOx(GPIOC_BASE, GPIO_OSPEEDR);
    tmp &= ~(0x3u << (2 * 2));
    tmp |=  (0x3u << (2 * 2));
    tmp &= ~(0x3u << (3 * 2));
    tmp |=  (0x3u << (3 * 2));
    GPIOx(GPIOC_BASE, GPIO_OSPEEDR) = tmp;

    /* AFRL: no-op */
    tmp = GPIOx(GPIOC_BASE, GPIO_AFRL);
    GPIOx(GPIOC_BASE, GPIO_AFRL) = tmp;

    /* AFRH: AF0 on PC2, PC3 (matches Rust PAC behavior) */
    tmp = GPIOx(GPIOC_BASE, GPIO_AFRH);
    tmp &= ~(0xFu << ((2 % 8) * 4));
    tmp |=  (0u   << ((2 % 8) * 4));
    tmp &= ~(0xFu << ((3 % 8) * 4));
    tmp |=  (0u   << ((3 % 8) * 4));
    GPIOx(GPIOC_BASE, GPIO_AFRH) = tmp;

    /* MODER: alternate function (0b10) */
    tmp = GPIOx(GPIOC_BASE, GPIO_MODER);
    tmp &= ~(0x3u << (2 * 2));
    tmp |=  (0x2u << (2 * 2));
    tmp &= ~(0x3u << (3 * 2));
    tmp |=  (0x2u << (3 * 2));
    GPIOx(GPIOC_BASE, GPIO_MODER) = tmp;
}

void c_spi2_enable_peripherals(void) {
    /* No peripheral clock enable needed (matches Rust PAC) */
}

void c_spi2_deinit(void) {
    uint32_t tmp;

    /* Return PA11 to analog */
    tmp = GPIOx(GPIOA_BASE, GPIO_MODER);
    tmp &= ~(0x3u << (11 * 2));
    tmp |=  (0x3u << (11 * 2));
    GPIOx(GPIOA_BASE, GPIO_MODER) = tmp;
    GPIOx(GPIOA_BASE, GPIO_PUPDR) &= ~(0x3u << (11 * 2));
    GPIOx(GPIOA_BASE, GPIO_OTYPER) &= ~(1u << 11);

    /* Return PA12 to analog */
    tmp = GPIOx(GPIOA_BASE, GPIO_MODER);
    tmp &= ~(0x3u << (12 * 2));
    tmp |=  (0x3u << (12 * 2));
    GPIOx(GPIOA_BASE, GPIO_MODER) = tmp;
    GPIOx(GPIOA_BASE, GPIO_PUPDR) &= ~(0x3u << (12 * 2));
    GPIOx(GPIOA_BASE, GPIO_OTYPER) &= ~(1u << 12);

    /* Return PC2 to analog */
    tmp = GPIOx(GPIOC_BASE, GPIO_MODER);
    tmp &= ~(0x3u << (2 * 2));
    tmp |=  (0x3u << (2 * 2));
    GPIOx(GPIOC_BASE, GPIO_MODER) = tmp;
    GPIOx(GPIOC_BASE, GPIO_PUPDR) &= ~(0x3u << (2 * 2));
    GPIOx(GPIOC_BASE, GPIO_OTYPER) &= ~(1u << 2);

    /* Return PC3 to analog */
    tmp = GPIOx(GPIOC_BASE, GPIO_MODER);
    tmp &= ~(0x3u << (3 * 2));
    tmp |=  (0x3u << (3 * 2));
    GPIOx(GPIOC_BASE, GPIO_MODER) = tmp;
    GPIOx(GPIOC_BASE, GPIO_PUPDR) &= ~(0x3u << (3 * 2));
    GPIOx(GPIOC_BASE, GPIO_OTYPER) &= ~(1u << 3);

    /* Mask SPI2 interrupt */
    nvic_disable_irq(IRQ_SPI2);
}
