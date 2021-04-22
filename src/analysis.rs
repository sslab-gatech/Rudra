mod send_sync_variance;
mod unsafe_dataflow;
mod unsafe_destructor;

use snafu::{Error, ErrorCompat};

pub use send_sync_variance::SendSyncVarianceChecker;
pub use unsafe_dataflow::UnsafeDataflowChecker;
pub use unsafe_destructor::UnsafeDestructorChecker;

pub type AnalysisResult<'tcx, T> = Result<T, Box<dyn AnalysisError + 'tcx>>;

use std::borrow::Cow;

pub trait AnalysisError: Error + ErrorCompat {
    fn kind(&self) -> AnalysisErrorKind;
    fn log(&self) {
        match self.kind() {
            AnalysisErrorKind::Unreachable => {
                error!("[{:?}] {}", self.kind(), self);
                if cfg!(feature = "backtraces") {
                    if let Some(backtrace) = ErrorCompat::backtrace(self) {
                        error!("Backtrace:\n{:?}", backtrace);
                    }
                }
            }
            AnalysisErrorKind::Unimplemented => {
                info!("[{:?}] {}", self.kind(), self);
                if cfg!(feature = "backtraces") {
                    if let Some(backtrace) = ErrorCompat::backtrace(self) {
                        info!("Backtrace:\n{:?}", backtrace);
                    }
                }
            }
            AnalysisErrorKind::OutOfScope => {
                debug!("[{:?}] {}", self.kind(), self);
                if cfg!(feature = "backtraces") {
                    if let Some(backtrace) = ErrorCompat::backtrace(self) {
                        debug!("Backtrace:\n{:?}", backtrace);
                    }
                }
            }
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum AnalysisErrorKind {
    /// An error that should never happen;
    /// If this happens, that means some of our assumption / invariant is broken.
    /// Normal programs would panic for it, but we want to avoid panic at all cost,
    /// so this error exists.
    Unreachable,
    /// A pattern that is not handled by our algorithm yet.
    Unimplemented,
    /// An expected failure, something like "we don't handle this by design",
    /// that worth recording.
    OutOfScope,
}

#[derive(Debug, Copy, Clone)]
pub enum AnalysisKind {
    UnsafeDestructor,
    SendSyncVariance(SendSyncAnalysisKind),
    UnsafeDataflow(UDBypassKind),
}

// TODO: Haven't decided on properly dividing up this category
#[derive(Debug, Copy, Clone)]
pub enum SendSyncAnalysisKind {
    // T: Send for Sync
    ConcurrentQueueSend,
    // T: Send for Send
    SendForSend,
    // T: Sync for Sync
    SyncForSync,
    // T: Send + Sync for Sync
    BothForSync
}

// Unsafe Dataflow BypassKind.
// Used to associate each Unsafe-Dataflow bug report with its cause.
#[derive(Debug, Copy, Clone)]
pub enum UDBypassKind {
    // Strong bypass
    ReadFlow,
    CopyFlow,
    VecFromRaw,
    // Weak bypass
    Transmute,
    WriteFlow,
    PtrAsRef,
    SliceUnchecked,
    SliceFromRaw,
}

impl Into<Cow<'static, str>> for AnalysisKind {
    fn into(self) -> Cow<'static, str> {
        match &self {
            AnalysisKind::UnsafeDestructor => "UnsafeDestructor",
            AnalysisKind::SendSyncVariance(svkind) => {
                use SendSyncAnalysisKind::*;
                "SendSyncVariance"
            },
            AnalysisKind::UnsafeDataflow(udkind) => {
                use UDBypassKind::*;
                match udkind {
                    ReadFlow => "UnsafeDataflow/ReadFlow",
                    CopyFlow => "UnsafeDataflow/CopyFlow",
                    VecFromRaw => "UnsafeDataflow/VecFromRaw",
                    Transmute => "UnsafeDataflow/Transmute",
                    WriteFlow => "UnsafeDataflow/WriteFlow",
                    PtrAsRef => "UnsafeDataflow/PtrAsRef",
                    SliceUnchecked => "UnsafeDataflow/SliceUnchecked",
                    SliceFromRaw => "UnsafeDataflow/SliceFromRaw",
                }
            },
        }
        .into()
    }
}
