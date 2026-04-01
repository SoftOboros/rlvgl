/* C HAL binding for UART8 on STM32H747I-DISCO.
 * Pins: PJ8 (TX, AF0), PJ9 (RX, AF0).
 * Push-pull, no pull, low speed.
 * Equivalent to chips/stm/bsps/src/stm32h747i_disco/pac/uart8.rs.
 */

#include "stm32h747xi.h"

void c_uart8_enable_gpio_clocks(void) {
    /* Enable GPIOJ clock (bit 9 in AHB4ENR) */
    REG32(RCC_AHB4ENR) |= (1u << 9);
}

void c_uart8_configure_pins(void) {
    uint32_t tmp;

    /* PUPDR: no pull (0b00) on PJ8, PJ9 */
    tmp = GPIOx(GPIOJ_BASE, GPIO_PUPDR);
    tmp &= ~(0x3u << (8 * 2));
    tmp &= ~(0x3u << (9 * 2));
    GPIOx(GPIOJ_BASE, GPIO_PUPDR) = tmp;

    /* OTYPER: push-pull (clear bits) */
    tmp = GPIOx(GPIOJ_BASE, GPIO_OTYPER);
    tmp &= ~(1u << 8);
    tmp &= ~(1u << 9);
    GPIOx(GPIOJ_BASE, GPIO_OTYPER) = tmp;

    /* OSPEEDR: low speed (0b00) */
    tmp = GPIOx(GPIOJ_BASE, GPIO_OSPEEDR);
    tmp &= ~(0x3u << (8 * 2));
    tmp &= ~(0x3u << (9 * 2));
    GPIOx(GPIOJ_BASE, GPIO_OSPEEDR) = tmp;

    /* AFRL: no-op */
    tmp = GPIOx(GPIOJ_BASE, GPIO_AFRL);
    GPIOx(GPIOJ_BASE, GPIO_AFRL) = tmp;

    /* AFRH: AF0 on PJ8, PJ9 */
    tmp = GPIOx(GPIOJ_BASE, GPIO_AFRH);
    tmp &= ~(0xFu << ((8 % 8) * 4));
    tmp |=  (0u   << ((8 % 8) * 4));
    tmp &= ~(0xFu << ((9 % 8) * 4));
    tmp |=  (0u   << ((9 % 8) * 4));
    GPIOx(GPIOJ_BASE, GPIO_AFRH) = tmp;

    /* MODER: alternate function (0b10) */
    tmp = GPIOx(GPIOJ_BASE, GPIO_MODER);
    tmp &= ~(0x3u << (8 * 2));
    tmp |=  (0x2u << (8 * 2));
    tmp &= ~(0x3u << (9 * 2));
    tmp |=  (0x2u << (9 * 2));
    GPIOx(GPIOJ_BASE, GPIO_MODER) = tmp;
}

void c_uart8_enable_peripherals(void) {
    /* No peripheral clock enable needed (matches Rust PAC) */
}

void c_uart8_deinit(void) {
    uint32_t tmp;

    /* Return PJ8 to analog */
    tmp = GPIOx(GPIOJ_BASE, GPIO_MODER);
    tmp &= ~(0x3u << (8 * 2));
    tmp |=  (0x3u << (8 * 2));
    GPIOx(GPIOJ_BASE, GPIO_MODER) = tmp;
    GPIOx(GPIOJ_BASE, GPIO_PUPDR) &= ~(0x3u << (8 * 2));
    GPIOx(GPIOJ_BASE, GPIO_OTYPER) &= ~(1u << 8);

    /* Return PJ9 to analog */
    tmp = GPIOx(GPIOJ_BASE, GPIO_MODER);
    tmp &= ~(0x3u << (9 * 2));
    tmp |=  (0x3u << (9 * 2));
    GPIOx(GPIOJ_BASE, GPIO_MODER) = tmp;
    GPIOx(GPIOJ_BASE, GPIO_PUPDR) &= ~(0x3u << (9 * 2));
    GPIOx(GPIOJ_BASE, GPIO_OTYPER) &= ~(1u << 9);

    /* Mask UART8 interrupt */
    nvic_disable_irq(IRQ_UART8);
}
