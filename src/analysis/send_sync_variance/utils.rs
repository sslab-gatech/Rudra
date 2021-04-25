use super::*;

// Note that len(adt_generics_iter) == len(substs_generics_iter)
pub fn generic_param_idx_mapper<'tcx>(
    adt_generics: &Vec<GenericParamDef>,
    substs_generics: &'tcx List<subst::GenericArg<'tcx>>,
) -> FxHashMap<PreMapIdx, PostMapIdx> {
    let mut generic_param_idx_mapper = FxHashMap::default();
    for (original, substituted) in adt_generics.iter().zip(substs_generics.iter()) {
        if let GenericArgKind::Type(ty) = substituted.unpack() {
            // Currently, we focus on the generic parameters that exist in the ADT definition.

            // We ignore cases where a generic parameter is replaced with a concrete type.
            // e.g. `impl Send for My<A, i32> {}`
            if let ty::TyKind::Param(param_ty) = ty.kind {
                generic_param_idx_mapper
                    .insert(PreMapIdx(param_ty.index), PostMapIdx(original.index));
            }
            // We also may not take into account
            // some additional generic parameters introduced in impl/method contexts.
            /*
            impl<'a, A: 'a, B: Fn(&'a A)> My<A, B> {
                // C.index = 3
                pub fn hello<'b, C>(&self, x: C, y: &'b B) {}
            }
            */
        }
    }
    return generic_param_idx_mapper;
}

// Within the given `ty`,
// return generic parameters that exist as owned `T`
pub fn owned_generic_params_in_ty<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
) -> impl IntoIterator<Item = PreMapIdx> {
    let mut owned_generic_params = FxHashSet::default();

    let mut worklist = vec![ty];
    let mut visited = FxHashSet::default();
    while let Some(ty) = worklist.pop() {
        if visited.contains(&ty) {
            continue;
        }

        visited.insert(ty);
        match ty.kind {
            ty::TyKind::Param(param_ty) => {
                owned_generic_params.insert(param_ty.index);
            }
            ty::TyKind::Adt(adt_def, substs) => {
                if ty.is_box() {
                    worklist.push(ty.boxed_ty());
                    continue;
                }
                // TODO:
                //   Besides `Box<T>`,
                //   do we need special handling for types that own T but doesn't have a field `T`?
                //   ex) Arc<T> or Rc<T> ?

                for adt_variant in adt_def.variants.iter() {
                    for adt_field in adt_variant.fields.iter() {
                        worklist.push(adt_field.ty(tcx, substs));
                    }
                }
            }
            ty::TyKind::Array(ty, _) => {
                worklist.push(ty);
            }
            ty::TyKind::Tuple(substs) => {
                for ty in substs.types() {
                    worklist.push(ty);
                }
            }
            _ => {}
        }
    }

    owned_generic_params.into_iter().map(|idx| PreMapIdx(idx))
}

// Within the given `ty`,
// return generic parameters that exist as `&T`.
pub fn borrowed_generic_params_in_ty<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
) -> impl IntoIterator<Item = PreMapIdx> {
    let mut borrowed_generic_params = FxHashSet::default();

    let mut worklist = vec![(ty, false)];
    let mut visited = FxHashSet::default();
    while let Some((ty, borrowed)) = worklist.pop() {
        if visited.contains(&ty) {
            continue;
        }
        
        visited.insert(ty);
        match ty.kind {
            ty::TyKind::Param(param_ty) => {
                if borrowed {
                    borrowed_generic_params.insert(param_ty.index);
                }
            }
            ty::TyKind::Ref(_, borrowed_ty, Mutability::Not) => {
                worklist.push((borrowed_ty, true));
            }
            ty::TyKind::Adt(adt_def, substs) => {
                if ty.is_box() {
                    worklist.push((ty.boxed_ty(), borrowed));
                    continue;
                }
                // TODO:
                //   Besides `Box<T>`,
                //   do we need special handling for types that own T but doesn't have a field `T`?
                //   ex) Arc<T> or Rc<T> ?

                for adt_variant in adt_def.variants.iter() {
                    for adt_field in adt_variant.fields.iter() {
                        let adt_field_ty = adt_field.ty(tcx, substs);
                        // We peel off just one level of ADT layer when trying to find exposed `&T`.
                        // This helps to limit complexity & rule out Mutex-like FPs.
                        if let ty::TyKind::Adt(_, _) = adt_field_ty.kind {
                        } else {
                            worklist.push((adt_field_ty, borrowed));
                        }
                    }
                }
            }
            ty::TyKind::Array(ty, _) => {
                worklist.push((ty, borrowed));
            }
            ty::TyKind::Tuple(substs) => {
                for ty in substs.types() {
                    worklist.push((ty, borrowed));
                }
            }
            _ => {}
        }
    }

    borrowed_generic_params
        .into_iter()
        .map(|idx| PreMapIdx(idx))
}

const PSEUDO_OWNED: [&'static str; 4] = [
    "std::convert::Into",
    "core::convert::Into",
    "std::iter::IntoIterator",
    "core::iter::IntoIterator",
];

// Check for trait bounds introduced in function-level context.
// We want to catch cases equivalent to sending `P` (refer to example below)
//
// example)
//    impl<P, Q> Channel<P, Q> {
//        fn send_p<M>(&self, _msg: M) where M: Into<P>, {}
//    }
pub fn find_pseudo_owned_in_fn_ctxt<'tcx>(
    tcx: TyCtxt<'tcx>,
    fn_did: DefId,
) -> FxHashMap<PreMapIdx, PreMapIdx> {
    let mut fn_ctxt_pseudo_owned_param_idx_map = FxHashMap::default();
    for atom in tcx
        .param_env(fn_did)
        .caller_bounds()
        .iter()
        .map(|x| x.skip_binders())
    {
        if let PredicateAtom::Trait(trait_predicate, _) = atom {
            if let ty::TyKind::Param(param_ty) = trait_predicate.self_ty().kind {
                let substs = trait_predicate.trait_ref.substs;
                let substs_types = substs.types().collect::<Vec<_>>();

                // trait_predicate =>  M: Into<P>
                //                     |    |
                //             (param_ty)  (trait_predicate.trait_ref)
                if PSEUDO_OWNED.contains(&tcx.def_path_str(trait_predicate.def_id()).as_str()) {
                    if let ty::TyKind::Param(param_1) = substs_types[1].kind {
                        fn_ctxt_pseudo_owned_param_idx_map
                            .insert(PreMapIdx(param_ty.index), PreMapIdx(param_1.index));
                    }
                }
            }
        }
    }

    fn_ctxt_pseudo_owned_param_idx_map
}
