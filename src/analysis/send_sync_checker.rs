//! Unsafe Send/Sync impl detector
use rustc_hir::def_id::DefId;
use rustc_hir::{GenericBound, WherePredicate};
use rustc_hir::{HirId, ItemKind, Node};
use rustc_middle::ty::TyCtxt;
use rustc_span::symbol::sym;

use snafu::{OptionExt, Snafu};

use crate::algorithm::LocalTraitIter;
use crate::prelude::*;
use crate::report::{Report, ReportLevel};

pub struct SendSyncChecker<'tcx> {
    rcx: RudraCtxt<'tcx>,
}

impl<'tcx> SendSyncChecker<'tcx> {
    pub fn new(rcx: RudraCtxt<'tcx>) -> Self {
        SendSyncChecker { rcx }
    }

    pub fn analyze(&mut self) {
        self.analyze_send();
        self.analyze_sync();
    }

    /// Detect cases where the wrapper of T implements `Send`, but T may not be `Send`
    fn analyze_send(&mut self) {
        if let Some(send_trait_def_id) = self.rcx.tcx().get_diagnostic_item(sym::send_trait) {
            for impl_item in LocalTraitIter::new(self.rcx, send_trait_def_id) {
                if find_suspicious_send_or_sync(self.rcx, impl_item, send_trait_def_id) {
                    let tcx = self.rcx.tcx();
                    rudra_report(Report::with_span(
                        tcx,
                        ReportLevel::Warning,
                        "SendSyncChecker",
                        "wrapper of P implements `Send`, while P may not implement `Send`",
                        tcx.hir().span(impl_item),
                    ));
                }
            }
        }
    }

    /// Detect cases where the wrapper of T implements `Sync`, but T may not be `Sync`
    fn analyze_sync(&mut self) {
        // Check Sync Trait
        fn sync_trait_def_id<'tcx>(tcx: TyCtxt<'tcx>) -> AnalysisResult<'tcx, DefId> {
            convert!(tcx.lang_items().sync_trait().context(CatchAll))
        }
        // key is DefId of trait, value is vec of HirId
        let sync_trait_def_id = unwrap_or!(sync_trait_def_id(self.rcx.tcx()) => return);

        for impl_item in LocalTraitIter::new(self.rcx, sync_trait_def_id) {
            if find_suspicious_send_or_sync(self.rcx, impl_item, sync_trait_def_id) {
                let tcx = self.rcx.tcx();
                rudra_report(Report::with_span(
                    tcx,
                    ReportLevel::Warning,
                    "SendSyncChecker",
                    "wrapper of P implements `Sync`, while P may not implement `Sync`",
                    tcx.hir().span(impl_item),
                ));
            }
        }
    }
}

fn find_suspicious_send_or_sync<'tcx>(
    rcx: RudraCtxt<'tcx>,
    hir_id: HirId,
    send_or_sync_trait_def_id: DefId,
) -> bool {
    let map = rcx.tcx().hir();
    if_chain! {
        if let Some(node) = map.find(hir_id);
        if let Node::Item(item) = node;
        if let ItemKind::Impl {
            ref generics,
            of_trait: Some(ref trait_ref),
            ..
        } = item.kind;
        if Some(send_or_sync_trait_def_id) == trait_ref.trait_def_id();
        then {
            // Inspect immediate trait bounds on generic parameters
            for generic_param in generics.params {
                for bound in generic_param.bounds {
                    if_chain! {
                        if let GenericBound::Trait(x, ..) = bound;
                        if let Some(cur_trait_def_id) = x.trait_ref.trait_def_id();
                        if cur_trait_def_id == send_or_sync_trait_def_id;
                        then {
                            return false;
                        }
                    }
                }
            }

            // Inspect trait bounds in `where` clause
            for where_predicate in generics.where_clause.predicates {
                if let WherePredicate::BoundPredicate(x) = where_predicate {
                    for bound in x.bounds {
                        if_chain! {
                            if let GenericBound::Trait(x, ..) = bound;
                            if let Some(cur_trait_def_id) = x.trait_ref.trait_def_id();
                            if cur_trait_def_id == send_or_sync_trait_def_id;
                            then {
                                return false;
                            }
                        }
                    }
                }
            }
        }
    }
    return true;
}

#[derive(Debug, Snafu)]
pub enum SendSyncError {
    CatchAll,
}

impl AnalysisError for SendSyncError {
    fn kind(&self) -> AnalysisErrorKind {
        use AnalysisErrorKind::*;
        use SendSyncError::*;
        match self {
            CatchAll => Unreachable,
        }
    }
}
