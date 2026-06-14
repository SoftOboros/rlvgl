# ESP-IDF component declaration for the DFR0550 comparison app.
idf_component_register(
    SRCS "dfr0550_idf_compare.c"
    INCLUDE_DIRS "."
    REQUIRES esp_driver_i2c esp_driver_gpio esp_lcd esp_mm esp_timer esp_driver_ppa
)
