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
    AssocKind, GenericParamDef, GenericParamDefKind, List, PredicateAtom, Ty, TyCtxt, TyS,
};
use rustc_span::symbol::sym;

use snafu::{OptionExt, Snafu};

use crate::analysis::{AnalysisKind, StateToReportLevel};
use crate::prelude::*;
use crate::report::{Report, ReportLevel};

use behavior::*;
pub use phantom::*;
pub use relaxed::*;
pub use strict::*;
pub use utils::*;

pub struct SendSyncVarianceChecker<'tcx> {
    rcx: RudraCtxt<'tcx>,
    /// For each ADT, keep track of reports.
    report_map: FxHashMap<DefId, Vec<Report>>,
    /// For each ADT, keep track of `T`s that are only within `PhantomData<T>`.
    phantom_map: FxHashMap<DefId, Vec<u32>>,
    /// For each ADT, keep track of AdtBehavior per generic param.
    behavior_map: FxHashMap<DefId, FxHashMap<PostMapIdx, AdtBehavior>>,
}

impl<'tcx> SendSyncVarianceChecker<'tcx> {
    pub fn new(rcx: RudraCtxt<'tcx>) -> Self {
        SendSyncVarianceChecker {
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
            if let Some((adt_def_id, send_sync_analyses)) =
                self.suspicious_send(impl_hir_id, send_trait_did, sync_trait_did, copy_trait_did)
            {
                if send_sync_analyses.report_level() >= self.rcx.report_level() {
                    let tcx = self.rcx.tcx();
                    self.report_map
                        .entry(adt_def_id)
                        .or_insert_with(|| Vec::with_capacity(2))
                        .push(Report::with_hir_id(
                            tcx,
                            self.rcx.report_level(),
                            AnalysisKind::SendSyncVariance(send_sync_analyses),
                            "Suspicious impl of `Send` found",
                            impl_hir_id,
                        ));
                }
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
            if let Some((struct_def_id, send_sync_analyses)) =
                self.suspicious_sync(impl_hir_id, send_trait_did, sync_trait_did, copy_trait_did)
            {
                if send_sync_analyses.report_level() >= self.rcx.report_level() {
                    let tcx = self.rcx.tcx();
                    self.report_map
                        .entry(struct_def_id)
                        .or_insert_with(|| Vec::with_capacity(2))
                        .push(Report::with_hir_id(
                            tcx,
                            self.rcx.report_level(),
                            AnalysisKind::SendSyncVariance(send_sync_analyses),
                            "Suspicious impl of `Sync` found",
                            impl_hir_id,
                        ));
                }
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
pub enum SendSyncVarianceError {
    CloneTraitNotFound,
    CopyTraitNotFound,
    SendTraitNotFound,
    SyncTraitNotFound,
    CatchAll,
}

impl AnalysisError for SendSyncVarianceError {
    fn kind(&self) -> AnalysisErrorKind {
        use SendSyncVarianceError::*;
        match self {
            CloneTraitNotFound => AnalysisErrorKind::Unreachable,
            CopyTraitNotFound => AnalysisErrorKind::Unreachable,
            SendTraitNotFound => AnalysisErrorKind::Unreachable,
            SyncTraitNotFound => AnalysisErrorKind::Unreachable,
            CatchAll => AnalysisErrorKind::Unreachable,
        }
    }
}

// Index of generic type parameter within an impl block.
// Since the same generic parameter can have different indices in
// different impl blocks, we need to map these indices back to its
// original indices (`PostMapIdx`) to reason about generic parameters globally.
#[derive(Copy, Clone, Eq, Hash, PartialEq)]
pub struct PreMapIdx(u32);
// Index of generic type parameter in the ADT definition.
#[derive(Copy, Clone, Eq, Hash, PartialEq)]
pub struct PostMapIdx(u32);

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

impl StateToReportLevel for SendSyncAnalysisKind {
    fn report_level(&self) -> ReportLevel {
        let high = SendSyncAnalysisKind::API_SEND_FOR_SYNC | SendSyncAnalysisKind::RELAX_SEND;
        let med = SendSyncAnalysisKind::API_SYNC_FOR_SYNC
            | SendSyncAnalysisKind::PHANTOM_SEND_FOR_SEND
            | SendSyncAnalysisKind::RELAX_SYNC;

        if !(*self & high).is_empty() {
            ReportLevel::Error
        } else if !(*self & med).is_empty() {
            ReportLevel::Warning
        } else {
            ReportLevel::Info
        }
    }
}
