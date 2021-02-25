//! Unsafe Send/Sync impl detector

mod behavior;
mod phantom;
// You need to fix the code to enable `relaxed` mode..
mod relaxed;
// Default mode is `strict`.
mod strict;
mod utils;

use rustc_data_structures::fx::{FxHashMap, FxHashSet};
use rustc_hir::def_id::DefId;
use rustc_hir::{GenericBound, GenericParam, GenericParamKind, WherePredicate};
use rustc_hir::{HirId, ItemKind, Node};
use rustc_middle::mir::terminator::Mutability;
use rustc_middle::ty::{
    self,
    subst::{self, GenericArgKind},
    AssocKind, FnSig, GenericParamDef, GenericParamDefKind, List, PredicateAtom, Ty, TyCtxt, TyS,
};
use rustc_span::symbol::sym;

use snafu::{OptionExt, Snafu};

use crate::prelude::*;
use crate::report::{Report, ReportLevel};

use behavior::*;
pub use phantom::*;
pub use relaxed::*;
pub use strict::*;
pub use utils::*;

pub struct SendSyncChecker<'tcx> {
    rcx: RudraCtxt<'tcx>,
    /// For each ADT, keep track of reports.
    report_map: FxHashMap<DefId, Vec<Report>>,
    /// For each ADT, keep track of `T`s that are only within `PhantomData<T>`.
    phantom_map: FxHashMap<DefId, Vec<u32>>,
    /// For each ADT, keep track of AdtBehavior per generic param.
    behavior_map: FxHashMap<DefId, FxHashMap<u32, AdtBehavior>>,
}

impl<'tcx> SendSyncChecker<'tcx> {
    pub fn new(rcx: RudraCtxt<'tcx>) -> Self {
        SendSyncChecker {
            rcx,
            report_map: FxHashMap::default(),
            phantom_map: FxHashMap::default(),
            behavior_map: FxHashMap::default(),
        }
    }

    pub fn analyze(mut self) {
        let send_trait_did = unwrap_or!(send_trait_def_id(self.rcx.tcx()) => return);
        let sync_trait_did = unwrap_or!(sync_trait_def_id(self.rcx.tcx()) => return);
        let copy_trait_did = unwrap_or!(copy_trait_def_id(self.rcx.tcx()) => return);

        // Main analysis
        self.analyze_send(send_trait_did, sync_trait_did, copy_trait_did);
        self.analyze_sync(send_trait_did, sync_trait_did, copy_trait_did);

        // Report any suspicious `Send`/`Sync` impls on the given struct.
        for (_struct_def_id, reports) in self.report_map.into_iter() {
            for report in reports.into_iter() {
                rudra_report(report);
            }
        }
    }

    /// Detect cases where the wrapper of T implements `Send`, but T may not be `Send`
    fn analyze_send(
        &mut self,
        send_trait_did: DefId,
        sync_trait_did: DefId,
        copy_trait_did: DefId,
    ) {
        // Iterate over `impl`s that implement `Send`.
        for &impl_hir_id in self.rcx.tcx().hir().trait_impls(send_trait_did) {
            if let Some(adt_def_id) =
                self.suspicious_send(impl_hir_id, send_trait_did, sync_trait_did, copy_trait_did)
            {
                let tcx = self.rcx.tcx();
                self.report_map
                    .entry(adt_def_id)
                    .or_insert_with(|| Vec::with_capacity(2))
                    .push(Report::with_hir_id(
                        tcx,
                        ReportLevel::Warning,
                        "SendSyncChecker",
                        "Suspicious impl of `Send` found",
                        impl_hir_id,
                    ));
            }
        }
    }

    /// Detect cases where the wrapper of T implements `Sync`, but T may not be `Sync`
    fn analyze_sync(
        &mut self,
        // report_map: &mut FxHashMap<DefId, Vec<Report>>,
        send_trait_did: DefId,
        sync_trait_did: DefId,
        copy_trait_did: DefId,
    ) {
        // Iterate over `impl`s that implement `Sync`.
        for &impl_hir_id in self.rcx.tcx().hir().trait_impls(sync_trait_did) {
            if let Some(struct_def_id) =
                self.suspicious_sync(impl_hir_id, send_trait_did, sync_trait_did, copy_trait_did)
            {
                let tcx = self.rcx.tcx();
                self.report_map
                    .entry(struct_def_id)
                    .or_insert_with(|| Vec::with_capacity(2))
                    .push(Report::with_hir_id(
                        tcx,
                        ReportLevel::Warning,
                        "SendSyncChecker",
                        "Suspicious impl of `Sync` found",
                        impl_hir_id,
                    ));
            }
        }
    }
}

/// Check Send Trait
fn send_trait_def_id<'tcx>(tcx: TyCtxt<'tcx>) -> AnalysisResult<'tcx, DefId> {
    convert!(tcx
        .get_diagnostic_item(sym::send_trait)
        .context(SendTraitNotFound))
}

/// Check Sync Trait
fn sync_trait_def_id<'tcx>(tcx: TyCtxt<'tcx>) -> AnalysisResult<'tcx, DefId> {
    convert!(tcx.lang_items().sync_trait().context(SyncTraitNotFound))
}

/// Check Copy Trait
fn copy_trait_def_id<'tcx>(tcx: TyCtxt<'tcx>) -> AnalysisResult<'tcx, DefId> {
    convert!(tcx.lang_items().copy_trait().context(CopyTraitNotFound))
}

/// Check Clone Trait
fn _clone_trait_def_id<'tcx>(tcx: TyCtxt<'tcx>) -> AnalysisResult<'tcx, DefId> {
    convert!(tcx.lang_items().clone_trait().context(CloneTraitNotFound))
}

#[derive(Debug, Snafu)]
pub enum SendSyncError {
    CloneTraitNotFound,
    CopyTraitNotFound,
    SendTraitNotFound,
    SyncTraitNotFound,
    CatchAll,
}

impl AnalysisError for SendSyncError {
    fn kind(&self) -> AnalysisErrorKind {
        use SendSyncError::*;
        match self {
            CloneTraitNotFound => AnalysisErrorKind::Unreachable,
            CopyTraitNotFound => AnalysisErrorKind::Unreachable,
            SendTraitNotFound => AnalysisErrorKind::Unreachable,
            SyncTraitNotFound => AnalysisErrorKind::Unreachable,
            CatchAll => AnalysisErrorKind::Unreachable,
        }
    }
}
