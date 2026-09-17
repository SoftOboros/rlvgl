//! Optional single-task C adapter for the existing SSD1306 and STTS22H drivers.

use alloc::boxed::Box;
use core::ffi::{c_char, c_void, CStr};
use embedded_hal::i2c::{ErrorType, I2c, Operation};
use rlvgl_core::{
    renderer::Renderer,
    widget::{Color, Rect},
};
use rlvgl_device_stts22h::{Address, Averaging, Config, Stts22h};
use rlvgl_platform::{display::DisplayDriver, Ssd1306Display};
use ssd1306::{
    prelude::{DisplayRotation, DisplaySize128x64},
    I2CDisplayInterface, Ssd1306,
};

type Transfer = unsafe extern "C" fn(u8, *const u8, usize, *mut u8, usize) -> i32;
#[derive(Clone, Copy)]
struct Bus(Transfer);
#[derive(Debug)]
struct BusError;
impl embedded_hal::i2c::Error for BusError {
    fn kind(&self) -> embedded_hal::i2c::ErrorKind {
        embedded_hal::i2c::ErrorKind::Other
    }
}
impl ErrorType for Bus {
    type Error = BusError;
}
impl I2c for Bus {
    fn transaction(
        &mut self,
        address: u8,
        operations: &mut [Operation<'_>],
    ) -> Result<(), BusError> {
        // Preserve the sensor's repeated-start write/read as one IDF operation.
        let result = match operations {
            [Operation::Write(w), Operation::Read(r)] => unsafe {
                (self.0)(address, w.as_ptr(), w.len(), r.as_mut_ptr(), r.len())
            },
            [Operation::Write(w)] => unsafe {
                (self.0)(address, w.as_ptr(), w.len(), core::ptr::null_mut(), 0)
            },
            [Operation::Read(r)] => unsafe {
                (self.0)(address, core::ptr::null(), 0, r.as_mut_ptr(), r.len())
            },
            _ => return Err(BusError),
        };
        if result == 0 {
            Ok(())
        } else {
            Err(BusError)
        }
    }
}
type Oled = Ssd1306Display<display_interface_i2c::I2CInterface<Bus>, DisplaySize128x64>;
struct Diagnostics {
    oled: Option<Oled>,
    sensor: Option<Stts22h<Bus>>,
}

/// Allocate once; the C caller exclusively owns this handle on one task.
/// Returns device-ready bits through `ready`; failed devices remain absent.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rlvgl_diagnostic_new(
    transfer: Transfer,
    oled: bool,
    sensor_address: u8,
    ready: *mut u32,
) -> *mut c_void {
    let bus = Bus(transfer);
    let oled = if oled {
        let raw = Ssd1306::new(
            I2CDisplayInterface::new_custom_address(bus, 0x3c),
            DisplaySize128x64,
            DisplayRotation::Rotate0,
        )
        .into_buffered_graphics_mode();
        Ssd1306Display::new(raw).ok()
    } else {
        None
    };
    let address = match sensor_address {
        0x38 => Some(Address::Vdd),
        0x3e => Some(Address::PullUp56K),
        0x3f => Some(Address::Gnd),
        _ => None,
    };
    let sensor = address.and_then(|a| {
        let mut sensor = Stts22h::with_address(bus, a);
        sensor
            .probe()
            .and_then(|()| sensor.configure(Config::low_odr(Averaging::Samples8)))
            .ok()
            .map(|()| sensor)
    });
    unsafe {
        *ready = u32::from(oled.is_some()) | (u32::from(sensor.is_some()) << 1);
    }
    Box::into_raw(Box::new(Diagnostics { oled, sensor })).cast()
}

/// Read a coherent signed sample. Failure disables further sensor I/O until boot.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rlvgl_diagnostic_sample(
    handle: *mut c_void,
    temperature: *mut i16,
) -> bool {
    let state = unsafe { &mut *handle.cast::<Diagnostics>() };
    if let Some(sensor) = &mut state.sensor {
        if let Ok(value) = sensor.read_temperature() {
            unsafe {
                *temperature = value.centi_celsius();
            }
            return true;
        }
    }
    state.sensor = None;
    false
}

/// Draw at most six newline-separated rows from a caller-owned C string.
/// The C bus adapter retains transfer errors because platform vsync is infallible.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rlvgl_diagnostic_draw(handle: *mut c_void, text: *const c_char) {
    let state = unsafe { &mut *handle.cast::<Diagnostics>() };
    if let Some(display) = &mut state.oled {
        display.fill_rect(
            Rect {
                x: 0,
                y: 0,
                width: 128,
                height: 64,
            },
            Color(0, 0, 0, 255),
        );
        if let Ok(text) = unsafe { CStr::from_ptr(text) }.to_str() {
            for (row, line) in text.lines().take(6).enumerate() {
                display.draw_text((0, 10 + row as i32 * 10), line, Color(255, 255, 255, 255));
            }
        }
        DisplayDriver::vsync(display);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::sync::atomic::{AtomicUsize, Ordering};
    static FAILED_READS: AtomicUsize = AtomicUsize::new(0);

    unsafe extern "C" fn failed_sample(
        address: u8,
        w: *const u8,
        nw: usize,
        r: *mut u8,
        nr: usize,
    ) -> i32 {
        if nr == 2 {
            FAILED_READS.fetch_add(1, Ordering::SeqCst);
            return -1;
        }
        unsafe { mock(address, w, nw, r, nr) }
    }

    #[test]
    fn read_failure_preserves_output_and_disables_retries() {
        FAILED_READS.store(0, Ordering::SeqCst);
        let mut ready = 0;
        let handle = unsafe { rlvgl_diagnostic_new(failed_sample, false, 0x38, &mut ready) };
        assert_eq!(ready, 2);
        let mut value = 123;
        assert!(!unsafe { rlvgl_diagnostic_sample(handle, &mut value) });
        assert!(!unsafe { rlvgl_diagnostic_sample(handle, &mut value) });
        assert_eq!(value, 123);
        assert_eq!(FAILED_READS.load(Ordering::SeqCst), 1);
        unsafe {
            drop(Box::from_raw(handle.cast::<Diagnostics>()));
        }
    }
    unsafe extern "C" fn mock(address: u8, w: *const u8, nw: usize, r: *mut u8, nr: usize) -> i32 {
        assert_eq!(address, 0x38);
        if nr > 0 {
            assert_eq!(nw, 1);
            let register = unsafe { *w };
            let bytes: &[u8] = match register {
                1 => &[0xa0],
                6 => &[0x85, 0xff],
                _ => panic!("unexpected register"),
            };
            assert_eq!(nr, bytes.len());
            unsafe {
                core::ptr::copy_nonoverlapping(bytes.as_ptr(), r, nr);
            }
        }
        0
    }
    #[test]
    fn sensor_only_preserves_signed_units_and_repeated_start() {
        let mut ready = 0;
        let handle = unsafe { rlvgl_diagnostic_new(mock, false, 0x38, &mut ready) };
        assert_eq!(ready, 2);
        let mut value = 0;
        assert!(unsafe { rlvgl_diagnostic_sample(handle, &mut value) });
        assert_eq!(value, -123);
        unsafe {
            drop(Box::from_raw(handle.cast::<Diagnostics>()));
        }
    }
    #[test]
    fn missing_devices_do_not_access_bus() {
        let mut ready = 99;
        let handle = unsafe { rlvgl_diagnostic_new(mock, false, 0, &mut ready) };
        assert_eq!(ready, 0);
        let mut value = 7;
        assert!(!unsafe { rlvgl_diagnostic_sample(handle, &mut value) });
        assert_eq!(value, 7);
        unsafe {
            drop(Box::from_raw(handle.cast::<Diagnostics>()));
        }
    }
}
