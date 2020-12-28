use rustc_hir::{def_id::DefId, BodyId};
use rustc_middle::ty::{Instance, ParamEnv};
use rustc_span::Span;

use snafu::{Backtrace, Snafu};
use termcolor::Color;

use crate::prelude::*;
use crate::{
    graph, ir, paths,
    report::{Report, ReportLevel},
    utils,
    visitor::ContainsUnsafe,
};

#[derive(Debug, Snafu)]
pub enum PanicSafetyError {
    PushPopBlock { backtrace: Backtrace },
    ResolveError { backtrace: Backtrace },
    InvalidSpan { backtrace: Backtrace },
}

impl AnalysisError for PanicSafetyError {
    fn kind(&self) -> AnalysisErrorKind {
        use PanicSafetyError::*;
        match self {
            PushPopBlock { .. } => AnalysisErrorKind::Unreachable,
            ResolveError { .. } => AnalysisErrorKind::OutOfScope,
            InvalidSpan { .. } => AnalysisErrorKind::Unreachable,
        }
    }
}

pub struct PanicSafetyAnalyzer<'tcx> {
    rcx: RudraCtxt<'tcx>,
}

impl<'tcx> PanicSafetyAnalyzer<'tcx> {
    pub fn new(rcx: RudraCtxt<'tcx>) -> Self {
        PanicSafetyAnalyzer { rcx }
    }

    pub fn analyze(self) {
        let tcx = self.rcx.tcx();
        let hir_map = tcx.hir();

        // Iterates all (type, related function) pairs
        for (_ty_hir_id, (body_id, related_item_span)) in self.rcx.types_with_related_items() {
            if let Some(status) = inner::PanicSafetyBodyAnalyzer::analyze_body(self.rcx, body_id) {
                if status.is_unsafe() {
                    let mut color_span = unwrap_or!(
                        utils::ColorSpan::new(tcx, related_item_span).context(InvalidSpan) => continue
                    );

                    for &span in status.strong_bypass_spans() {
                        color_span.add_sub_span(Color::Red, span);
                    }

                    for &span in status.weak_bypass_spans() {
                        color_span.add_sub_span(Color::Yellow, span);
                    }

                    for &span in status.panicking_function_spans() {
                        color_span.add_sub_span(Color::Cyan, span);
                    }

                    let level = if status.strong_bypass_spans().is_empty() {
                        ReportLevel::Info
                    } else {
                        ReportLevel::Warning
                    };

                    rudra_report(Report::with_color_span(
                        tcx,
                        level,
                        "PanicSafety",
                        format!(
                            "Potential panic safety issue in `{}`",
                            tcx.def_path_str(hir_map.body_owner_def_id(body_id).to_def_id())
                        ),
                        &color_span,
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
        strong_bypass: Vec<Span>,
        weak_bypass: Vec<Span>,
        panicking_function: Vec<Span>,
        is_unsafe: bool,
    }

    impl PanicSafetyStatus {
        pub fn is_unsafe(&self) -> bool {
            self.is_unsafe
        }

        pub fn strong_bypass_spans(&self) -> &Vec<Span> {
            &self.strong_bypass
        }

        pub fn weak_bypass_spans(&self) -> &Vec<Span> {
            &self.weak_bypass
        }

        pub fn panicking_function_spans(&self) -> &Vec<Span> {
            &self.panicking_function
        }
    }

    pub struct PanicSafetyBodyAnalyzer<'a, 'tcx> {
        rcx: RudraCtxt<'tcx>,
        body: &'a ir::Body<'tcx>,
        param_env: ParamEnv<'tcx>,
        status: PanicSafetyStatus,
    }

    impl<'a, 'tcx> PanicSafetyBodyAnalyzer<'a, 'tcx> {
        fn new(rcx: RudraCtxt<'tcx>, param_env: ParamEnv<'tcx>, body: &'a ir::Body<'tcx>) -> Self {
            PanicSafetyBodyAnalyzer {
                rcx,
                body,
                param_env,
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
                trace_calls_in_body(rcx, body_did);
                None
            } else if ContainsUnsafe::contains_unsafe(rcx.tcx(), body_id) {
                match rcx.translate_body(body_did).as_ref() {
                    Err(e) => {
                        // MIR is not available for def - log it and continue
                        e.log();
                        None
                    }
                    Ok(body) => {
                        let param_env = rcx.tcx().param_env(body_did);
                        let body_analyzer = PanicSafetyBodyAnalyzer::new(rcx, param_env, body);
                        Some(body_analyzer.analyze())
                    }
                }
            } else {
                // We don't perform interprocedural analysis,
                // thus safe functions are considered safe
                Some(Default::default())
            }
        }

        fn analyze(mut self) -> PanicSafetyStatus {
            let mut reachability = graph::Reachability::new(self.body);

            for (id, terminator) in self.body.terminators().enumerate() {
                match terminator.kind {
                    ir::TerminatorKind::StaticCall {
                        callee_did,
                        callee_substs,
                        ..
                    } => {
                        let ext = self.rcx.tcx().ext();

                        // Check for lifetime bypass
                        let symbol_vec = ext.get_def_path(callee_did);
                        if paths::STRONG_LIFETIME_BYPASS_LIST.contains(&symbol_vec) {
                            reachability.mark_source(id);
                            self.status
                                .strong_bypass
                                .push(terminator.original.source_info.span);
                        } else if paths::WEAK_LIFETIME_BYPASS_LIST.contains(&symbol_vec) {
                            reachability.mark_source(id);
                            self.status
                                .weak_bypass
                                .push(terminator.original.source_info.span);
                        } else if paths::GENERIC_FN_LIST.contains(&symbol_vec) {
                            reachability.mark_sink(id);
                            self.status
                                .panicking_function
                                .push(terminator.original.source_info.span);
                        } else {
                            // Check for generic function calls
                            match Instance::resolve(
                                self.rcx.tcx(),
                                self.param_env,
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
                                    reachability.mark_sink(id);
                                    self.status
                                        .panicking_function
                                        .push(terminator.original.source_info.span);
                                }
                            }
                        }
                    }
                    _ => (),
                }
            }

            self.status.is_unsafe = reachability.is_reachable();

            self.status
        }
    }

    fn trace_calls_in_body<'tcx>(rcx: RudraCtxt<'tcx>, body_def_id: DefId) {
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
