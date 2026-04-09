// tests/ast_from_c.rs — Smoke test for C-source IR extraction.

#[path = "../src/bin/creator/ast.rs"]
mod ast;

#[test]
fn extract_basic_gpio_and_kernel() {
    let tmp = tempfile::tempdir().unwrap();
    let c_path = tmp.path().join("main.c");
    std::fs::write(
        &c_path,
        r#"
        void mx(void) {
            GPIO_InitTypeDef GPIO_InitStruct = {0};
            __HAL_RCC_USART1_CLK_ENABLE();
            GPIO_InitStruct.Pin = GPIO_PIN_9;
            GPIO_InitStruct.Mode = GPIO_MODE_AF_PP;
            GPIO_InitStruct.Pull = GPIO_NOPULL;
            GPIO_InitStruct.Speed = GPIO_SPEED_FREQ_VERY_HIGH;
            GPIO_InitStruct.Alternate = GPIO_AF7_USART1;
            HAL_GPIO_Init(GPIOA, &GPIO_InitStruct);
        }
        "#,
    )
    .unwrap();

    let ir = ast::extract_from_c_sources(
        &[c_path],
        ast::ExtractOptions {
            mcu: "STM32H747XIHx",
            package: "LQFP176",
        },
    )
    .unwrap();

    assert_eq!(ir.mcu, "STM32H747XIHx");
    assert_eq!(ir.package, "LQFP176");
    assert!(ir.clocks.kernels.contains_key(&"usart1".to_string()));
    assert!(ir.pinctrl.iter().any(|p| p.pin == "PA9"));
    // Ensure AF number captured
    assert!(ir.pinctrl.iter().any(|p| p.af == 7));
}
