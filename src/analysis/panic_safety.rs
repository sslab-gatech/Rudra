use rustc_hir::BodyId;
use rustc_span::Span;

use snafu::{Backtrace, Snafu};

use crate::{
    prelude::*,
    report::{Report, ReportLevel},
    visitor::ContainsUnsafe,
};

#[derive(Debug, Snafu)]
pub enum PanicSafetyError {
    PushPopBlock { backtrace: Backtrace },
}

impl AnalysisError for PanicSafetyError {
    fn kind(&self) -> AnalysisErrorKind {
        use AnalysisErrorKind::*;
        use PanicSafetyError::*;
        match self {
            PushPopBlock { .. } => Unreachable,
        }
    }
}

pub struct PanicSafetyChecker<'tcx> {
    rcx: RudraCtxt<'tcx>,
}

impl<'tcx> PanicSafetyChecker<'tcx> {
    pub fn new(rcx: RudraCtxt<'tcx>) -> Self {
        PanicSafetyChecker { rcx }
    }

    /// The panic safety detector alpha version.
    /// It implements the name-based strategy with no reachability analysis.
    /// See 2020-12-08 meeting note for the detail.
    pub fn analyze(self) {
        // Iterates all (type, related function) pairs
        for (ty_hir_id, body_id) in self.rcx.types_with_related_items() {
            let panic_safety_status = inner::PanicSafetyVisitor::analyze_body(self.rcx, body_id);
            if panic_safety_status.is_unsafe() {
                let tcx = self.rcx.tcx();
                rudra_report(Report::with_hir_id(
                    tcx,
                    ReportLevel::Warning,
                    "PanicSafety",
                    format!(
                        "Potential panic safety issue in `{}`",
                        tcx.hir().name(ty_hir_id)
                    ),
                    body_id.hir_id,
                ))
            }
        }
    }
}

mod inner {
    use super::*;

    #[derive(Default)]
    pub struct PanicSafetyStatus {
        lifetime_bypass: Option<Span>,
        panicking_function: Option<Span>,
    }

    impl PanicSafetyStatus {
        pub fn is_unsafe(&self) -> bool {
            self.lifetime_bypass.is_some() && self.panicking_function.is_some()
        }
    }

    pub struct PanicSafetyVisitor<'tcx> {
        rcx: RudraCtxt<'tcx>,
    }

    impl<'tcx> PanicSafetyVisitor<'tcx> {
        pub fn analyze_body(rcx: RudraCtxt<'tcx>, body_id: BodyId) -> PanicSafetyStatus {
            let visitor = PanicSafetyVisitor { rcx };
            if ContainsUnsafe::contains_unsafe(rcx.tcx(), body_id) {
                // TODO: Perform MIR-based analysis here
            }

            todo!()
        }
    }
}
