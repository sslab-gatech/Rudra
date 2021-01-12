//! Unsafe Send/Sync impl detector

// You need to fix the code to enable `relaxed` mode..
mod relaxed;
// Default mode is `strict`.
mod strict;

use rustc_data_structures::fx::{FxHashMap, FxHashSet};
use rustc_hir::def_id::DefId;
use rustc_hir::{GenericBound, GenericParam, GenericParamKind, WherePredicate};
use rustc_hir::{HirId, ItemKind, Node};
use rustc_middle::ty::{
    self,
    subst::{self, GenericArgKind},
    AdtDef, GenericParamDefKind, List, PredicateAtom, TyCtxt,
};
use rustc_span::symbol::sym;

use snafu::{OptionExt, Snafu};

use crate::prelude::*;
use crate::report::{Report, ReportLevel};

pub use relaxed::*;
pub use strict::*;

pub struct SendSyncVarianceChecker<'tcx> {
    rcx: RudraCtxt<'tcx>,
    /// For each struct, keep track of reports.
    report_map: FxHashMap<DefId, Vec<Report>>,
    /// For each relevant ADT, keep track of `T`s that are only within `PhantomData<T>`.
    phantom_map: FxHashMap<DefId, Vec<u32>>,
}

impl<'tcx> SendSyncVarianceChecker<'tcx> {
    pub fn new(rcx: RudraCtxt<'tcx>) -> Self {
        SendSyncVarianceChecker {
            rcx,
            report_map: FxHashMap::default(),
            phantom_map: FxHashMap::default(),
        }
    }

    pub fn analyze(mut self) {
        let send_trait_did = unwrap_or!(send_trait_def_id(self.rcx.tcx()) => return);
        let sync_trait_did = unwrap_or!(sync_trait_def_id(self.rcx.tcx()) => return);
        let copy_trait_did = unwrap_or!(copy_trait_def_id(self.rcx.tcx()) => return);

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
                    .or_insert(Vec::with_capacity(2))
                    .push(Report::with_hir_id(
                        tcx,
                        ReportLevel::Warning,
                        "SendSyncVariance",
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
                    .or_insert(Vec::with_capacity(2))
                    .push(Report::with_hir_id(
                        tcx,
                        ReportLevel::Warning,
                        "SendSyncVariance",
                        "Suspicious impl of `Sync` found",
                        impl_hir_id,
                    ));
            }
        }
    }
}

/// For a given ADT (struct, enum, union),
/// return the indices of `T`s that are only inside `PhantomData<T>`.
fn phantom_indices<'tcx>(
    tcx: TyCtxt<'tcx>,
    adt_def: &AdtDef,
    substs: &'tcx List<subst::GenericArg<'tcx>>,
) -> Vec<u32> {
    // Store indices of gen_params that are in/out of `PhantomData<_>`
    let (mut in_phantom, mut out_phantom) = (FxHashSet::default(), FxHashSet::default());

    for variant in &adt_def.variants {
        for field in &variant.fields {
            let field_ty = field.ty(tcx, substs);

            let mut walker = field_ty.walk();
            while let Some(node) = walker.next() {
                if let GenericArgKind::Type(inner_ty) = node.unpack() {
                    if inner_ty.is_phantom_data() {
                        walker.skip_current_subtree();

                        for x in inner_ty.walk() {
                            if let GenericArgKind::Type(ph_ty) = x.unpack() {
                                if let ty::TyKind::Param(ty) = ph_ty.kind {
                                    in_phantom.insert(ty.index);
                                }
                            }
                        }
                        continue;
                    }

                    if let ty::TyKind::Param(ty) = inner_ty.kind {
                        out_phantom.insert(ty.index);
                    }
                }
            }
        }
    }

    // Check for params that are both inside & outside of `PhantomData<_>`
    let in_phantom = in_phantom
        .into_iter()
        .filter(|e| !out_phantom.contains(e))
        .collect();

    return in_phantom;
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

#[derive(Debug, Snafu)]
pub enum SendSyncVarianceError {
    CopyTraitNotFound,
    SendTraitNotFound,
    SyncTraitNotFound,
    CatchAll,
}

impl AnalysisError for SendSyncVarianceError {
    fn kind(&self) -> AnalysisErrorKind {
        use SendSyncVarianceError::*;
        match self {
            CopyTraitNotFound => AnalysisErrorKind::Unreachable,
            SendTraitNotFound => AnalysisErrorKind::Unreachable,
            SyncTraitNotFound => AnalysisErrorKind::Unreachable,
            CatchAll => AnalysisErrorKind::Unreachable,
        }
    }
}
