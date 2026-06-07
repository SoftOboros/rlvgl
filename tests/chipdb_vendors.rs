//! Integration tests for vendor chip database crates.

macro_rules! vendor_check {
    ($name:ident, $krate:ident, $vendor:literal, $board:literal, $chip:literal) => {
        #[test]
        fn $name() {
            assert_eq!($krate::vendor(), $vendor);
            let boards = $krate::boards();
            assert!(!boards.is_empty());
            let found = $krate::find($board).expect(concat!("board not found: ", $board));
            assert_eq!(found.board, $board);
            assert_eq!(found.chip, $chip);
        }
    };
}

vendor_check!(stm, rlvgl_chips_stm, "stm", "STM32F4DISCOVERY", "STM32F407");
vendor_check!(nrf, rlvgl_chips_nrf, "nrf", "nRF52840-DK", "nRF52840");
vendor_check!(
    esp,
    rlvgl_chips_esp,
    "esp",
    "ESP32-C3-DevKitM-1",
    "ESP32-C3"
);
vendor_check!(nxp, rlvgl_chips_nxp, "nxp", "MIMXRT1060-EVKB", "MIMXRT1062");
vendor_check!(
    silabs,
    rlvgl_chips_silabs,
    "silabs",
    "SLSTK3701A",
    "EFM32GG11"
);
vendor_check!(
    microchip,
    rlvgl_chips_microchip,
    "microchip",
    "Adafruit Feather M4 Express",
    "ATSAMD51J19A"
);
vendor_check!(
    renesas,
    rlvgl_chips_renesas,
    "renesas",
    "EK-RA6M5",
    "R7FA6M5BH"
);
vendor_check!(ti, rlvgl_chips_ti, "ti", "MSP432P401R", "MSP432P401R");
vendor_check!(
    rp2040,
    rlvgl_chips_rp2040,
    "rp2040",
    "Raspberry Pi Pico",
    "RP2040"
);
