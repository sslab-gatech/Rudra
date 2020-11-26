//! Unsafe Send/Sync impl detector (strict)

use super::*;

impl<'tcx> SendSyncChecker<'tcx> {
    /// Detect suspicious Sync with strict rules.
    /// Report if any of the generic parameters of `impl Sync` aren't Sync.
    pub fn suspicious_sync_strict(
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

                // At the end, this set contains `Symbol.as_u32()`s of generic params that aren't `Sync`
                let mut suspicious_generic_params = FxHashSet::default();

                // Inspect immediate trait bounds on generic parameters
                self.initialize_suspects(
                    &[sync_trait_def_id],
                    generics.params,
                    &mut suspicious_generic_params,
                );

                self.filter_suspects(
                    &[sync_trait_def_id],
                    generics.where_clause.predicates,
                    &mut suspicious_generic_params,
                );

                return !suspicious_generic_params.is_empty();
            }
        }
        return false;
    }

    pub fn suspicious_send_strict(
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

                // At the end, this set only contain `Symbol.as_u32()`s of generic params that don't implement `Send`
                let mut suspicious_generic_params = FxHashSet::default();

                // Inspect immediate trait bounds on generic parameters
                // to initialize set of suspects that may not be `Send`
                self.initialize_suspects(
                    &[send_trait_def_id, sync_trait_def_id],
                    generics.params,
                    &mut suspicious_generic_params
                );

                // Inspect trait bounds in `where` clause.
                // Filter out suspects that have `Send` bound in where clause.
                self.filter_suspects(
                    &[send_trait_def_id, sync_trait_def_id],
                    generics.where_clause.predicates,
                    &mut suspicious_generic_params
                );

                return !suspicious_generic_params.is_empty();
            }
        }
        return false;
    }

    /// To `suspicious_generic_params`,
    /// insert generic parameters that don't have a bound included in `target_trait_def_ids`
    fn initialize_suspects(
        &self,
        target_trait_def_ids: &[DefId],
        generic_params: &[GenericParam],
        suspicious_generic_params: &mut FxHashSet<u32>,
    ) {
        // Inspect immediate trait bounds on generic parameters
        for generic_param in generic_params {
            if let GenericParamKind::Type { .. } = generic_param.kind {
                let mut suspicious = true;

                'outer: for bound in generic_param.bounds {
                    if let GenericBound::Trait(x, ..) = bound {
                        if let Some(def_id) = x.trait_ref.trait_def_id() {
                            if target_trait_def_ids.contains(&def_id) {
                                suspicious = false;
                                break;
                            }

                            // Check super-traits.
                            for p in self.rcx.tcx().super_predicates_of(def_id).predicates {
                                if let PredicateAtom::Trait(z, _) = p.0.skip_binders() {
                                    if target_trait_def_ids.contains(&z.trait_ref.def_id) {
                                        suspicious = false;
                                        break 'outer;
                                    }
                                }
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
    }

    /// From `suspicious_generic_params`,
    /// remove generic parameters that have a `Sync` bound in `where_predicates`.
    fn filter_suspects(
        &self,
        target_trait_def_ids: &[DefId],
        where_predicates: &[WherePredicate],
        suspicious_generic_params: &mut FxHashSet<u32>,
    ) {
        for where_predicate in where_predicates {
            if_chain! {
                if let WherePredicate::BoundPredicate(x) = where_predicate;
                if let TyKind::Path(QPath::Resolved(_, path)) =  x.bounded_ty.kind;
                if let rustc_hir::def::Res::Def(_, did) = path.res;
                then {
                    let ident = self.rcx.tcx().item_name(did).as_u32();
                    for bound in x.bounds {
                        if let GenericBound::Trait(y, ..) = bound {
                            if let Some(def_id) = y.trait_ref.trait_def_id() {
                                if target_trait_def_ids.contains(&def_id) {
                                    suspicious_generic_params.remove(&ident);
                                    continue;
                                }

                                // Check super-traits.
                                for p in self.rcx.tcx().super_predicates_of(def_id).predicates {
                                    if let PredicateAtom::Trait(z, _) = p.0.skip_binders() {
                                        if target_trait_def_ids.contains(&z.trait_ref.def_id) {
                                            suspicious_generic_params.remove(&ident);
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
