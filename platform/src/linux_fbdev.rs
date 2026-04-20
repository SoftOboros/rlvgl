//! Linux framebuffer (`/dev/fb0`) display driver.
//!
//! Implements [`DisplayDriver`] by memory-mapping the kernel framebuffer
//! device and writing pixels directly. Intended for embedded Linux targets
//! such as the BeagleBone Black where a lightweight, dependency-free path
//! to the panel is preferred over a full windowing stack.

use std::eprintln;

use crate::display::DisplayDriver;
use crate::screen::Screen;
use rlvgl_core::widget::{Color, Rect};
use std::fs::{File, OpenOptions};
use std::os::unix::io::AsRawFd;

// ioctl constants for fbdev
const FBIOGET_VSCREENINFO: libc::c_ulong = 0x4600;
const FBIOGET_FSCREENINFO: libc::c_ulong = 0x4602;

/// Variable screen info from the kernel (subset of `fb_var_screeninfo`).
#[repr(C)]
#[derive(Default, Clone, Copy)]
struct FbVarScreeninfo {
    xres: u32,
    yres: u32,
    xres_virtual: u32,
    yres_virtual: u32,
    xoffset: u32,
    yoffset: u32,
    bits_per_pixel: u32,
    grayscale: u32,
    red: FbBitfield,
    green: FbBitfield,
    blue: FbBitfield,
    transp: FbBitfield,
    nonstd: u32,
    activate: u32,
    height: u32,
    width: u32,
    accel_flags: u32,
    pixclock: u32,
    left_margin: u32,
    right_margin: u32,
    upper_margin: u32,
    lower_margin: u32,
    hsync_len: u32,
    vsync_len: u32,
    sync: u32,
    vmode: u32,
    rotate: u32,
    colorspace: u32,
    reserved: [u32; 4],
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct FbBitfield {
    offset: u32,
    length: u32,
    msb_right: u32,
}

/// Fixed screen info from the kernel (subset of `fb_fix_screeninfo`).
#[repr(C)]
#[derive(Clone, Copy)]
struct FbFixScreeninfo {
    id: [u8; 16],
    smem_start: libc::c_ulong,
    smem_len: u32,
    kind: u32,
    type_aux: u32,
    visual: u32,
    xpanstep: u16,
    ypanstep: u16,
    ywrapstep: u16,
    line_length: u32,
    mmio_start: libc::c_ulong,
    mmio_len: u32,
    accel: u32,
    capabilities: u16,
    reserved: [u16; 2],
}

impl Default for FbFixScreeninfo {
    fn default() -> Self {
        // Safety: all-zero is valid for this struct
        unsafe { core::mem::zeroed() }
    }
}

/// Linux framebuffer display driver.
///
/// Opens `/dev/fb0` (or a user-specified path), queries geometry, and
/// memory-maps the framebuffer for direct pixel writes.
pub struct LinuxFbdevDisplay {
    _file: File,
    mmap_ptr: *mut u8,
    mmap_len: usize,
    width: u32,
    height: u32,
    stride: u32,
    bpp: u32,
}

// The mmap pointer is process-wide and not shared across threads.
unsafe impl Send for LinuxFbdevDisplay {}

impl LinuxFbdevDisplay {
    /// Open a framebuffer device and memory-map it.
    ///
    /// # Panics
    ///
    /// Panics if the device cannot be opened, queried, or mapped.
    pub fn open(path: &str) -> Self {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .unwrap_or_else(|e| panic!("failed to open {path}: {e}"));
        let fd = file.as_raw_fd();

        let mut vinfo = FbVarScreeninfo::default();
        let mut finfo = FbFixScreeninfo::default();

        unsafe {
            if libc::ioctl(fd, FBIOGET_VSCREENINFO, &mut vinfo as *mut _) != 0 {
                panic!("FBIOGET_VSCREENINFO failed");
            }
            if libc::ioctl(fd, FBIOGET_FSCREENINFO, &mut finfo as *mut _) != 0 {
                panic!("FBIOGET_FSCREENINFO failed");
            }
        }

        let mmap_len = finfo.smem_len as usize;
        let mmap_ptr = unsafe {
            libc::mmap(
                core::ptr::null_mut(),
                mmap_len,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                fd,
                0,
            )
        };
        if mmap_ptr == libc::MAP_FAILED {
            panic!("mmap failed for {path}");
        }

        eprintln!(
            "fbdev: {}x{} @ {}bpp, stride={}, mmap={}K",
            vinfo.xres,
            vinfo.yres,
            vinfo.bits_per_pixel,
            finfo.line_length,
            mmap_len / 1024,
        );

        Self {
            _file: file,
            mmap_ptr: mmap_ptr as *mut u8,
            mmap_len,
            width: vinfo.xres,
            height: vinfo.yres,
            stride: finfo.line_length,
            bpp: vinfo.bits_per_pixel,
        }
    }

    /// Open the default `/dev/fb0` device.
    pub fn open_default() -> Self {
        Self::open("/dev/fb0")
    }
}

impl Drop for LinuxFbdevDisplay {
    fn drop(&mut self) {
        unsafe {
            libc::munmap(self.mmap_ptr as *mut libc::c_void, self.mmap_len);
        }
    }
}

impl DisplayDriver for LinuxFbdevDisplay {
    fn screen(&self) -> Screen {
        Screen::landscape(self.width, self.height)
    }

    fn flush(&mut self, area: Rect, colors: &[Color]) {
        let fb = unsafe { core::slice::from_raw_parts_mut(self.mmap_ptr, self.mmap_len) };

        for row in 0..area.height as usize {
            let py = area.y as usize + row;
            if py >= self.height as usize {
                break;
            }
            for col in 0..area.width as usize {
                let px = area.x as usize + col;
                if px >= self.width as usize {
                    break;
                }
                let color = colors[row * area.width as usize + col];
                let offset = py * self.stride as usize + px * (self.bpp / 8) as usize;

                match self.bpp {
                    32 => {
                        // BGRA8888 (typical Linux fbdev byte order)
                        if offset + 3 < fb.len() {
                            fb[offset] = color.2; // B
                            fb[offset + 1] = color.1; // G
                            fb[offset + 2] = color.0; // R
                            fb[offset + 3] = color.3; // A
                        }
                    }
                    16 => {
                        // RGB565
                        if offset + 1 < fb.len() {
                            let r5 = (color.0 >> 3) as u16;
                            let g6 = (color.1 >> 2) as u16;
                            let b5 = (color.2 >> 3) as u16;
                            let pixel = (r5 << 11) | (g6 << 5) | b5;
                            fb[offset] = pixel as u8;
                            fb[offset + 1] = (pixel >> 8) as u8;
                        }
                    }
                    24 => {
                        // BGR888
                        if offset + 2 < fb.len() {
                            fb[offset] = color.2; // B
                            fb[offset + 1] = color.1; // G
                            fb[offset + 2] = color.0; // R
                        }
                    }
                    _ => {} // unsupported bpp, skip
                }
            }
        }
    }
}
