//! Unsafe Send/Sync impl detector (strict)

use super::*;

impl<'tcx> SendSyncChecker<'tcx> {
    /// Returns Some(DefId of ADT) if `impl Sync` for the ADT looks suspicious
    /// (ADT: struct / enum / union)
    pub fn suspicious_sync(
        &mut self,
        impl_hir_id: HirId,
        send_trait_def_id: DefId,
        sync_trait_def_id: DefId,
        _copy_trait_def_id: DefId,
    ) -> Option<DefId> {
        let tcx = self.rcx.tcx();
        if let Some(trait_ref) = tcx.impl_trait_ref(impl_hir_id.owner) {
            if let ty::TyKind::Adt(adt_def, substs) = trait_ref.self_ty().kind {
                let adt_did = adt_def.did;

                // Keep track of generic params that need to be `Send + Sync`.
                // e.g. `Arc<T>` requires T: Send + Sync for Send
                let mut need_sync: FxHashSet<u32> = FxHashSet::default();

                // Keep track of generic params that need to be `Send`.
                let mut need_send: FxHashSet<u32> = FxHashSet::default();

                // Generic params that only occur within `PhantomData<_>`
                let phantom_params = self
                    .phantom_map
                    .entry(adt_did)
                    .or_insert_with(|| phantom_indices(tcx, adt_def, substs));

                // Get `AdtBehavior` per generic parameter.
                let adt_behavior = self
                    .behavior_map
                    .entry(adt_did)
                    .or_insert(adt_behavior(tcx, adt_did));

                // Initialize set of generic type params of the given `impl Sync`.
                for gen_param in &self.rcx.tcx().generics_of(impl_hir_id.owner).params {
                    if let GenericParamDefKind::Type { .. } = gen_param.kind {
                        // Check if the current ADT acts as a `ConcurrentQ` type for the generic parameter.
                        if let Some(AdtBehavior::ConcurrentQ) = adt_behavior.get(&gen_param.index) {
                            need_send.insert(gen_param.index);
                        } else {
                            need_sync.insert(gen_param.index);
                        }
                    }
                }

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
                            if phantom_params.contains(&param_ty.index) {
                                continue;
                            }

                            let trait_did = trait_predicate.def_id();
                            if trait_did == sync_trait_def_id {
                                need_sync.remove(&param_ty.index);
                                // Naively assume a `Sync` object is also `Send`.
                                need_send.remove(&param_ty.index);
                            } else if trait_did == send_trait_def_id {
                                need_send.remove(&param_ty.index);
                            }
                        }
                    }
                }

                return if need_sync.is_empty() && need_send.is_empty() {
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
        copy_trait_def_id: DefId,
    ) -> Option<DefId> {
        let tcx = self.rcx.tcx();
        if let Some(trait_ref) = tcx.impl_trait_ref(impl_hir_id.owner) {
            if let ty::TyKind::Adt(adt_def, substs) = trait_ref.self_ty().kind {
                let adt_did = adt_def.did;

                // Keep track of generic params that need to be `Send`.
                let mut need_send: FxHashSet<u32> = FxHashSet::default();

                // Keep track of generic params that need to be `Send + Sync`.
                // e.g. `Arc<T>` requires T: Send + Sync for Send
                let mut need_sync: FxHashSet<u32> = FxHashSet::default();

                // Generic params that only occur within `PhantomData<_>`
                let phantom_params = self
                    .phantom_map
                    .entry(adt_did)
                    .or_insert_with(|| phantom_indices(tcx, adt_def, substs));

                // Get `AdtBehavior` per generic parameter.
                let adt_behavior = self
                    .behavior_map
                    .entry(adt_did)
                    .or_insert(adt_behavior(tcx, adt_did));

                // Initialize sets `need_send` & `need_sync`
                for gen_param in tcx.generics_of(impl_hir_id.owner).params.iter() {
                    if let GenericParamDefKind::Type { .. } = gen_param.kind {
                        need_send.insert(gen_param.index);

                        // Check if the current ADT acts as a `PtrLike` type for the generic parameter.
                        if let Some(AdtBehavior::PtrLike) = adt_behavior.get(&gen_param.index) {
                            need_sync.insert(gen_param.index);
                        }
                    }
                }

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
                            // Check if query is cached before calling `phantom_indices`.
                            if phantom_params.contains(&param_ty.index) {
                                continue;
                            }

                            let trait_did = trait_predicate.def_id();
                            if trait_did == send_trait_def_id || trait_did == copy_trait_def_id {
                                need_send.remove(&param_ty.index);
                            } else if trait_did == sync_trait_def_id {
                                if need_sync.contains(&param_ty.index) {
                                    need_sync.remove(&param_ty.index);
                                } else {
                                    need_send.remove(&param_ty.index);
                                }
                            }
                        }
                    }
                }

                return if need_send.is_empty() && need_sync.is_empty() {
                    None
                } else {
                    Some(adt_did)
                };
            }
        }
        return None;
    }
}
