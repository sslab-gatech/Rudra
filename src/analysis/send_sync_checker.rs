//! Unsafe Send/Sync impl detector

use rustc_data_structures::fx::FxHashSet;
use rustc_hir::QPath;
use rustc_hir::def_id::DefId;
use rustc_hir::{GenericBound, WherePredicate, GenericParamKind};
use rustc_hir::{HirId, ItemKind, Node, TyKind};
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
        if_chain! {
            if let Some(send_trait_def_id) = self.rcx.tcx().get_diagnostic_item(sym::send_trait);
            if let Some(sync_trait_def_id) = self.rcx.tcx().get_diagnostic_item(sym::sync_trait);
            then {
                for impl_item in LocalTraitIter::new(self.rcx, send_trait_def_id) {
                    if find_suspicious_send(self.rcx, impl_item, send_trait_def_id, sync_trait_def_id) {
                        let tcx = self.rcx.tcx();
                        rudra_report(Report::with_span(
                            tcx,
                            ReportLevel::Warning,
                            "SendSyncChecker",
                            "Suspicious impl of `Send` found",
                            tcx.hir().span(impl_item),
                        ));
                    }
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
            if find_suspicious_sync(self.rcx, impl_item, sync_trait_def_id) {
                let tcx = self.rcx.tcx();
                rudra_report(Report::with_span(
                    tcx,
                    ReportLevel::Warning,
                    "SendSyncChecker",
                    "Suspicious impl of `Sync` found",
                    tcx.hir().span(impl_item),
                ));
            }
        }
    }
}

fn find_suspicious_send<'tcx>(
    rcx: RudraCtxt<'tcx>,
    hir_id: HirId,
    send_trait_def_id: DefId,
    sync_trait_def_id: DefId,
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
        if Some(send_trait_def_id) == trait_ref.trait_def_id();
        then {
            // If `impl Send` doesn't involve generic parameters, don't catch it.
            if generics.params.len() == 0 {
                return false;
            }

            // At the end, this set only contain `Symbol.as_u32()`s of generic params that don't implement `Send`
            let mut suspicious_generic_params = FxHashSet::default();

            // Inspect immediate trait bounds on generic parameters
            for generic_param in generics.params {
                if let GenericParamKind::Type { .. } = generic_param.kind {
                    let mut suspicious = true;
                    
                    // Check each immediate trait bound for generic_param to see if it implements `Sync`
                    for bound in generic_param.bounds {
                        if let GenericBound::Trait(x, ..) = bound {
                            if let Some(trait_def_id) = x.trait_ref.trait_def_id() {
                                if trait_def_id == send_trait_def_id || trait_def_id == sync_trait_def_id {
                                    suspicious = false;
                                    break;
                                }
                            }
                        }
                    }

                    if suspicious {
                        if let rustc_hir::ParamName::Plain(ident) = generic_param.name {
                            suspicious_generic_params.insert(ident.name.as_u32());
                        }
                    }
                }
            }

            // Inspect trait bounds in `where` clause
            for where_predicate in generics.where_clause.predicates {
                if_chain! {
                    if let WherePredicate::BoundPredicate(x) = where_predicate;
                    if let TyKind::Path(QPath::Resolved(_, path)) =  x.bounded_ty.kind;
                    if let rustc_hir::def::Res::Def(_, did) = path.res;
                    then {
                        let ident = rcx.tcx().item_name(did).as_u32();
                        for bound in x.bounds {
                            if let GenericBound::Trait(y, ..) = bound {
                                if let Some(trait_def_id) = y.trait_ref.trait_def_id() {
                                    if trait_def_id == send_trait_def_id || trait_def_id == sync_trait_def_id {
                                        suspicious_generic_params.remove(&ident);
                                    }
                                }
                            }
                        }
                    }
                }
            }

            return !suspicious_generic_params.is_empty();
        }
    }
    return false;
}

fn find_suspicious_sync<'tcx>(
    rcx: RudraCtxt<'tcx>,
    hir_id: HirId,
    sync_trait_def_id: DefId,
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
        if Some(sync_trait_def_id) == trait_ref.trait_def_id();
        then {
            // If `impl Sync` doesn't involve generic parameters, don't catch it.
            if generics.params.len() == 0 {
                return false;
            }

            // At the end, this set only contain `Symbol.as_u32()`s of generic params that don't implement `Sync`
            let mut suspicious_generic_params = FxHashSet::default();

            // Inspect immediate trait bounds on generic parameters
            for generic_param in generics.params {
                if let GenericParamKind::Type { .. } = generic_param.kind {
                    let mut suspicious = true;
                    
                    // Check each immediate trait bound for generic_param to see if it implements `Sync`
                    for bound in generic_param.bounds {
                        if let GenericBound::Trait(x, ..) = bound {
                            if Some(sync_trait_def_id) == x.trait_ref.trait_def_id() {
                                suspicious = false;
                                break;
                            }
                        }
                    }                    

                    if suspicious {
                        if let rustc_hir::ParamName::Plain(ident) = generic_param.name {
                            suspicious_generic_params.insert(ident.name.as_u32());
                        }
                    }
                }
            }

            // Inspect trait bounds in `where` clause
            for where_predicate in generics.where_clause.predicates {
                if_chain! {
                    if let WherePredicate::BoundPredicate(x) = where_predicate;
                    if let TyKind::Path(QPath::Resolved(_, path)) =  x.bounded_ty.kind;
                    if let rustc_hir::def::Res::Def(_, did) = path.res;
                    then {
                        let ident = rcx.tcx().item_name(did).as_u32();
                        for bound in x.bounds {
                            if let GenericBound::Trait(y, ..) = bound {
                                if Some(sync_trait_def_id) == y.trait_ref.trait_def_id() {
                                    suspicious_generic_params.remove(&ident);
                                }
                            }
                        }
                    }
                }
            }

            return !suspicious_generic_params.is_empty();
        }
    }
    return false;
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
