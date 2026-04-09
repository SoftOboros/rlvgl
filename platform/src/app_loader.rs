//! Dynamic application loader for the simulator.
//!
//! Uses `libloading` to load `.dylib`/`.so` files at runtime, resolving the
//! `rlvgl_create_app` and `rlvgl_destroy_app` FFI symbols defined by app
//! crates.
//!
//! # Safety contract
//!
//! The loaded dylib **must** be compiled with the same `rustc` version,
//! target, and `rlvgl-core` version as the host binary. This is naturally
//! satisfied when building from the same workspace. The fat pointer
//! `*mut dyn Application` is stable within a single compiler+target pair.

use std::path::Path;

use libloading::{Library, Symbol};
use rlvgl_core::application::Application;

#[allow(improper_ctypes_definitions)]
type CreateFn = unsafe extern "C" fn() -> *mut dyn Application;
#[allow(improper_ctypes_definitions)]
type DestroyFn = unsafe extern "C" fn(*mut dyn Application);

/// A dynamically loaded application.
///
/// Holds the library handle to keep the dylib loaded, plus a raw pointer to
/// the `Application` instance and the destroy function to call on drop.
pub struct LoadedApp {
    _library: Library,
    app: *mut dyn Application,
    destroy_fn: DestroyFn,
}

impl LoadedApp {
    /// Load an application from the dylib at `path`.
    ///
    /// # Errors
    ///
    /// Returns an error if the library cannot be opened or the expected
    /// symbols are missing.
    ///
    /// # Safety
    ///
    /// The dylib must have been compiled with the same `rustc` and
    /// `rlvgl-core` version as the host.
    pub unsafe fn load(path: impl AsRef<Path>) -> Result<Self, libloading::Error> {
        unsafe {
            let library = Library::new(path.as_ref().as_os_str())?;

            let create: Symbol<CreateFn> =
                library.get(rlvgl_core::application::CREATE_APP_SYMBOL)?;
            let destroy: Symbol<DestroyFn> =
                library.get(rlvgl_core::application::DESTROY_APP_SYMBOL)?;

            let app = create();
            let destroy_fn = *destroy;

            Ok(Self {
                _library: library,
                app,
                destroy_fn,
            })
        }
    }

    /// Get a shared reference to the loaded application.
    pub fn app(&self) -> &dyn Application {
        unsafe { &*self.app }
    }

    /// Get a mutable reference to the loaded application.
    pub fn app_mut(&mut self) -> &mut dyn Application {
        unsafe { &mut *self.app }
    }
}

impl Drop for LoadedApp {
    fn drop(&mut self) {
        unsafe {
            // Call destroy before the library is unloaded (library drops after this field).
            (self.destroy_fn)(self.app);
        }
    }
}
