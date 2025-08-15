//! STM32H747I-DISCO display and touch drivers.
//!
//! Offers a minimal bring-up path for the discovery board's MIPI-DSI display
//! and touch peripherals. The display driver enables LTDC and DSI clocks,
//! issues a short initialization sequence to the OTM8009A panel, and configures
//! layer 1 for an RGB565 framebuffer. Touch input is provided via the FT5336
//! controller. Backlight PWM and panel reset control are wired through
//! `embedded-hal` traits.

#[cfg(feature = "stm32h747i_disco")]
use crate::ft5336::Ft5336;
#[cfg(all(
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_arch = "aarch64")
))]
use crate::otm8009a::Otm8009a;
use crate::{Blitter, DisplayDriver, InputDevice};
#[cfg(feature = "stm32h747i_disco")]
use embedded_hal::{digital::InputPin, i2c::I2c, i2c::SevenBitAddress};
#[cfg(feature = "stm32h747i_disco")]
use embedded_hal::{digital::OutputPin, pwm::PwmPin};
use rlvgl_core::event::Event;
use rlvgl_core::widget::{Color, Rect};
#[cfg(all(
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_arch = "aarch64")
))]
use stm32h7::stm32h747cm7::{DSIHOST, FMC, LTDC, RCC};

/// Display driver for the STM32H747I-DISCO board.
///
/// Wraps a [`Blitter`] and configures LTDC/DSI clocks. The actual flush path is
/// still unimplemented and will eventually transfer pixel data over MIPI-DSI.
pub struct Stm32h747iDiscoDisplay<B: Blitter, BL = (), RST = ()> {
    blitter: B,
    #[cfg(feature = "stm32h747i_disco")]
    backlight: BL,
    #[cfg(feature = "stm32h747i_disco")]
    reset: RST,
    #[cfg(all(
        feature = "stm32h747i_disco",
        any(target_arch = "arm", target_arch = "aarch64")
    ))]
    ltdc: LTDC,
    #[cfg(all(
        feature = "stm32h747i_disco",
        any(target_arch = "arm", target_arch = "aarch64")
    ))]
    dsi: DSIHOST,
}

impl<B: Blitter, BL, RST> Stm32h747iDiscoDisplay<B, BL, RST> {
    /// Create a new display driver, enabling LTDC and DSI clocks and preparing
    /// the panel control pins.
    #[cfg(all(
        feature = "stm32h747i_disco",
        any(target_arch = "arm", target_arch = "aarch64")
    ))]
    pub fn new(
        blitter: B,
        backlight: BL,
        mut reset: RST,
        ltdc: LTDC,
        dsi: DSIHOST,
        fmc: FMC,
        rcc: &mut RCC,
    ) -> Self
    where
        BL: PwmPin,
        RST: OutputPin,
    {
        // Enable LTDC and DSI peripheral clocks
        rcc.apb3enr
            .modify(|_, w| w.ltdcen().set_bit().dsien().set_bit());
        // Ensure the panel is held in reset and the backlight is off
        let _ = reset.set_low();
        let mut disp = Self {
            blitter,
            backlight,
            reset,
            ltdc,
            dsi,
        };
        disp.backlight.disable();
        disp.reset_panel();
        Otm8009a::init(&mut disp.dsi);
        let fb = Self::init_sdram(fmc, rcc);
        disp.setup_ltdc_layer(fb, 800, 480);
        disp
    }

    #[cfg(feature = "stm32h747i_disco")]
    fn set_backlight(&mut self, level: BL::Duty)
    where
        BL: PwmPin,
    {
        self.backlight.set_duty(level);
        self.backlight.enable();
    }

    #[cfg(feature = "stm32h747i_disco")]
    fn reset_panel(&mut self)
    where
        RST: OutputPin,
    {
        let _ = self.reset.set_low();
        // A real implementation would delay here to satisfy the reset timing
        let _ = self.reset.set_high();
    }

    #[cfg(all(
        feature = "stm32h747i_disco",
        any(target_arch = "arm", target_arch = "aarch64")
    ))]
    fn setup_ltdc_layer(&mut self, fb: u32, width: u16, height: u16) {
        use stm32h7::stm32h747cm7::ltdc::layer::pfcr::PF;
        let pitch = width * 2; // RGB565
        self.ltdc.layer[0].cfbar.write(|w| w.cfbadd().bits(fb));
        self.ltdc.layer[0]
            .cfblr
            .write(|w| w.cfbll().bits(pitch + 3).cfbp().bits(pitch));
        self.ltdc.layer[0]
            .cfblnr
            .write(|w| unsafe { w.cfblnbr().bits(height) });
        self.ltdc.layer[0]
            .pfcr
            .write(|w| w.pf().variant(PF::Rgb565));
        self.ltdc.layer[0].cr.modify(|_, w| w.len().enabled());
        self.ltdc.srcr.write(|w| w.imr().reload());
    }

    #[cfg(all(
        feature = "stm32h747i_disco",
        any(target_arch = "arm", target_arch = "aarch64")
    ))]
    /// Initialize the external SDRAM and return its base address.
    fn init_sdram(fmc: FMC, rcc: &mut RCC) -> u32 {
        // Enable the FMC interface clock for SDRAM access
        rcc.ahb3enr.modify(|_, w| w.fmcen().set_bit());

        // Configure SDRAM control and timing registers for the IS42S32800G
        fmc.sdcr[0].write(|w| {
            w.nc()
                .bits(0b01) // 9 column bits
                .nr()
                .bits(0b01) // 12 row bits
                .mwid()
                .bits(0b10) // 32-bit width
                .nb()
                .bits(0b01) // 4 internal banks
                .cas()
                .bits(0b11) // CAS latency 3
                .wp()
                .clear_bit()
                .sdclk()
                .bits(0b10) // SDRAM clock = HCLK/2
                .rburst()
                .set_bit()
                .rpipe()
                .bits(0)
        });
        fmc.sdtr[0].write(|w| {
            w.tmrd()
                .bits(2 - 1)
                .txsr()
                .bits(7 - 1)
                .tras()
                .bits(4 - 1)
                .trc()
                .bits(7 - 1)
                .twr()
                .bits(2 - 1)
                .trp()
                .bits(2 - 1)
                .trcd()
                .bits(2 - 1)
        });

        // Clock enable command
        fmc.sdcmr
            .write(|w| unsafe { w.mode().bits(1).ctb1().set_bit() });
        while fmc.sdsr.read().busy().bit_is_set() {}
        // Precharge all command
        fmc.sdcmr
            .write(|w| unsafe { w.mode().bits(2).ctb1().set_bit() });
        while fmc.sdsr.read().busy().bit_is_set() {}
        // Auto-refresh command
        fmc.sdcmr
            .write(|w| unsafe { w.mode().bits(3).ctb1().set_bit().nrfs().bits(8) });
        while fmc.sdsr.read().busy().bit_is_set() {}
        // Load mode register command with burst length =1, CAS=3
        fmc.sdcmr
            .write(|w| unsafe { w.mode().bits(4).ctb1().set_bit().mrd().bits(0x0231) });
        while fmc.sdsr.read().busy().bit_is_set() {}

        // Set refresh rate (approx. 64ms/4096 rows @100MHz)
        fmc.sdrtr.write(|w| unsafe { w.count().bits(0x0606) });

        // Return base address of SDRAM bank1
        0xC000_0000
    }
}

impl<B: Blitter> DisplayDriver for Stm32h747iDiscoDisplay<B> {
    fn flush(&mut self, _area: Rect, _colors: &[Color]) {
        todo!("MIPI-DSI flush not yet implemented");
    }
}

/// Touch input driver for the STM32H747I-DISCO board.
///
/// Polls the FT5336 capacitive controller over I²C.
#[cfg(feature = "stm32h747i_disco")]
pub struct Stm32h747iDiscoInput<I2C, INT = ()> {
    touch: Ft5336<I2C>,
    int: Option<INT>,
    last: Option<(u16, u16)>,
}

#[cfg(feature = "stm32h747i_disco")]
impl<I2C> Stm32h747iDiscoInput<I2C>
where
    I2C: I2c<SevenBitAddress>,
{
    /// Create a new input driver from an initialized I²C peripheral.
    pub fn new(i2c: I2C) -> Self {
        Self {
            touch: Ft5336::new(i2c),
            int: None,
            last: None,
        }
    }
}

#[cfg(feature = "stm32h747i_disco")]
impl<I2C, INT> Stm32h747iDiscoInput<I2C, INT>
where
    I2C: I2c<SevenBitAddress>,
    INT: InputPin,
{
    /// Create a new input driver using an interrupt line.
    pub fn new_with_int(i2c: I2C, int: INT) -> Self {
        Self {
            touch: Ft5336::new(i2c),
            int: Some(int),
            last: None,
        }
    }

    fn int_active(&mut self) -> bool {
        self.int
            .as_mut()
            .map(|p| p.is_low().unwrap_or(false))
            .unwrap_or(true)
    }
}

#[cfg(feature = "stm32h747i_disco")]
impl<I2C> InputDevice for Stm32h747iDiscoInput<I2C>
where
    I2C: I2c<SevenBitAddress>,
{
    fn poll(&mut self) -> Option<Event> {
        let touch = self.touch.read_touch().ok()?;
        match (touch, self.last) {
            (Some((x, y)), Some((lx, ly))) => {
                self.last = Some((x, y));
                if (x, y) != (lx, ly) {
                    Some(Event::PointerMove {
                        x: x as i32,
                        y: y as i32,
                    })
                } else {
                    None
                }
            }
            (Some((x, y)), None) => {
                self.last = Some((x, y));
                Some(Event::PointerDown {
                    x: x as i32,
                    y: y as i32,
                })
            }
            (None, Some((lx, ly))) => {
                self.last = None;
                Some(Event::PointerUp {
                    x: lx as i32,
                    y: ly as i32,
                })
            }
            (None, None) => None,
        }
    }
}

#[cfg(feature = "stm32h747i_disco")]
impl<I2C, INT> InputDevice for Stm32h747iDiscoInput<I2C, INT>
where
    I2C: I2c<SevenBitAddress>,
    INT: InputPin,
{
    fn poll(&mut self) -> Option<Event> {
        if !self.int_active() {
            return None;
        }
        let touch = self.touch.read_touch().ok()?;
        match (touch, self.last) {
            (Some((x, y)), Some((lx, ly))) => {
                self.last = Some((x, y));
                if (x, y) != (lx, ly) {
                    Some(Event::PointerMove {
                        x: x as i32,
                        y: y as i32,
                    })
                } else {
                    None
                }
            }
            (Some((x, y)), None) => {
                self.last = Some((x, y));
                Some(Event::PointerDown {
                    x: x as i32,
                    y: y as i32,
                })
            }
            (None, Some((lx, ly))) => {
                self.last = None;
                Some(Event::PointerUp {
                    x: lx as i32,
                    y: ly as i32,
                })
            }
            (None, None) => None,
        }
    }
}
