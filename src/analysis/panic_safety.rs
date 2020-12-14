use rustc_hir::{def_id::DefId, BodyId};
use rustc_span::Span;

use snafu::{Backtrace, Snafu};

use crate::prelude::*;
use crate::{
    ir, paths,
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

    pub fn analyze(self) {
        // Iterates all (type, related function) pairs
        for (ty_hir_id, body_id) in self.rcx.types_with_related_items() {
            if let Some(panic_safety_status) =
                inner::PanicSafetyVisitor::analyze_body(self.rcx, body_id)
            {
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
        status: PanicSafetyStatus,
    }

    impl<'tcx> PanicSafetyVisitor<'tcx> {
        fn new(rcx: RudraCtxt<'tcx>) -> Self {
            PanicSafetyVisitor {
                rcx,
                status: Default::default(),
            }
        }

        pub fn analyze_body(rcx: RudraCtxt<'tcx>, body_id: BodyId) -> Option<PanicSafetyStatus> {
            let hir_map = rcx.tcx().hir();
            let body_did = hir_map.body_owner_def_id(body_id).to_def_id();

            if rcx.tcx().ext().match_def_path(
                body_did,
                &["rudra_paths_discovery", "PathsDiscovery", "discover"],
            ) {
                // Special case for paths discovery
                trace_body(rcx, body_did);
                None
            } else if ContainsUnsafe::contains_unsafe(rcx.tcx(), body_id) {
                match rcx.translate_body(body_did).as_ref() {
                    Err(e) => {
                        // MIR is not available for def - log it and continue
                        e.log();
                        None
                    }
                    Ok(body) => {
                        let mut visitor = PanicSafetyVisitor::new(rcx);
                        visitor.analyze_body_impl(body);
                        Some(visitor.status)
                    }
                }
            } else {
                // We don't perform interprocedural analysis,
                // thus safe functions are considered safe
                Some(Default::default())
            }
        }

        fn analyze_body_impl(&mut self, body: &ir::Body<'tcx>) {
            // The panic safety detector alpha version.
            // It implements the name-based strategy with no reachability analysis.
            // See 2020-12-08 meeting note for the detail.
            for terminator in body.terminators() {
                match terminator.kind {
                    ir::TerminatorKind::StaticCall { callee_did, .. } => {
                        let ext = self.rcx.tcx().ext();

                        // Check for lifetime bypass
                        static LIFETIME_BYPASS_LIST: [&[&str]; 8] = [
                            &paths::PTR_READ,
                            &paths::PTR_WRITE,
                            &paths::PTR_SLICE_FROM_RAW_PARTS,
                            &paths::PTR_SLICE_FROM_RAW_PARTS_MUT,
                            &paths::SLICE_FROM_RAW_PARTS,
                            &paths::SLICE_FROM_RAW_PARTS_MUT,
                            &paths::INTRINSICS_COPY,
                            &paths::INTRINSICS_COPY_NONOVERLAPPING,
                        ];

                        for path in &LIFETIME_BYPASS_LIST {
                            if ext.match_def_path(callee_did, path) {
                                self.status.lifetime_bypass =
                                    Some(terminator.original.source_info.span);
                            }
                        }

                        // TODO: Check for generic function calls
                    }
                    _ => (),
                }
            }
        }
    }

    fn trace_body<'tcx>(rcx: RudraCtxt<'tcx>, body_def_id: DefId) {
        warn!("Paths discovery function has been detected");
        if let Ok(body) = rcx.translate_body(body_def_id).as_ref() {
            for terminator in body.terminators() {
                match terminator.kind {
                    ir::TerminatorKind::StaticCall { callee_did, .. } => {
                        let ext = rcx.tcx().ext();
                        println!(
                            "{}",
                            ext.get_def_path(callee_did)
                                .iter()
                                .fold(String::new(), |a, b| a + " :: " + &*b.as_str())
                        );
                    }
                    _ => (),
                }
            }
        }
    }
}
