mod send_sync_variance;
mod unsafe_dataflow;
mod unsafe_destructor;

use snafu::{Error, ErrorCompat};

use crate::report::ReportLevel;

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
    UnsafeDataflow(State),
}

pub trait FilterStateByRank {
    fn filter_by_rank(&mut self, report_level: ReportLevel);
}

bitflags! {
    #[derive(Default)]
    pub struct SendSyncAnalysisKind: u8 {
        // T: Send for impl Sync (with api check & phantom check)
        const API_SEND_FOR_SYNC = 0b00000001;
        // T: Sync for impl Sync (with api check & phantom check)
        const API_SYNC_FOR_SYNC = 0b00000100;
        // T: Send for impl Send (with phantom check)
        const PHANTOM_SEND_FOR_SEND = 0b00000010;
        // T: Send for impl Send (no api check, no phantom check)
        const NAIVE_SEND_FOR_SEND = 0b00001000;
        // T: Sync for impl Sync (no api check, no phantom check)
        const NAIVE_SYNC_FOR_SYNC = 0b00010000;
        // Relaxed Send for impl Send (with phantom check)
        const RELAX_SEND = 0b00100000;
        // Relaxed Sync for impl Sync (with phantom check)
        const RELAX_SYNC = 0b01000000;
    }
}

impl FilterStateByRank for SendSyncAnalysisKind {
    fn filter_by_rank(&mut self, report_level: ReportLevel) {
        match report_level {
            ReportLevel::Error => {
                *self &= SendSyncAnalysisKind::API_SEND_FOR_SYNC | SendSyncAnalysisKind::RELAX_SEND
            }
            ReportLevel::Warning => {
                *self &= SendSyncAnalysisKind::API_SEND_FOR_SYNC
                    | SendSyncAnalysisKind::RELAX_SEND
                    | SendSyncAnalysisKind::API_SYNC_FOR_SYNC
                    | SendSyncAnalysisKind::PHANTOM_SEND_FOR_SEND
                    | SendSyncAnalysisKind::RELAX_SYNC
            }
            ReportLevel::Info => {}
        }
    }
}

// Unsafe Dataflow BypassKind.
// Used to associate each Unsafe-Dataflow bug report with its cause.
bitflags! {
    #[derive(Default)]
    pub struct State: u16 {
        const READ_FLOW = 0b00000001;
        const COPY_FLOW = 0b00000010;
        const VEC_FROM_RAW = 0b00000100;
        const TRANSMUTE = 0b00001000;
        const WRITE_FLOW = 0b00010000;
        const PTR_AS_REF = 0b00100000;
        const SLICE_UNCHECKED = 0b01000000;
        const SLICE_FROM_RAW = 0b10000000;
        const VEC_SET_LEN = 0b100000000;
    }
}

impl FilterStateByRank for State {
    fn filter_by_rank(&mut self, report_level: ReportLevel) {
        match report_level {
            ReportLevel::Error => *self &= State::VEC_FROM_RAW | State::VEC_SET_LEN,
            ReportLevel::Warning => {
                *self &= State::VEC_FROM_RAW
                    | State::VEC_SET_LEN
                    | State::READ_FLOW
                    | State::COPY_FLOW
                    | State::WRITE_FLOW
            }
            ReportLevel::Info => {}
        }
    }
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
