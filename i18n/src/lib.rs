//! Thin compile-time i18n for rlvgl.
//!
//! Translations are stored in `locales/*.json`, compiled to a compact binary
//! blob at build time, and embedded via `include_bytes!`.  Use the [`t!`]
//! macro to look up strings:
//!
//! ```ignore
//! use rlvgl_i18n::t;
//!
//! let label = t!("demo.plugins");              // &'static str
//! let msg   = t!("demo.clicks", count = 42);   // String
//! ```
//!
//! ## Binary blob format (RLTN v1)
//!
//! The `.bin` file produced by the build is self-contained and can be copied
//! to an SD card or flash partition.  Call [`load_translations`] at runtime
//! to override the built-in blob with one loaded from media.
//!
//! ```text
//! [0..4]   magic   b"RLTN"
//! [4]      version 1
//! [5]      num_locales
//! [6..8]   num_keys   (u16 LE)
//! [8..]    entries    (num_locales × num_keys) × 6 bytes:
//!            offset: u32 LE  (into string data region)
//!            len:    u16 LE
//! [..]     string data  (UTF-8, packed)
//! ```
#![no_std]

extern crate alloc;

use alloc::string::String;
use core::fmt::{Display, Write};
use core::sync::atomic::{AtomicPtr, AtomicU8, Ordering};

// Generated: Locale enum, Key enum, BUILTIN_BLOB, t! macro, locale_from_u8.
include!(concat!(env!("OUT_DIR"), "/translations.rs"));

const HEADER_SIZE: usize = 8;
const ENTRY_SIZE: usize = 6;

static CURRENT_LOCALE: AtomicU8 = AtomicU8::new(0);

/// Pointer to the active translation blob.  Defaults to the built-in blob;
/// can be swapped at runtime via [`load_translations`].
static ACTIVE_BLOB: AtomicPtr<u8> = AtomicPtr::new(core::ptr::null_mut());

fn active_blob() -> &'static [u8] {
    let ptr = ACTIVE_BLOB.load(Ordering::Relaxed);
    if ptr.is_null() {
        BUILTIN_BLOB
    } else {
        // Safety: load_translations ensures the pointer is valid and 'static.
        unsafe { core::slice::from_raw_parts(ptr, blob_len(ptr)) }
    }
}

/// Read the total blob length from its header + string data.
///
/// # Safety
/// `ptr` must point to a valid RLTN blob.
unsafe fn blob_len(ptr: *const u8) -> usize {
    unsafe {
        let num_locales = *ptr.add(5) as usize;
        let num_keys = u16::from_le_bytes([*ptr.add(6), *ptr.add(7)]) as usize;
        let num_entries = num_locales * num_keys;
        if num_entries == 0 {
            return HEADER_SIZE;
        }
        let last_entry = HEADER_SIZE + (num_entries - 1) * ENTRY_SIZE;
        let offset = u32::from_le_bytes([
            *ptr.add(last_entry),
            *ptr.add(last_entry + 1),
            *ptr.add(last_entry + 2),
            *ptr.add(last_entry + 3),
        ]) as usize;
        let len = u16::from_le_bytes([*ptr.add(last_entry + 4), *ptr.add(last_entry + 5)]) as usize;
        let data_start = HEADER_SIZE + num_entries * ENTRY_SIZE;
        data_start + offset + len
    }
}

/// Override the built-in translations with a blob loaded from media.
///
/// The data must live for `'static` (e.g. `Box::leak` a buffer read from SD).
/// Pass `None` to revert to the built-in blob.
///
/// # Safety
/// The caller must ensure `data` points to a valid RLTN v1 blob whose
/// `num_locales` and `num_keys` match the compiled [`Locale`] and [`Key`]
/// enums.
pub unsafe fn load_translations(data: Option<&'static [u8]>) {
    match data {
        Some(d) => ACTIVE_BLOB.store(d.as_ptr() as *mut u8, Ordering::Release),
        None => ACTIVE_BLOB.store(core::ptr::null_mut(), Ordering::Release),
    }
}

/// Set the active locale for all subsequent `t!()` calls.
pub fn set_locale(locale: Locale) {
    CURRENT_LOCALE.store(locale as u8, Ordering::Relaxed);
}

/// Return the currently active locale.
pub fn locale() -> Locale {
    locale_from_u8(CURRENT_LOCALE.load(Ordering::Relaxed))
}

/// Look up a string in a RLTN blob.  Returns a `&str` with the blob's
/// lifetime (always `'static` for both built-in and leaked FS blobs).
#[inline]
fn lookup_in(blob: &[u8], locale: Locale, key: Key) -> &str {
    let num_keys = u16::from_le_bytes([blob[6], blob[7]]) as usize;
    let idx = (locale as usize) * num_keys + (key as usize);
    let base = HEADER_SIZE + idx * ENTRY_SIZE;
    let offset =
        u32::from_le_bytes([blob[base], blob[base + 1], blob[base + 2], blob[base + 3]]) as usize;
    let len = u16::from_le_bytes([blob[base + 4], blob[base + 5]]) as usize;
    let data_start = HEADER_SIZE + num_keys * (blob[5] as usize) * ENTRY_SIZE;
    let start = data_start + offset;
    // Safety: build.rs guarantees valid UTF-8 in the string data region.
    unsafe { core::str::from_utf8_unchecked(&blob[start..start + len]) }
}

/// Look up a plain (non-parameterized) translation for the current locale.
#[inline]
pub fn t_static(key: Key) -> &'static str {
    // Safety: active_blob() always returns a 'static reference.
    lookup_in(active_blob(), locale(), key)
}

/// Format a translation template by replacing `{name}` placeholders.
pub fn t_format(key: Key, params: &[(&str, &dyn Display)]) -> String {
    let template = t_static(key);
    let mut out = String::with_capacity(template.len() + 16);
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        if let Some(end) = after.find('}') {
            let name = &after[..end];
            if let Some((_, val)) = params.iter().find(|(n, _)| *n == name) {
                let _ = write!(out, "{}", val);
            } else {
                out.push('{');
                out.push_str(name);
                out.push('}');
            }
            rest = &after[end + 1..];
        } else {
            out.push_str(&rest[start..]);
            break;
        }
    }
    out.push_str(rest);
    out
}

/// Return a reference to the built-in binary translation blob.
///
/// Useful for diagnostics or for writing the blob to media.
pub fn builtin_blob() -> &'static [u8] {
    BUILTIN_BLOB
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;

    #[test]
    fn blob_header() {
        assert_eq!(&BUILTIN_BLOB[..4], b"RLTN");
        assert_eq!(BUILTIN_BLOB[4], 1); // version
    }

    #[test]
    fn plain_lookup() {
        set_locale(Locale::En);
        assert_eq!(t!("demo.plugins"), "Plugins");
    }

    #[test]
    fn parameterized() {
        set_locale(Locale::En);
        let s = t!("demo.clicks", count = 42);
        assert_eq!(s, "Clicks: 42");
    }

    #[test]
    fn locale_switch() {
        set_locale(Locale::Fr);
        assert_eq!(t!("demo.plugins"), "Extensions");
        set_locale(Locale::En);
        assert_eq!(t!("demo.plugins"), "Plugins");
    }

    #[test]
    fn parameterized_fr() {
        set_locale(Locale::Fr);
        let s = t!("demo.clicks", count = 7);
        assert_eq!(s, "Clics : 7");
    }
}
