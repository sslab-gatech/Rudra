use rustc_hir::{def_id::DefId, BodyId};
use rustc_middle::ty::{Instance, ParamEnv};
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
    ResolveError { backtrace: Backtrace },
}

impl AnalysisError for PanicSafetyError {
    fn kind(&self) -> AnalysisErrorKind {
        use PanicSafetyError::*;
        match self {
            PushPopBlock { .. } => AnalysisErrorKind::Unreachable,
            ResolveError { .. } => AnalysisErrorKind::OutOfScope,
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
        let tcx = self.rcx.tcx();
        let hir_map = tcx.hir();

        // Iterates all (type, related function) pairs
        for (_ty_hir_id, body_id) in self.rcx.types_with_related_items() {
            if let Some(panic_safety_status) =
                inner::PanicSafetyVisitor::analyze_body(self.rcx, body_id)
            {
                if panic_safety_status.is_unsafe() {
                    rudra_report(Report::with_hir_id(
                        tcx,
                        ReportLevel::Warning,
                        "PanicSafety",
                        format!(
                            "Potential panic safety issue in `{}`",
                            tcx.def_path_str(hir_map.body_owner_def_id(body_id).to_def_id())
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

    #[derive(Debug, Default)]
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
                        let param_env = rcx.tcx().param_env(body_did);
                        visitor.analyze_body_impl(param_env, body);
                        Some(visitor.status)
                    }
                }
            } else {
                // We don't perform interprocedural analysis,
                // thus safe functions are considered safe
                Some(Default::default())
            }
        }

        fn analyze_body_impl(&mut self, param_env: ParamEnv<'tcx>, body: &ir::Body<'tcx>) {
            // The panic safety detector alpha version.
            // It implements the name-based strategy with no reachability analysis.
            // See 2020-12-08 meeting note for the detail.
            for terminator in body.terminators() {
                match terminator.kind {
                    ir::TerminatorKind::StaticCall {
                        callee_did,
                        callee_substs,
                        ..
                    } => {
                        let ext = self.rcx.tcx().ext();

                        // Check for lifetime bypass
                        static LIFETIME_BYPASS_LIST: [&[&str]; 7] = [
                            &paths::PTR_READ,
                            &paths::PTR_WRITE,
                            &paths::PTR_DIRECT_WRITE,
                            &paths::INTRINSICS_COPY,
                            &paths::INTRINSICS_COPY_NONOVERLAPPING,
                            &paths::VEC_SET_LEN,
                            &paths::VEC_FROM_RAW_PARTS,
                        ];

                        for path in &LIFETIME_BYPASS_LIST {
                            if ext.match_def_path(callee_did, path) {
                                self.status.lifetime_bypass =
                                    Some(terminator.original.source_info.span);
                            }
                        }

                        // Check for generic function calls
                        match Instance::resolve(
                            self.rcx.tcx(),
                            param_env,
                            callee_did,
                            callee_substs,
                        ) {
                            Err(_e) => log_err!(ResolveError),
                            Ok(Some(_)) => {
                                // Calls were successfully resolved
                            }
                            Ok(None) => {
                                // Call contains unresolvable generic parts
                                // Here, we are making a two step approximation:
                                // 1. Unresolvable generic code is potentially user-provided
                                // 2. User-provided code potentially panics
                                self.status.panicking_function =
                                    Some(terminator.original.source_info.span);
                            }
                        }
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
