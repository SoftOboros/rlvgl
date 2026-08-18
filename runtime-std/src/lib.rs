//! Native-thread ownership scaffold for interpreter-neutral rlvgl state.
//!
//! This first CPY-02 migration slice proves a narrow property: state that is
//! intentionally not [`Send`] can be constructed, used, and destroyed on one
//! native thread without crossing the join boundary. It does not yet define
//! the CPY-03 service lifecycle, queues, readiness, frames, or shutdown.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

use std::{fmt, io, thread};

/// A joinable one-shot task whose state remains entirely on its owner thread.
///
/// The state type does not need to implement [`Send`]. Only the builder, task
/// closure, and returned output cross the spawn/join boundaries. Dropping this
/// handle detaches the one-shot task exactly as dropping a standard
/// [`thread::JoinHandle`] does; this scaffold makes no CPY-03 service-close
/// claim.
#[derive(Debug)]
#[must_use = "join the native task or explicitly accept detaching it"]
pub struct OwnedThreadTask<Output> {
    handle: thread::JoinHandle<Output>,
}

impl<Output> OwnedThreadTask<Output> {
    /// Return the native thread's stable identity.
    pub fn thread_id(&self) -> thread::ThreadId {
        self.handle.thread().id()
    }

    /// Return the configured native thread name.
    pub fn thread_name(&self) -> Option<&str> {
        self.handle.thread().name()
    }

    /// Return whether the native task has finished.
    pub fn is_finished(&self) -> bool {
        self.handle.is_finished()
    }

    /// Wait for the native task and return its interpreter-neutral output.
    ///
    /// A panic payload is deliberately not projected across this boundary.
    /// CPY-03 will own the eventual stable service-fault taxonomy.
    pub fn join(self) -> Result<Output, NativeTaskJoinError> {
        self.handle
            .join()
            .map_err(|_| NativeTaskJoinError::Panicked)
    }
}

/// Stable failure class for the pre-service one-shot join boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeTaskJoinError {
    /// The owner thread unwound before returning its output.
    Panicked,
}

impl fmt::Display for NativeTaskJoinError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Panicked => formatter.write_str("native owner thread panicked"),
        }
    }
}

impl std::error::Error for NativeTaskJoinError {}

/// Spawn a named native thread that owns a possibly non-`Send` state value.
///
/// `build` runs after the native thread starts. The resulting state never
/// leaves that thread: `run` receives it by mutable reference, then the state
/// is dropped before the output becomes joinable. This matches the ownership
/// requirement for rlvgl's non-`Send` [`rlvgl_core::endpoint::Endpoint`]
/// without selecting any CPY-03 queue or lifecycle policy.
pub fn spawn_owned_thread_task<State, Output, Build, Run>(
    name: impl Into<String>,
    build: Build,
    run: Run,
) -> io::Result<OwnedThreadTask<Output>>
where
    State: 'static,
    Output: Send + 'static,
    Build: FnOnce() -> State + Send + 'static,
    Run: FnOnce(&mut State) -> Output + Send + 'static,
{
    let handle = thread::Builder::new().name(name.into()).spawn(move || {
        let mut state = build();
        run(&mut state)
    })?;
    Ok(OwnedThreadTask { handle })
}
