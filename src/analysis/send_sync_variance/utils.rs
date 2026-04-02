use super::*;

// Note that len(adt_generics_iter) == len(args_generics_iter)
pub fn generic_param_idx_mapper<'tcx>(
    adt_generics: &[GenericParamDef],
    args_generics: ty::GenericArgsRef<'tcx>,
) -> FxHashMap<PreMapIdx, PostMapIdx> {
    let mut generic_param_idx_mapper = FxHashMap::default();
    for (original, substituted) in adt_generics.iter().zip(args_generics.iter()) {
        if let GenericArgKind::Type(ty) = substituted.unpack() {
            // Currently, we focus on the generic parameters that exist in the ADT definition.

            // We ignore cases where a generic parameter is replaced with a concrete type.
            // e.g. `impl Send for My<A, i32> {}`
            if let ty::TyKind::Param(param_ty) = ty.kind() {
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

const OWNING_ADTS: &[&[&str]] = &[&["core", "option", "Option"], &["core", "result", "Result"]];

// Within the given `ty`,
// return generic parameters that exist as owned `T`
pub fn owned_generic_params_in_ty<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
) -> impl IntoIterator<Item = PreMapIdx> {
    let ext = tcx.ext();
    let mut owned_generic_params = FxHashSet::default();

    let mut worklist = vec![ty];
    let mut visited = FxHashSet::default();
    while let Some(ty) = worklist.pop() {
        if visited.contains(&ty) {
            continue;
        }

        visited.insert(ty);
        match ty.kind() {
            ty::TyKind::Param(param_ty) => {
                owned_generic_params.insert(param_ty.index);
            }
            ty::TyKind::Adt(adt_def, args) => {
                if ty.is_box() {
                    if let Some(boxed) = ty.boxed_ty() {
                        worklist.push(boxed);
                    }
                    continue;
                }
                // TODO:
                //   Besides `Box<T>`,
                //   do we need special handling for types that own T but doesn't have a field `T`?
                //   ex) Arc<T> or Rc<T> ?

                // Try limiting to cases like Option<T> & Result<T, !> to reduce FP rate.
                for path in OWNING_ADTS {
                    if ext.match_def_path(adt_def.did(), path) {
                        for adt_variant in adt_def.variants().iter() {
                            for adt_field in adt_variant.fields.iter() {
                                let ty = adt_field.ty(tcx, args);
                                if let ty::TyKind::Param(_) = ty.kind() {
                                    worklist.push(ty);
                                }
                            }
                        }
                    }
                }
            }
            ty::TyKind::Array(inner_ty, _) => {
                worklist.push(*inner_ty);
            }
            ty::TyKind::Tuple(args) => {
                for ty in args.iter() {
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
        match ty.kind() {
            ty::TyKind::Param(param_ty) => {
                if borrowed {
                    borrowed_generic_params.insert(param_ty.index);
                }
            }
            ty::TyKind::Ref(_, borrowed_ty, Mutability::Not) => {
                worklist.push((*borrowed_ty, true));
            }
            ty::TyKind::Adt(adt_def, args) => {
                if ty.is_box() {
                    if let Some(boxed) = ty.boxed_ty() {
                        worklist.push((boxed, borrowed));
                    }
                    continue;
                }

                for adt_variant in adt_def.variants().iter() {
                    for adt_field in adt_variant.fields.iter() {
                        let adt_field_ty = adt_field.ty(tcx, args);
                        // We peel off just one level of ADT layer when trying to find exposed `&T`.
                        // This helps to limit complexity & rule out Mutex-like FPs.
                        if let ty::TyKind::Adt(_, _) = adt_field_ty.kind() {
                        } else {
                            worklist.push((adt_field_ty, borrowed));
                        }
                    }
                }
            }
            ty::TyKind::Array(inner_ty, _) => {
                worklist.push((*inner_ty, borrowed));
            }
            ty::TyKind::Tuple(args) => {
                for ty in args.iter() {
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
    for clause in tcx
        .param_env(fn_did)
        .caller_bounds()
        .iter()
    {
        if let ty::ClauseKind::Trait(trait_predicate) = clause.kind().skip_binder() {
            if let ty::TyKind::Param(param_ty) = trait_predicate.self_ty().kind() {
                let args = trait_predicate.trait_ref.args;
                let args_types: Vec<_> = args.iter()
                    .filter_map(|arg| arg.as_type())
                    .collect();

                // trait_predicate =>  M: Into<P>
                //                     |    |
                //             (param_ty)  (trait_predicate.trait_ref)
                if PSEUDO_OWNED.contains(&tcx.def_path_str(trait_predicate.def_id()).as_str()) {
                    if args_types.len() > 1 {
                        if let ty::TyKind::Param(param_1) = args_types[1].kind() {
                            fn_ctxt_pseudo_owned_param_idx_map
                                .insert(PreMapIdx(param_ty.index), PreMapIdx(param_1.index));
                        }
                    }
                }
            }
        }
    }

    fn_ctxt_pseudo_owned_param_idx_map
}
