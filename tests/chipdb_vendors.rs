//! Integration tests for vendor chip database crates.

macro_rules! vendor_check {
    ($name:ident, $krate:ident, $vendor:literal, $board:literal, $chip:literal) => {
        #[test]
        fn $name() {
            assert_eq!($krate::vendor(), $vendor);
            let boards = $krate::boards();
            assert_eq!(boards.len(), 1);
            assert_eq!(boards[0].board, $board);
            assert_eq!(boards[0].chip, $chip);
            assert!($krate::find($board).is_some());
        }
    };
}

vendor_check!(stm, rlvgl_chips_stm, "stm", "STM32F4DISCOVERY", "STM32F407");
vendor_check!(nrf, rlvgl_chips_nrf, "nrf", "nRF52840-DK", "nRF52840");
vendor_check!(esp, rlvgl_chips_esp, "esp", "ESP32-DevKitC", "ESP32");
vendor_check!(nxp, rlvgl_chips_nxp, "nxp", "LPC1768", "LPC1768");
vendor_check!(
    silabs,
    rlvgl_chips_silabs,
    "silabs",
    "EFM32GG11",
    "EFM32GG11"
);
vendor_check!(
    microchip,
    rlvgl_chips_microchip,
    "microchip",
    "ATSAMD51J19A",
    "ATSAMD51J19A"
);
vendor_check!(
    renesas,
    rlvgl_chips_renesas,
    "renesas",
    "RA6M1-EK",
    "R7FA6M1"
);
vendor_check!(ti, rlvgl_chips_ti, "ti", "MSP432P401R", "MSP432P401R");
vendor_check!(
    rp2040,
    rlvgl_chips_rp2040,
    "rp2040",
    "Raspberry Pi Pico",
    "RP2040"
);
