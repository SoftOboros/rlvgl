/* C HAL binding for SPI5 on STM32H747I-DISCO.
 * Pins: PJ10 (MOSI, AF0), PJ11 (MISO, AF0), PK0 (SCK, AF0), PK1 (NSS, AF0).
 * Push-pull, no pull, very high speed.
 * Equivalent to chips/stm/bsps/src/stm32h747i_disco/pac/spi5.rs.
 */

#include "stm32h747xi.h"

void c_spi5_enable_gpio_clocks(void) {
    /* Enable GPIOJ (bit 9) and GPIOK (bit 10) clocks */
    REG32(RCC_AHB4ENR) |= (1u << 9) | (1u << 10);
}

void c_spi5_configure_pins(void) {
    uint32_t tmp;

    /* --- GPIOJ: PJ10, PJ11 --- */

    /* PUPDR: no pull (0b00) */
    tmp = GPIOx(GPIOJ_BASE, GPIO_PUPDR);
    tmp &= ~(0x3u << (10 * 2));
    tmp &= ~(0x3u << (11 * 2));
    GPIOx(GPIOJ_BASE, GPIO_PUPDR) = tmp;

    /* OTYPER: push-pull (clear bits) */
    tmp = GPIOx(GPIOJ_BASE, GPIO_OTYPER);
    tmp &= ~(1u << 10);
    tmp &= ~(1u << 11);
    GPIOx(GPIOJ_BASE, GPIO_OTYPER) = tmp;

    /* OSPEEDR: very high speed (0b11) */
    tmp = GPIOx(GPIOJ_BASE, GPIO_OSPEEDR);
    tmp &= ~(0x3u << (10 * 2));
    tmp |=  (0x3u << (10 * 2));
    tmp &= ~(0x3u << (11 * 2));
    tmp |=  (0x3u << (11 * 2));
    GPIOx(GPIOJ_BASE, GPIO_OSPEEDR) = tmp;

    /* AFRL: no-op */
    tmp = GPIOx(GPIOJ_BASE, GPIO_AFRL);
    GPIOx(GPIOJ_BASE, GPIO_AFRL) = tmp;

    /* AFRH: AF0 on PJ10, PJ11 */
    tmp = GPIOx(GPIOJ_BASE, GPIO_AFRH);
    tmp &= ~(0xFu << ((10 % 8) * 4));
    tmp |=  (0u   << ((10 % 8) * 4));
    tmp &= ~(0xFu << ((11 % 8) * 4));
    tmp |=  (0u   << ((11 % 8) * 4));
    GPIOx(GPIOJ_BASE, GPIO_AFRH) = tmp;

    /* MODER: alternate function (0b10) */
    tmp = GPIOx(GPIOJ_BASE, GPIO_MODER);
    tmp &= ~(0x3u << (10 * 2));
    tmp |=  (0x2u << (10 * 2));
    tmp &= ~(0x3u << (11 * 2));
    tmp |=  (0x2u << (11 * 2));
    GPIOx(GPIOJ_BASE, GPIO_MODER) = tmp;

    /* --- GPIOK: PK0, PK1 --- */

    /* PUPDR: no pull (0b00) */
    tmp = GPIOx(GPIOK_BASE, GPIO_PUPDR);
    tmp &= ~(0x3u << (0 * 2));
    tmp &= ~(0x3u << (1 * 2));
    GPIOx(GPIOK_BASE, GPIO_PUPDR) = tmp;

    /* OTYPER: push-pull (clear bits) */
    tmp = GPIOx(GPIOK_BASE, GPIO_OTYPER);
    tmp &= ~(1u << 0);
    tmp &= ~(1u << 1);
    GPIOx(GPIOK_BASE, GPIO_OTYPER) = tmp;

    /* OSPEEDR: very high speed (0b11) */
    tmp = GPIOx(GPIOK_BASE, GPIO_OSPEEDR);
    tmp &= ~(0x3u << (0 * 2));
    tmp |=  (0x3u << (0 * 2));
    tmp &= ~(0x3u << (1 * 2));
    tmp |=  (0x3u << (1 * 2));
    GPIOx(GPIOK_BASE, GPIO_OSPEEDR) = tmp;

    /* AFRL: no-op */
    tmp = GPIOx(GPIOK_BASE, GPIO_AFRL);
    GPIOx(GPIOK_BASE, GPIO_AFRL) = tmp;

    /* AFRH: AF0 on PK0, PK1 (matches Rust PAC behavior) */
    tmp = GPIOx(GPIOK_BASE, GPIO_AFRH);
    tmp &= ~(0xFu << ((0 % 8) * 4));
    tmp |=  (0u   << ((0 % 8) * 4));
    tmp &= ~(0xFu << ((1 % 8) * 4));
    tmp |=  (0u   << ((1 % 8) * 4));
    GPIOx(GPIOK_BASE, GPIO_AFRH) = tmp;

    /* MODER: alternate function (0b10) */
    tmp = GPIOx(GPIOK_BASE, GPIO_MODER);
    tmp &= ~(0x3u << (0 * 2));
    tmp |=  (0x2u << (0 * 2));
    tmp &= ~(0x3u << (1 * 2));
    tmp |=  (0x2u << (1 * 2));
    GPIOx(GPIOK_BASE, GPIO_MODER) = tmp;
}

void c_spi5_enable_peripherals(void) {
    /* No peripheral clock enable needed (matches Rust PAC) */
}

void c_spi5_deinit(void) {
    uint32_t tmp;

    /* Return PJ10 to analog */
    tmp = GPIOx(GPIOJ_BASE, GPIO_MODER);
    tmp &= ~(0x3u << (10 * 2));
    tmp |=  (0x3u << (10 * 2));
    GPIOx(GPIOJ_BASE, GPIO_MODER) = tmp;
    GPIOx(GPIOJ_BASE, GPIO_PUPDR) &= ~(0x3u << (10 * 2));
    GPIOx(GPIOJ_BASE, GPIO_OTYPER) &= ~(1u << 10);

    /* Return PJ11 to analog */
    tmp = GPIOx(GPIOJ_BASE, GPIO_MODER);
    tmp &= ~(0x3u << (11 * 2));
    tmp |=  (0x3u << (11 * 2));
    GPIOx(GPIOJ_BASE, GPIO_MODER) = tmp;
    GPIOx(GPIOJ_BASE, GPIO_PUPDR) &= ~(0x3u << (11 * 2));
    GPIOx(GPIOJ_BASE, GPIO_OTYPER) &= ~(1u << 11);

    /* Return PK0 to analog */
    tmp = GPIOx(GPIOK_BASE, GPIO_MODER);
    tmp &= ~(0x3u << (0 * 2));
    tmp |=  (0x3u << (0 * 2));
    GPIOx(GPIOK_BASE, GPIO_MODER) = tmp;
    GPIOx(GPIOK_BASE, GPIO_PUPDR) &= ~(0x3u << (0 * 2));
    GPIOx(GPIOK_BASE, GPIO_OTYPER) &= ~(1u << 0);

    /* Return PK1 to analog */
    tmp = GPIOx(GPIOK_BASE, GPIO_MODER);
    tmp &= ~(0x3u << (1 * 2));
    tmp |=  (0x3u << (1 * 2));
    GPIOx(GPIOK_BASE, GPIO_MODER) = tmp;
    GPIOx(GPIOK_BASE, GPIO_PUPDR) &= ~(0x3u << (1 * 2));
    GPIOx(GPIOK_BASE, GPIO_OTYPER) &= ~(1u << 1);

    /* Mask SPI5 interrupt */
    nvic_disable_irq(IRQ_SPI5);
}
