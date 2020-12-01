//! Unsafe Send/Sync impl detector

// You need to fix the code to enable `relaxed` mode..
mod relaxed;
// Default mode is `strict`.
mod strict;

use rustc_data_structures::fx::FxHashSet;
use rustc_hir::def::DefKind;
use rustc_hir::def_id::DefId;
use rustc_hir::QPath;
use rustc_hir::{GenericArg, GenericBound, GenericParam, GenericParamKind, WherePredicate};
use rustc_hir::{HirId, ItemKind, Node, TyKind, VariantData, StructField};
use rustc_hir::Ty;
use rustc_middle::hir::map::Map;
use rustc_middle::ty::{PredicateAtom, TyCtxt};
use rustc_span::symbol::sym;

use snafu::{OptionExt, Snafu};

use crate::algorithm::LocalTraitIter;
use crate::prelude::*;
use crate::report::{Report, ReportLevel};

pub use relaxed::*;
pub use strict::*;

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
                let copy_trait_def_id = unwrap_or!(copy_trait_def_id(self.rcx.tcx()) => return);

                for impl_item in LocalTraitIter::new(self.rcx, send_trait_def_id) {
                    if self.suspicious_send_strict(
                        impl_item,
                        send_trait_def_id,
                        sync_trait_def_id,
                        copy_trait_def_id
                    ) {
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
    fn analyze_sync(&self) {
        // key is DefId of trait, value is vec of HirId
        let sync_trait_def_id = unwrap_or!(sync_trait_def_id(self.rcx.tcx()) => return);

        for impl_item in LocalTraitIter::new(self.rcx, sync_trait_def_id) {
            if self.suspicious_sync_strict(impl_item, sync_trait_def_id) {
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

/// Check Copy Trait
fn copy_trait_def_id<'tcx>(tcx: TyCtxt<'tcx>) -> AnalysisResult<'tcx, DefId> {
    convert!(tcx.lang_items().copy_trait().context(CatchAll))
}

/// Check Sync Trait
fn sync_trait_def_id<'tcx>(tcx: TyCtxt<'tcx>) -> AnalysisResult<'tcx, DefId> {
    convert!(tcx.lang_items().sync_trait().context(CatchAll))
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
