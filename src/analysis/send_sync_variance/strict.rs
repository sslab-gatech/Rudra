//! Unsafe Send/Sync impl detector (strict)

use super::*;

impl<'tcx> SendSyncVarianceChecker<'tcx> {
    /// Returns Some(DefId of ADT) if `impl Sync` for the ADT looks suspicious
    /// (ADT: struct / enum / union)
    pub fn suspicious_sync(
        &mut self,
        impl_hir_id: HirId,
        _send_trait_def_id: DefId,
        sync_trait_def_id: DefId,
        _copy_trait_def_id: DefId,
    ) -> Option<DefId> {
        if let Some(trait_ref) = self.rcx.tcx().impl_trait_ref(impl_hir_id.owner) {
            if let ty::TyKind::Adt(adt_def, substs) = trait_ref.self_ty().kind {
                // At the end, this set should only contain indices of generic params
                // which may cause safety issues in the `Sync` impl.
                let mut suspicious_generic_params = FxHashSet::default();

                // Initialize set of generic type params of the given `impl Sync`.
                for gen_param in &self.rcx.tcx().generics_of(impl_hir_id.owner).params {
                    if let GenericParamDefKind::Type { .. } = gen_param.kind {
                        suspicious_generic_params.insert(gen_param.index);
                    }
                }

                // TODO: Check whether any of the generic params are okay with a (Send | Copy) bound.
                // e.g.  concurrent queue types that only have APIs that return T don't need T to be Sync.
                // -------------------------------------------------------------------

                let tcx = self.rcx.tcx();
                // Iterate over predicates to check trait bounds on generic params.
                for atom in tcx
                    .param_env(impl_hir_id.owner)
                    .caller_bounds()
                    .iter()
                    .map(|x| x.skip_binders())
                {
                    if let PredicateAtom::Trait(trait_predicate, _) = atom {
                        if let ty::TyKind::Param(param_ty) = trait_predicate.self_ty().kind {
                            // Find generic parameters that are in/out of `PhantomData<T>`.
                            // Check if query is cached before calling `phantom_indices`
                            if self
                                .phantom_map
                                .entry(adt_def.did)
                                .or_insert_with(|| phantom_indices(tcx, adt_def, substs))
                                .contains(&param_ty.index)
                            {
                                continue;
                            }

                            let trait_did = trait_predicate.def_id();
                            if trait_did == sync_trait_def_id {
                                suspicious_generic_params.remove(&param_ty.index);
                            }
                        }
                    }
                }

                return if suspicious_generic_params.is_empty() {
                    None
                } else {
                    Some(adt_def.did)
                };
            }
        }
        return None;
    }

    /// Returns Some(DefId of ADT) if `impl Send` for the ADT looks suspicious
    /// (ADT: struct / enum / union)
    pub fn suspicious_send(
        &mut self,
        impl_hir_id: HirId,
        send_trait_def_id: DefId,
        sync_trait_def_id: DefId,
        _copy_trait_def_id: DefId,
    ) -> Option<DefId> {
        if let Some(trait_ref) = self.rcx.tcx().impl_trait_ref(impl_hir_id.owner) {
            if let ty::TyKind::Adt(adt_def, substs) = trait_ref.self_ty().kind {
                // let adt_ty = trait_ref.self_ty();
                // info!("{:?}", adt_def.did);

                // At the end, this set should only contain indices of generic params
                // which may cause safety issues in the `Send` impl.
                let mut suspicious_generic_params = FxHashSet::default();

                // Initialize set of generic type params of the given `impl Send`.
                for gen_param in &self.rcx.tcx().generics_of(impl_hir_id.owner).params {
                    if let GenericParamDefKind::Type { .. } = gen_param.kind {
                        suspicious_generic_params.insert(gen_param.index);
                    }
                }

                // Find generic parameters that are in/out of `PhantomData<T>`.
                let phantom_indices = phantom_indices(self.rcx.tcx(), adt_def, substs);

                // TODO: Check whether any of the generic params require a Sync bound.
                // e.g. `Arc<T>` requires T: Send + Sync for Send
                // let mut params_need_sync= FxHashSet::default();
                // -------------------------------------------------------------------

                // Iterate over predicates to check trait bounds on generic params.
                for atom in self
                    .rcx
                    .tcx()
                    .param_env(impl_hir_id.owner)
                    .caller_bounds()
                    .iter()
                    .map(|x| x.skip_binders())
                {
                    if let PredicateAtom::Trait(trait_predicate, _) = atom {
                        if let ty::TyKind::Param(param_ty) = trait_predicate.self_ty().kind {
                            if phantom_indices.contains(&param_ty.index) {
                                continue;
                            }
                            let trait_did = trait_predicate.def_id();

                            if trait_did == send_trait_def_id || trait_did == sync_trait_def_id {
                                suspicious_generic_params.remove(&param_ty.index);
                            }
                        }
                    }
                }

                return if suspicious_generic_params.is_empty() {
                    None
                } else {
                    Some(adt_def.did)
                };
            }
        }
        return None;
    }
}
