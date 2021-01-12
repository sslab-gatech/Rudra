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
        let map = self.rcx.tcx().hir();
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

                // Inspect immediate trait bounds on generic parameters
                if self.trait_in_imm_relaxed(
                    &[send_trait_def_id, sync_trait_def_id],
                    generics.params
                ) {
                    return false;
                }

                // Inspect trait bounds in where clauses
                return !self.trait_in_where_relaxed(
                    &[send_trait_def_id, sync_trait_def_id],
                    generics.where_clause.predicates
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
        let map = self.rcx.tcx().hir();
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

                // Inspect immediate trait bounds on generic parameters
                if self.trait_in_imm_relaxed(
                   &[sync_trait_def_id],
                   generics.params
                ) {
                   return false;
                }

                return !self.trait_in_where_relaxed(
                    &[sync_trait_def_id],
                    generics.where_clause.predicates
                );
            }
        }
        return false;
    }

    fn trait_in_imm_relaxed(
        &self,
        target_trait_def_ids: &[DefId],
        generic_params: &[GenericParam],
    ) -> bool {
        for generic_param in generic_params {
            if let GenericParamKind::Type { .. } = generic_param.kind {
                for bound in generic_param.bounds {
                    if let GenericBound::Trait(x, ..) = bound {
                        if let Some(def_id) = x.trait_ref.trait_def_id() {
                            if target_trait_def_ids.contains(&def_id) {
                                return true;
                            }

                            // Check super-traits
                            for p in self.rcx.tcx().super_predicates_of(def_id).predicates {
                                if let PredicateAtom::Trait(x, _) = p.0.skip_binders() {
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
        return false;
    }

    fn trait_in_where_relaxed(
        &self,
        target_trait_def_ids: &[DefId],
        where_predicates: &[WherePredicate],
    ) -> bool {
        for where_predicate in where_predicates {
            if let WherePredicate::BoundPredicate(x) = where_predicate {
                for bound in x.bounds {
                    if let GenericBound::Trait(y, ..) = bound {
                        if let Some(def_id) = y.trait_ref.trait_def_id() {
                            if target_trait_def_ids.contains(&def_id) {
                                return true;
                            }

                            for p in self.rcx.tcx().super_predicates_of(def_id).predicates {
                                if let PredicateAtom::Trait(z, _) = p.0.skip_binders() {
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
