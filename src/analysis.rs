mod send_sync_variance;
mod unsafe_dataflow;
mod unsafe_destructor;

use snafu::{Error, ErrorCompat};

use crate::report::ReportLevel;

pub use send_sync_variance::{SendSyncAnalysisKind, SendSyncVarianceChecker};
pub use unsafe_dataflow::{State, UnsafeDataflowChecker};
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
    UnsafeDataflow(State),
}

trait StateToReportLevel {
    fn report_level(&self) -> ReportLevel;
}

impl Into<Cow<'static, str>> for AnalysisKind {
    fn into(self) -> Cow<'static, str> {
        match &self {
            AnalysisKind::UnsafeDestructor => "UnsafeDestructor".into(),
            AnalysisKind::SendSyncVariance(sv_analyses) => {
                let mut v = vec!["SendSyncVariance:"];
                if sv_analyses.contains(SendSyncAnalysisKind::API_SEND_FOR_SYNC) {
                    v.push("ApiSendForSync")
                }
                if sv_analyses.contains(SendSyncAnalysisKind::API_SYNC_FOR_SYNC) {
                    v.push("ApiSyncforSync")
                }
                if sv_analyses.contains(SendSyncAnalysisKind::PHANTOM_SEND_FOR_SEND) {
                    v.push("PhantomSendForSend")
                }
                if sv_analyses.contains(SendSyncAnalysisKind::NAIVE_SEND_FOR_SEND) {
                    v.push("NaiveSendForSend")
                }
                if sv_analyses.contains(SendSyncAnalysisKind::NAIVE_SYNC_FOR_SYNC) {
                    v.push("NaiveSyncForSync")
                }
                if sv_analyses.contains(SendSyncAnalysisKind::RELAX_SEND) {
                    v.push("RelaxSend")
                }
                if sv_analyses.contains(SendSyncAnalysisKind::RELAX_SYNC) {
                    v.push("RelaxSync")
                }
                v.join("/").into()
            }
            AnalysisKind::UnsafeDataflow(bypass_kinds) => {
                let mut v = vec!["UnsafeDataflow:"];
                if bypass_kinds.contains(State::READ_FLOW) {
                    v.push("ReadFlow")
                }
                if bypass_kinds.contains(State::COPY_FLOW) {
                    v.push("CopyFlow")
                }
                if bypass_kinds.contains(State::VEC_FROM_RAW) {
                    v.push("VecFromRaw")
                }
                if bypass_kinds.contains(State::TRANSMUTE) {
                    v.push("Transmute")
                }
                if bypass_kinds.contains(State::WRITE_FLOW) {
                    v.push("WriteFlow")
                }
                if bypass_kinds.contains(State::PTR_AS_REF) {
                    v.push("PtrAsRef")
                }
                if bypass_kinds.contains(State::SLICE_UNCHECKED) {
                    v.push("SliceUnchecked")
                }
                if bypass_kinds.contains(State::SLICE_FROM_RAW) {
                    v.push("SliceFromRaw")
                }
                if bypass_kinds.contains(State::VEC_SET_LEN) {
                    v.push("VecSetLen")
                }
                v.join("/").into()
            }
        }
    }
}
