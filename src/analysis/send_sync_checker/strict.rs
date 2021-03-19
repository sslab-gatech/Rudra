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
        let rcx = self.rcx;
        let tcx = rcx.tcx();
        if let Some(trait_ref) = tcx.impl_trait_ref(impl_hir_id.owner) {
            if let ty::TyKind::Adt(adt_def, impl_trait_substs) = trait_ref.self_ty().kind {
                let adt_did = adt_def.did;
                let adt_ty = tcx.type_of(adt_did);

                // Keep track of generic params that need to be `Sync`.
                let mut need_sync: FxHashSet<u32> = FxHashSet::default();

                // Keep track of generic params that need to be `Send`.
                let mut need_send: FxHashSet<u32> = FxHashSet::default();

                // Generic params that only occur within `PhantomData<_>`
                let phantom_params = self
                    .phantom_map
                    .entry(adt_did)
                    .or_insert_with(|| phantom_indices(tcx, adt_ty));

                // Get `AdtBehavior` per generic parameter.
                let adt_behavior = self
                    .behavior_map
                    .entry(adt_did)
                    .or_insert_with(|| adt_behavior(rcx, adt_did));

                // Initialize sets `need_send` & `need_sync`.
                if adt_def.is_struct() {
                    for gen_param in tcx.generics_of(adt_did).params.iter() {
                        if let GenericParamDefKind::Type { .. } = gen_param.kind {
                            // Skip generic parameters that are only within `PhantomData<T>`.
                            if phantom_params.contains(&gen_param.index) {
                                continue;
                            }

                            // Check if the current ADT acts as a `ConcurrentQueue` type for the generic parameter.
                            if let Some(behavior) = adt_behavior.get(&gen_param.index) {
                                match behavior {
                                    AdtBehavior::ConcurrentQueue => {
                                        need_send.insert(gen_param.index);
                                    }
                                    AdtBehavior::Standard => {
                                        need_sync.insert(gen_param.index);
                                    }
                                    AdtBehavior::Undefined => {}
                                }
                            }
                        }
                    }
                } else {
                    // Fields of enums/unions can be accessed by pattern matching.
                    // In this case, we don't make distinction of treatment according to `AdtBehavior`.
                    // We require all generic parameters to be `Sync`.
                    for gen_param in tcx.generics_of(adt_did).params.iter() {
                        if let GenericParamDefKind::Type { .. } = gen_param.kind {
                            // Skip generic parameters that are only within `PhantomData<T>`.
                            if phantom_params.contains(&gen_param.index) {
                                continue;
                            }

                            need_sync.insert(gen_param.index);
                        }
                    }
                }

                // If the below assertion fails, there must be an issue with librustc we're using.
                // assert_eq!(tcx.generics_of(adt_did).params.len(), substs.len());
                let generic_param_idx_map =
                    generic_param_idx_mapper(&tcx.generics_of(adt_did).params, impl_trait_substs);

                // Iterate over predicates to check trait bounds on generic params.
                for atom in tcx
                    .param_env(impl_hir_id.owner)
                    .caller_bounds()
                    .iter()
                    .map(|x| x.skip_binders())
                {
                    if let PredicateAtom::Trait(trait_predicate, _) = atom {
                        if let ty::TyKind::Param(param_ty) = trait_predicate.self_ty().kind {
                            if let Some(mapped_idx) = generic_param_idx_map.get(&param_ty.index) {
                                let trait_did = trait_predicate.def_id();
                                if trait_did == sync_trait_def_id {
                                    need_sync.remove(mapped_idx);
                                } else if trait_did == send_trait_def_id {
                                    need_send.remove(mapped_idx);
                                }
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

    /// Returns `Some(DefId of ADT)` if `impl Send` for the ADT looks suspicious
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
            if let ty::TyKind::Adt(adt_def, impl_trait_substs) = trait_ref.self_ty().kind {
                let adt_did = adt_def.did;
                let adt_ty = tcx.type_of(adt_did);

                // Keep track of generic params that need to be `Send`.
                let mut need_send: FxHashSet<u32> = FxHashSet::default();

                // Generic params that only occur within `PhantomData<_>`
                let phantom_params = self
                    .phantom_map
                    .entry(adt_did)
                    .or_insert_with(|| phantom_indices(tcx, adt_ty));

                // If the below assertion fails, there must be an issue with librustc we're using.
                // assert_eq!(tcx.generics_of(adt_did).params.len(), substs.len());
                let generic_param_idx_map =
                    generic_param_idx_mapper(&tcx.generics_of(adt_did).params, impl_trait_substs);

                // Initialize set `need_send`
                for gen_param in tcx.generics_of(adt_did).params.iter() {
                    if let GenericParamDefKind::Type { .. } = gen_param.kind {
                        // Skip generic parameters that are only within `PhantomData<T>`.
                        if phantom_params.contains(&gen_param.index) {
                            continue;
                        }

                        need_send.insert(gen_param.index);
                    }
                }

                /* Our current filtering policy for `impl Send`:
                    1. Allow `T: Send` for `impl Send`
                    2. Allow `T: Sync` for `impl Send`
                        There are rare counterexamples (`!Send + Sync`) like `MutexGuard<_>`,
                        but we assume that in most of the common cases this holds true.
                    3. Allow `T: Copy` for `impl Send`
                        We shouldn't unconditionally allow `T: Copy for impl Send`,
                        due to the following edge case:
                        ```
                            // Below example be problematic for cases where T: !Sync .
                            struct Atom1<'a, T>(&'a T);
                            unsafe impl<'a, T: Copy> Send for Atom1<'a, T> {}
                        ```
                        TODO: implement additional checking to catch above edge case.
                */

                // Iterate over predicates to check trait bounds on generic params.
                for atom in tcx
                    .param_env(impl_hir_id.owner)
                    .caller_bounds()
                    .iter()
                    .map(|x| x.skip_binders())
                {
                    if let PredicateAtom::Trait(trait_predicate, _) = atom {
                        if let ty::TyKind::Param(param_ty) = trait_predicate.self_ty().kind {
                            if let Some(mapped_idx) = generic_param_idx_map.get(&param_ty.index) {
                                let trait_did = trait_predicate.def_id();
                                if trait_did == send_trait_def_id
                                    || trait_did == sync_trait_def_id
                                    || trait_did == copy_trait_def_id
                                {
                                    need_send.remove(mapped_idx);
                                }
                            }
                        }
                    }
                }

                return if need_send.is_empty() {
                    None
                } else {
                    Some(adt_did)
                };
            }
        }
        return None;
    }
}