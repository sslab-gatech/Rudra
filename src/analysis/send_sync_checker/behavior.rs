//! Identify behavior of ADT (per generic param)

use super::*;

// For each generic parameter of an ADT, ADT shows one of the behaviors below.
// Use this info to strengthen/relax safety bar of trait bounds on generic params.
#[derive(Debug)]
pub(crate) enum AdtBehavior {
    // Acts like a shared pointer (e.g. Arc, Rc, Beef).
    // For `impl Send`, `T: Send + Sync` is needed. (filtering criteria fortified)
    // Identify as 'PtrLike' if ADT satisfies both of the conditions below:
    //
    // Condition #1:
    //    At least one API (of ADT) satisfies both of the following conditions.
    //    (1) `&self` contained within first parameter
    //    (2) return type contains either `&T` or `&mut T`
    // Condition #2:
    //    Cloning the ADT does not clone `T`
    // PtrLike,

    // Solely moves `T` and doesn't take `&T` as API input.
    // For `impl Sync`, `T: Send` is sufficient. (fitering criteria relaxed)
    // Identify as 'PtrLike' if all `&self` methods of ADT only
    // take owned `T` within inputs and/or return owned `T` within return type.
    ConcurrentQueue,

    // For "Standard" ADTs, we apply our original filtering criteria:
    // * `impl Send for ADT<T, ..>` => require `T: Send`
    // * `impl Sync for ADT<T, ..>` => require `T: Sync`
    Standard,
}

bitflags! {
    struct Cond: u8 {
        /* TODO: Implement fine-grained handling for ptr-like types.
        // At least one API of ADT takes `&self` within input and `&T` within output.
        const REF_REF = 0b00000001;
        // `Clone` impl clones on generic param `T.
        const CLONED = 0b00000010;
        */
        // `T` only appears in ADT API input/output as owned `T`.
        // (Sender/Receiver side of Queue APIs)
        // e.g. `T`, `Box<T>`, `Option<T>`, `Result<T, !>`
        const PASSOWNED = 0b00000100;
    }
}

impl Cond {
    /* TODO: Implement fine-grained handling for ptr-like types.
    fn ptr_like(&self) -> bool {
        self.intersects(Cond::REF_REF & !Cond::CLONED)
    }
    */
    fn is_concurrent_queue(&self) -> bool {
        self.intersects(Cond::PASSOWNED)
    }
}

const TO_OWNED: [&'static str; 4] = [
    "std::convert::Into",
    "core::convert::Into",
    "std::iter::IntoIterator",
    "core::iter::IntoIterator",
];

/// For each generic parameter (identified by index) of a given ADT,
/// inspect fn signature & body to identify `AdtBehavior`.
/// Inspects all methods of the given ADT, including methods from trait impls.
pub(crate) fn adt_behavior<'tcx>(
    rcx: RudraCtxt<'tcx>,
    adt_did: DefId,
) -> FxHashMap<u32, AdtBehavior> {
    let tcx = rcx.tcx();
    // Map: (idx of generic parameter `T`) => (`Cond`)
    let mut cond_map = FxHashMap::default();

    // Set of `T`s that appear only as owned `T` in either input or output of APIs.
    let mut owned_generic_params = FxHashSet::default();
    // Set of `T`s that appear only as `&T` in return type of APIs.
    let mut peek_generic_params = FxHashSet::default();

    let _adt_ty = tcx.type_of(adt_did);
    // For ADT `Foo<A, B>` => adt_ty_name = `Foo`
    let adt_ty_name = tcx.item_name(adt_did);

    let adt_generic_params = &tcx.generics_of(adt_did).params;

    // Inspect `impl`s relevant to the given ADT.
    for (impl_hir_id, item) in tcx.hir().krate().items.iter() {
        if let ItemKind::Impl { self_ty, .. } = &item.kind {
            let impl_self_ty = tcx.type_of(self_ty.hir_id.owner);
            if let ty::TyKind::Adt(impl_self_adt_def, impl_substs) = impl_self_ty.kind {
                let impl_self_ty_name = tcx.item_name(impl_self_adt_def.did);
                if adt_ty_name != impl_self_ty_name {
                    continue;
                }

                // At this point, `adt_ty.name == impl_self_ty_name` . (Foo == Foo)

                // There are three possiblities now..
                // (1) adt_ty != impl_self_ty . (Foo<A, B> != Foo<i64, B>)
                // (2) adt_ty != impl_self_ty . (Foo<A, B> != Foo<A, B: Send>)
                // (3) adt_ty == impl_self_ty . (Foo<A, B> == Foo<A, B>)
                // TODO: Should we cater to each of the possibilities?

                // DefIds of methods within the given impl block.
                let method_dids = tcx
                    .associated_items(impl_hir_id.owner)
                    .in_definition_order()
                    .filter_map(|assoc_item| {
                        if assoc_item.kind == AssocKind::Fn && assoc_item.fn_has_self_parameter {
                            // We are only inspecting methods that take `self` within its input.
                            Some(assoc_item.def_id)
                        } else {
                            None
                        }
                    });

                // Since each `impl` block may assign different indices to equivalent generic parameters,
                // We need one translation map per `impl` block.
                let generic_param_idx_map =
                    generic_param_idx_mapper(adt_generic_params, impl_substs);

                // Inspect `&self` methods defined within current impl block.
                for (method_did, fn_sig) in method_dids
                    .map(|did| (did, tcx.fn_sig(did).skip_binder()))
                    .filter(|(_, fn_sig)| {
                        // Only inspect `safe` methods?
                        if let rustc_hir::Unsafety::Unsafe = fn_sig.unsafety {
                            return false;
                        }
                        // Check if the given method takes `&self` within its first parameter's type.
                        // e.g. `&self`, `Box<&self>`, `Pin<&self>`, ..
                        let mut walker = fn_sig.inputs()[0].walk();
                        while let Some(node) = walker.next() {
                            if let GenericArgKind::Type(ty) = node.unpack() {
                                if let ty::TyKind::Ref(_, _, Mutability::Not) = ty.kind {
                                    return true;
                                }
                            }
                        }
                        false
                    })
                {
                    /*
                        Check for trait bounds introduced in function-level context.
                        We want to catch cases equivalent to sending `P` (refer to example below)

                        // example )
                        impl<P, Q> Channel<P, Q> {
                            fn send_p<M>(&self, _msg: M) where M: Into<P>, {}
                        }
                    */
                    let mut param_to_param = FxHashMap::default();
                    for atom in tcx
                        .param_env(method_did)
                        .caller_bounds()
                        .iter()
                        .map(|x| x.skip_binders())
                    {
                        if let PredicateAtom::Trait(trait_predicate, _) = atom {
                            if let ty::TyKind::Param(param_ty) = trait_predicate.self_ty().kind {
                                let substs = trait_predicate.trait_ref.substs;

                                // trait_predicate =>  M: Into<P>
                                //                     |    |
                                //             (param_ty)  (trait_predicate.trait_ref)

                                if TO_OWNED
                                    .contains(&tcx.def_path_str(trait_predicate.def_id()).as_str())
                                {
                                    // substs = [M, P]
                                    if let ty::TyKind::Param(param_1) = substs.type_at(1).kind {
                                        param_to_param.insert(param_ty.index, param_1.index);
                                    }
                                }
                            }
                        }
                    }

                    // Check generic parameters that are passed as owned `T`.
                    for ty in fn_sig.inputs_and_output.iter() {
                        for mut owned_idx in owned_generic_params_in_ty(tcx, ty) {
                            if let Some(&idx) = param_to_param.get(&owned_idx) {
                                owned_idx = idx;
                            }
                            if let Some(&mapped_idx) = generic_param_idx_map.get(&owned_idx) {
                                owned_generic_params.insert(mapped_idx);
                            }
                        }
                    }

                    // Check whether the ADT has peek functionality for any of the generic parameters.
                    for peek_idx in borrowed_generic_params_in_ty(tcx, fn_sig.output()) {
                        if let Some(&mapped_idx) = param_to_param.get(&peek_idx) {
                            peek_generic_params.insert(mapped_idx);
                        }
                    }
                }
            }
        }
    }

    for &param_idx in owned_generic_params.difference(&peek_generic_params) {
        cond_map
            .entry(param_idx)
            .or_insert(Cond::empty())
            .insert(Cond::PASSOWNED);
    }

    // Map: (idx of generic parameter) => (AdtBehavior)
    let mut behavior_map = FxHashMap::default();
    for (param_idx, cond) in cond_map.into_iter() {
        behavior_map.insert(
            param_idx,
            if cond.is_concurrent_queue() {
                AdtBehavior::ConcurrentQueue
            } else {
                AdtBehavior::Standard
            },
        );
    }
    return behavior_map;
}
