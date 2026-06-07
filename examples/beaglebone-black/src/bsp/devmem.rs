//! `/dev/mem` page-cache for userspace access to AM335x peripheral registers
//! and a reserved-memory framebuffer region.
//!
//! The [`am335x`](super::am335x) bsp module uses this to translate a
//! physical address `u32` into a mapped virtual `*mut u32` at register-write
//! time, so the same init sequence that runs on bare-metal runs verbatim
//! from a Linux binary (the only difference is how the pointer is
//! resolved).
//!
//! The running kernel must not be using the region in question:
//!
//! - Peripheral registers (LCDC, CM_PER, CTRLMOD, I2C2, GPIO1): unbind
//!   the kernel driver first (see `tools/unbind-tilcdc.sh` for LCDC).
//! - Framebuffer region: reserve via `mem=510M` or `memmap=2M$0x9FE00000`
//!   on the kernel command line (see `tools/reserve-fb.sh`).

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::os::unix::io::AsRawFd;
use std::sync::{Mutex, OnceLock};

/// Physical address of the reserved framebuffer region (top 2 MB of DDR).
pub const FB_PA: u32 = 0x9FE0_0000;

/// Framebuffer size in bytes (800 × 480 × 4, TFT24 unpacked).
pub const FB_SIZE: usize = 800 * 480 * 4;

const PAGE_SIZE: u32 = 4096;
const PAGE_MASK: u32 = PAGE_SIZE - 1;

static DEVMEM: OnceLock<DevMem> = OnceLock::new();

/// Process-wide `/dev/mem` handle plus a cache of 4 KB page mappings.
pub struct DevMem {
    _file: File,
    fd: i32,
    pages: Mutex<HashMap<u32, *mut u8>>,
}

unsafe impl Send for DevMem {}
unsafe impl Sync for DevMem {}

impl DevMem {
    /// Initialize the process-global `/dev/mem` handle. Safe to call more
    /// than once; the first call wins.
    pub fn init() -> &'static DevMem {
        DEVMEM.get_or_init(|| {
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .open("/dev/mem")
                .unwrap_or_else(|e| {
                    panic!(
                        "failed to open /dev/mem ({e}); run as root (sudo) or \
                         ensure /dev/mem access is permitted"
                    )
                });
            let fd = file.as_raw_fd();
            DevMem {
                _file: file,
                fd,
                pages: Mutex::new(HashMap::new()),
            }
        })
    }

    /// Retrieve the initialized handle. Panics if [`DevMem::init`] has
    /// not been called yet.
    pub fn global() -> &'static DevMem {
        DEVMEM
            .get()
            .expect("DevMem::init() must be called before first register access")
    }

    /// Map a multi-page region at a fixed physical address and return the
    /// virtual pointer.  Used for the framebuffer.
    pub fn map_region(&self, pa: u32, size: usize) -> *mut u8 {
        let page_base = pa & !PAGE_MASK;
        let page_off = (pa - page_base) as usize;
        let rounded = ((page_off + size) + PAGE_SIZE as usize - 1) & !(PAGE_SIZE as usize - 1);
        let va = unsafe {
            libc::mmap(
                core::ptr::null_mut(),
                rounded,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                self.fd,
                page_base as libc::off_t,
            )
        };
        if va == libc::MAP_FAILED {
            panic!(
                "mmap(/dev/mem, {rounded} bytes at 0x{page_base:08X}) failed — \
                 make sure the region is reserved via `mem=` or `memmap=` \
                 and that the kernel is not using it"
            );
        }
        unsafe { (va as *mut u8).add(page_off) }
    }

    fn map_page(&self, page_base: u32) -> *mut u8 {
        let mut cache = self.pages.lock().expect("devmem page cache poisoned");
        if let Some(&va) = cache.get(&page_base) {
            return va;
        }
        let va = unsafe {
            libc::mmap(
                core::ptr::null_mut(),
                PAGE_SIZE as usize,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                self.fd,
                page_base as libc::off_t,
            )
        };
        if va == libc::MAP_FAILED {
            panic!("mmap(/dev/mem, 4K at 0x{page_base:08X}) failed");
        }
        let va = va as *mut u8;
        cache.insert(page_base, va);
        va
    }
}

/// Translate a physical register address to a writable virtual pointer.
///
/// # Safety
///
/// The caller must ensure the physical address refers to an AM335x
/// peripheral register that is safe to write from userspace (i.e. no
/// kernel driver is currently bound to the containing device).
#[inline]
pub unsafe fn translate_mut(pa: u32) -> *mut u32 {
    let page_base = pa & !PAGE_MASK;
    let page_off = (pa - page_base) as usize;
    let va = DevMem::global().map_page(page_base);
    unsafe { va.add(page_off) as *mut u32 }
}

/// Translate a physical register address to a read-only virtual pointer.
///
/// # Safety
///
/// See [`translate_mut`].
#[inline]
pub unsafe fn translate(pa: u32) -> *const u32 {
    unsafe { translate_mut(pa) as *const u32 }
}

/// Map the reserved framebuffer region and return `(virtual_pointer, physical_address)`.
pub fn map_framebuffer() -> (*mut u8, u32) {
    let va = DevMem::global().map_region(FB_PA, FB_SIZE);
    (va, FB_PA)
}
