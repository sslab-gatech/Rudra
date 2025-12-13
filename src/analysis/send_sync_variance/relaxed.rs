//! Unsafe Send/Sync impl detector (relaxed)
//! Note: This module is currently disabled and needs updating for newer rustc versions.
#![allow(dead_code)]
#![allow(unused_imports)]

use super::*;

// The relaxed analysis mode has been temporarily disabled due to significant
// rustc API changes. It can be re-enabled and updated if needed.
impl<'tcx> SendSyncVarianceChecker<'tcx> {
    /// Detect suspicious `Send` with relaxed rules.
    /// Report only if all generic parameters of `impl Send` aren't `Send`.
    fn suspicious_send_relaxed(
        &self,
        _hir_id: HirId,
        _send_trait_def_id: DefId,
        _sync_trait_def_id: DefId,
    ) -> bool {
        // Relaxed mode is currently disabled
        false
    }

    /// Detect suspicious Sync with relaxed rules.
    /// Report only if all generic parameters of `impl Sync` aren't Sync.
    fn suspicious_sync_relaxed(
        &self,
        _hir_id: HirId,
        _sync_trait_def_id: DefId,
    ) -> bool {
        // Relaxed mode is currently disabled
        false
    }

    fn trait_in_imm_relaxed(
        &self,
        _target_trait_def_ids: &[DefId],
        _generic_params: &[GenericParam],
    ) -> bool {
        false
    }

    fn trait_in_where_relaxed(
        &self,
        _target_trait_def_ids: &[DefId],
        _where_predicates: &[WherePredicate],
    ) -> bool {
        false
    }
}
