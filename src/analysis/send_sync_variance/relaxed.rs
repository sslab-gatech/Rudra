//! Unsafe Send/Sync impl detector (relaxed)
#![allow(dead_code)]

use super::*;

// We may not use the relaxed versions at all,
// but keeping them alive just in case..
impl<'tcx> SendSyncVarianceChecker<'tcx> {
    /// Detect suspicious `Send` with relaxed rules.
    /// Report only if all generic parameters of `impl Send` aren't `Send`.
    fn suspicious_send_relaxed(
        &self,
        hir_id: HirId,
        send_trait_def_id: DefId,
        sync_trait_def_id: DefId,
    ) -> bool {
        let tcx = self.rcx.tcx();
        let node = tcx.hir_node(hir_id);
        if_chain! {
            if let Node::Item(item) = node;
            if let ItemKind::Impl(Impl {
                ref generics,
                of_trait: Some(ref trait_ref),
                ..
            }) = item.kind;
            if Some(send_trait_def_id) == trait_ref.trait_def_id();
            then {
                // If `impl Send` doesn't involve generic parameters, don't catch it.
                if generics.params.len() == 0 {
                    return false;
                }

                // Inspect immediate trait bounds on generic parameters
                if self.trait_in_imm_relaxed(
                    tcx,
                    generics,
                    &[send_trait_def_id, sync_trait_def_id],
                ) {
                    return false;
                }

                // Inspect trait bounds in where clauses
                return !self.trait_in_where_relaxed(
                    &[send_trait_def_id, sync_trait_def_id],
                    generics.predicates
                );
            }
        }
        return false;
    }

    /// Detect suspicious Sync with relaxed rules.
    /// Report only if all generic parameters of `impl Sync` aren't Sync.
    fn suspicious_sync_relaxed(
        &self,
        // HirId of the `Impl Sync` item
        hir_id: HirId,
        sync_trait_def_id: DefId,
    ) -> bool {
        let tcx = self.rcx.tcx();
        let node = tcx.hir_node(hir_id);
        if_chain! {
            if let Node::Item(item) = node;
            if let ItemKind::Impl(Impl {
                ref generics,
                of_trait: Some(ref trait_ref),
                ..
            }) = item.kind;
            if Some(sync_trait_def_id) == trait_ref.trait_def_id();
            then {
                // If `impl Sync` doesn't involve generic parameters, don't catch it.
                if generics.params.len() == 0 {
                    return false;
                }

                // Inspect immediate trait bounds on generic parameters
                if self.trait_in_imm_relaxed(
                    tcx,
                    generics,
                    &[sync_trait_def_id],
                ) {
                    return false;
                }

                return !self.trait_in_where_relaxed(
                    &[sync_trait_def_id],
                    generics.predicates
                );
            }
        }
        return false;
    }

    fn trait_in_imm_relaxed(
        &self,
        tcx: TyCtxt<'tcx>,
        generics: &rustc_hir::Generics<'_>,
        target_trait_def_ids: &[DefId],
    ) -> bool {
        for generic_param in generics.params {
            if let GenericParamKind::Type { .. } = generic_param.kind {
                // Use bounds_for_param to get bounds for this parameter
                let param_def_id = generic_param.def_id;
                for bound_pred in generics.bounds_for_param(param_def_id) {
                    for bound in bound_pred.bounds {
                        if let GenericBound::Trait(poly_trait_ref) = bound {
                            if let Some(def_id) = poly_trait_ref.trait_ref.trait_def_id() {
                                if target_trait_def_ids.contains(&def_id) {
                                    return true;
                                }

                                // Check super-traits
                                for (clause, _span) in tcx.explicit_super_predicates_of(def_id).skip_binder() {
                                    if let ty::ClauseKind::Trait(x) = clause.kind().skip_binder() {
                                        if target_trait_def_ids.contains(&x.trait_ref.def_id) {
                                            return true;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        return false;
    }

    fn trait_in_where_relaxed(
        &self,
        target_trait_def_ids: &[DefId],
        where_predicates: &[WherePredicate<'_>],
    ) -> bool {
        let tcx = self.rcx.tcx();
        for where_predicate in where_predicates {
            if let WherePredicateKind::BoundPredicate(x) = where_predicate.kind {
                for bound in x.bounds {
                    if let GenericBound::Trait(poly_trait_ref) = bound {
                        if let Some(def_id) = poly_trait_ref.trait_ref.trait_def_id() {
                            if target_trait_def_ids.contains(&def_id) {
                                return true;
                            }

                            for (clause, _span) in tcx.explicit_super_predicates_of(def_id).skip_binder() {
                                if let ty::ClauseKind::Trait(z) = clause.kind().skip_binder() {
                                    if target_trait_def_ids.contains(&z.trait_ref.def_id) {
                                        return true;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        return false;
    }
}
