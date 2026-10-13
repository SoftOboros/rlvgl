//! Race-safe operating-system readiness for the CPY native service.

use std::io;

#[cfg(unix)]
mod unix {
    use std::{
        io,
        os::fd::{AsFd, BorrowedFd, OwnedFd},
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
    };

    use rustix::io::{Errno, read, write};
    #[cfg(not(target_os = "linux"))]
    use rustix::io::{FdFlags, fcntl_getfd, fcntl_setfd};

    /// Operating-system primitive behind a [`ReadinessSignal`].
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum ReadinessKind {
        /// A Linux nonblocking, close-on-exec `eventfd`.
        EventFd,
        /// A nonblocking, close-on-exec Unix self-pipe.
        SelfPipe,
    }

    #[derive(Debug)]
    struct Shared {
        pending: AtomicBool,
        writer: OwnedFd,
        kind: ReadinessKind,
    }

    /// Level-triggered readiness handle for a native service's egress queue.
    ///
    /// The descriptor contains no semantic data. Consumers use it only to
    /// learn that records may be drainable, then call the service drain API.
    #[derive(Debug)]
    pub struct ReadinessSignal {
        reader: OwnedFd,
        shared: Arc<Shared>,
    }

    impl ReadinessSignal {
        /// Return the platform primitive used by this handle.
        pub fn kind(&self) -> ReadinessKind {
            self.shared.kind
        }

        pub(crate) fn clear_before_drain(&self) -> io::Result<()> {
            let mut buffer = [0_u8; 64];
            loop {
                match read(&self.reader, &mut buffer) {
                    Ok(0) => return Ok(()),
                    Ok(_) if self.shared.kind == ReadinessKind::EventFd => return Ok(()),
                    Ok(_) => continue,
                    Err(Errno::AGAIN) => return Ok(()),
                    Err(error) => return Err(error.into()),
                }
            }
        }

        pub(crate) fn finish_drain(&self, queue_nonempty: bool) -> io::Result<()> {
            self.shared.pending.store(false, Ordering::Release);
            if queue_nonempty {
                Notifier {
                    shared: Arc::clone(&self.shared),
                }
                .notify()?;
            }
            Ok(())
        }
    }

    impl AsFd for ReadinessSignal {
        fn as_fd(&self) -> BorrowedFd<'_> {
            self.reader.as_fd()
        }
    }

    #[derive(Clone, Debug)]
    pub(crate) struct Notifier {
        shared: Arc<Shared>,
    }

    impl Notifier {
        pub(crate) fn notify(&self) -> io::Result<bool> {
            if self.shared.pending.swap(true, Ordering::AcqRel) {
                return Ok(false);
            }
            let bytes: &[u8] = if self.shared.kind == ReadinessKind::EventFd {
                &1_u64.to_ne_bytes()
            } else {
                &[1]
            };
            match write(&self.shared.writer, bytes) {
                Ok(_) => Ok(true),
                Err(Errno::AGAIN) => Ok(false),
                Err(error) => {
                    self.shared.pending.store(false, Ordering::Release);
                    Err(error.into())
                }
            }
        }
    }

    pub(crate) fn pair() -> io::Result<(ReadinessSignal, Notifier)> {
        #[cfg(target_os = "linux")]
        let (reader, writer, kind) = {
            use rustix::event::{EventfdFlags, eventfd};
            let writer = eventfd(0, EventfdFlags::CLOEXEC | EventfdFlags::NONBLOCK)?;
            let reader = rustix::io::fcntl_dupfd_cloexec(&writer, 0)?;
            (reader, writer, ReadinessKind::EventFd)
        };

        #[cfg(not(target_os = "linux"))]
        let (reader, writer, kind) = {
            use rustix::{
                fs::{OFlags, fcntl_getfl, fcntl_setfl},
                pipe::pipe,
            };
            let (reader, writer) = pipe()?;
            for descriptor in [&reader, &writer] {
                let status = fcntl_getfl(descriptor)?;
                fcntl_setfl(descriptor, status | OFlags::NONBLOCK)?;
                let flags = fcntl_getfd(descriptor)?;
                fcntl_setfd(descriptor, flags | FdFlags::CLOEXEC)?;
            }
            (reader, writer, ReadinessKind::SelfPipe)
        };

        let shared = Arc::new(Shared {
            pending: AtomicBool::new(false),
            writer,
            kind,
        });
        Ok((
            ReadinessSignal {
                reader,
                shared: Arc::clone(&shared),
            },
            Notifier { shared },
        ))
    }
}

#[cfg(not(unix))]
mod portable {
    use std::{
        io,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
    };

    /// In-process fallback used only outside the selected CPY Unix matrix.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum ReadinessKind {
        /// No pollable descriptor is available on this unqualified target.
        InProcessOnly,
    }

    /// Non-pollable readiness state for targets outside the CPY Unix matrix.
    #[derive(Debug)]
    pub struct ReadinessSignal {
        pending: Arc<AtomicBool>,
    }

    impl ReadinessSignal {
        /// Return the in-process-only fallback kind.
        pub fn kind(&self) -> ReadinessKind {
            ReadinessKind::InProcessOnly
        }

        pub(crate) fn clear_before_drain(&self) -> io::Result<()> {
            Ok(())
        }

        pub(crate) fn finish_drain(&self, queue_nonempty: bool) -> io::Result<()> {
            self.pending.store(queue_nonempty, Ordering::Release);
            Ok(())
        }
    }

    #[derive(Clone, Debug)]
    pub(crate) struct Notifier {
        pending: Arc<AtomicBool>,
    }

    impl Notifier {
        pub(crate) fn notify(&self) -> io::Result<bool> {
            Ok(!self.pending.swap(true, Ordering::AcqRel))
        }
    }

    pub(crate) fn pair() -> io::Result<(ReadinessSignal, Notifier)> {
        let pending = Arc::new(AtomicBool::new(false));
        Ok((
            ReadinessSignal {
                pending: Arc::clone(&pending),
            },
            Notifier { pending },
        ))
    }
}

#[cfg(not(unix))]
pub(crate) use portable::{Notifier, pair};
#[cfg(not(unix))]
pub use portable::{ReadinessKind, ReadinessSignal};
#[cfg(unix)]
pub(crate) use unix::{Notifier, pair};
#[cfg(unix)]
pub use unix::{ReadinessKind, ReadinessSignal};

pub(crate) fn new_pair() -> io::Result<(ReadinessSignal, Notifier)> {
    pair()
}
