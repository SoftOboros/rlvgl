/* C HAL binding for USART1 on STM32H747I-DISCO.
 * Pins: PA9 (TX, AF7), PA10 (RX, AF7).
 * Push-pull, no pull, low speed.
 * Equivalent to chips/stm/bsps/src/stm32h747i_disco/pac/usart1.rs.
 */

#include "stm32h747xi.h"

void c_usart1_enable_gpio_clocks(void) {
    /* Enable GPIOA clock (bit 0 in AHB4ENR) */
    REG32(RCC_AHB4ENR) |= (1u << 0);
}

void c_usart1_configure_pins(void) {
    uint32_t tmp;

    /* PUPDR: no pull (0b00) on PA10, PA9 */
    tmp = GPIOx(GPIOA_BASE, GPIO_PUPDR);
    tmp &= ~(0x3u << (10 * 2));
    tmp &= ~(0x3u << (9 * 2));
    GPIOx(GPIOA_BASE, GPIO_PUPDR) = tmp;

    /* OTYPER: push-pull (clear bits) */
    tmp = GPIOx(GPIOA_BASE, GPIO_OTYPER);
    tmp &= ~(1u << 10);
    tmp &= ~(1u << 9);
    GPIOx(GPIOA_BASE, GPIO_OTYPER) = tmp;

    /* OSPEEDR: low speed (0b00) */
    tmp = GPIOx(GPIOA_BASE, GPIO_OSPEEDR);
    tmp &= ~(0x3u << (10 * 2));
    tmp &= ~(0x3u << (9 * 2));
    GPIOx(GPIOA_BASE, GPIO_OSPEEDR) = tmp;

    /* AFRL: no-op */
    tmp = GPIOx(GPIOA_BASE, GPIO_AFRL);
    GPIOx(GPIOA_BASE, GPIO_AFRL) = tmp;

    /* AFRH: AF7 on PA10, PA9 */
    tmp = GPIOx(GPIOA_BASE, GPIO_AFRH);
    tmp &= ~(0xFu << ((10 % 8) * 4));
    tmp |=  (7u   << ((10 % 8) * 4));
    tmp &= ~(0xFu << ((9 % 8) * 4));
    tmp |=  (7u   << ((9 % 8) * 4));
    GPIOx(GPIOA_BASE, GPIO_AFRH) = tmp;

    /* MODER: alternate function (0b10) */
    tmp = GPIOx(GPIOA_BASE, GPIO_MODER);
    tmp &= ~(0x3u << (10 * 2));
    tmp |=  (0x2u << (10 * 2));
    tmp &= ~(0x3u << (9 * 2));
    tmp |=  (0x2u << (9 * 2));
    GPIOx(GPIOA_BASE, GPIO_MODER) = tmp;
}

void c_usart1_enable_peripherals(void) {
    /* No peripheral clock enable needed (matches Rust PAC) */
}

void c_usart1_deinit(void) {
    uint32_t tmp;

    /* Return PA10 to analog */
    tmp = GPIOx(GPIOA_BASE, GPIO_MODER);
    tmp &= ~(0x3u << (10 * 2));
    tmp |=  (0x3u << (10 * 2));
    GPIOx(GPIOA_BASE, GPIO_MODER) = tmp;
    GPIOx(GPIOA_BASE, GPIO_PUPDR) &= ~(0x3u << (10 * 2));
    GPIOx(GPIOA_BASE, GPIO_OTYPER) &= ~(1u << 10);

    /* Return PA9 to analog */
    tmp = GPIOx(GPIOA_BASE, GPIO_MODER);
    tmp &= ~(0x3u << (9 * 2));
    tmp |=  (0x3u << (9 * 2));
    GPIOx(GPIOA_BASE, GPIO_MODER) = tmp;
    GPIOx(GPIOA_BASE, GPIO_PUPDR) &= ~(0x3u << (9 * 2));
    GPIOx(GPIOA_BASE, GPIO_OTYPER) &= ~(1u << 9);

    /* Mask USART1 interrupt */
    nvic_disable_irq(IRQ_USART1);
}
