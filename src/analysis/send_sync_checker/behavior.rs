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

    // Solely moves `T` and doesn't return `&T` within API output.
    // For `impl Sync`, `T: Send` is needed.
    // Identify as 'ConcurrentQueue' if all `&self` methods of ADT only
    // take owned `T` within inputs and/or return owned `T` within return type.
    // This category also includes `Mutex-like` types,
    // mutex-like types & concurrentQueue-like types share similar features
    // in terms of API input/output.
    ConcurrentQueue,

    // For "Standard" ADTs, we apply our original filtering criteria:
    // * `impl Send for ADT<T, ..>` => require `T: Send`
    // * `impl Sync for ADT<T, ..>` => require `T: Sync`
    Standard,

    // This category may or may not contain true positives.
    // We don't do further analysis for this category.
    // Future work could try to implement more precision
    // in analyzing this category.
    Undefined,
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
        const PASS_OWNED = 0b00000100;

        // Satisfies both of the following conditions:
        // * `&T` is not exposed in any method ret type.
        // * TODO: There exists a method that takes closure `Fn(&T)` as input.
        //
        // Current limitation:
        // This category may include cases where `&T` isn't directly exposed as ret type,
        // but the ret type (ADT) has an API to expose `&T`.
        // Greater precision can be achieved by implementing the following:
        // * For more precision: check APIs of the ret types.
        // * For even more precision: check (APIs of the ret types of (APIs of the ret types)).
        // * Need more precision?: recurse until reaching your predefined maximum search depth.
        // Inspection cost expected to be exponential to # of layers (of ADT).
        const NO_DEREF = 0b00001000;

        // Satisfies either one of the following conditions:
        // * `&T` is exposed in method return type.
        // * TODO: `&T` can be accessed by method input closure `Fn(&T)`
        // Current limitation:
        // May miss out on cases where `&T` isn't directly exposed as ret type,
        // but the ret type (ADT) has an API to expose `&T`.
        const DEREF = 0b00010000;
    }
}

impl Cond {
    /* TODO: Implement fine-grained handling for ptr-like types.
    fn ptr_like(&self) -> bool {
        self.intersects(Cond::REF_REF & !Cond::CLONED)
    }
    */
    fn is_concurrent_queue(&self) -> bool {
        self.intersects(Cond::NO_DEREF) && self.intersects(Cond::PASS_OWNED)
    }
    fn is_undefined(&self) -> bool {
        self.intersects(Cond::NO_DEREF) && !self.intersects(Cond::PASS_OWNED)
    }
}

// Enum to differentiate DefIds of
// `&self` methods with constructor functions.
enum FnType {
    ConstructSelf(DefId),
    // Note that this only refers to `&self` methods and not `&mut self` methods.
    TakeBorrowedSelf(DefId),
}

/// For each generic parameter (identified by index) of a given ADT,
/// inspect fn signature & body to identify `AdtBehavior`.
/// Inspects all `safe` methods of the given ADT, including methods from trait impls.
pub(crate) fn adt_behavior<'tcx>(
    rcx: RudraCtxt<'tcx>,
    adt_did: DefId,
) -> FxHashMap<PostMapIdx, AdtBehavior> {
    let tcx = rcx.tcx();

    // Set of `T`s that appear only as owned `T` in either input or output of APIs.
    let mut owned_generic_params = FxHashSet::default();
    // Set of `T`s that appear only as `&T` in return type of APIs.
    let mut deref_generic_params = FxHashSet::default();

    let adt_ty = tcx.type_of(adt_did);
    // For ADT `Foo<A, B>` => adt_ty_name = `Foo`
    let adt_ty_name = tcx.item_name(adt_did);

    let adt_generic_params = &tcx.generics_of(adt_did).params;

    if let Some(relevant_impls) = rcx.index_adt_cache(&adt_did) {
        // Inspect `impl`s relevant to the given ADT.
        for (impl_hir_id, impl_self_ty) in relevant_impls.iter() {
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

                // DefIds of `safe` functions (within the given impl block)
                // which either satisfy the following.
                // * Take `&self` within its first parameter type.
                // * Construct the Self type, but don't contain `self` within its inputs.
                let relevant_safe_fns = tcx
                    .associated_items(impl_hir_id.owner)
                    .in_definition_order()
                    .filter_map(|assoc_item| {
                        if assoc_item.kind == AssocKind::Fn {
                            let fn_did = assoc_item.def_id;
                            let fn_sig = tcx.fn_sig(fn_did).skip_binder();
                            if let rustc_hir::Unsafety::Unsafe = fn_sig.unsafety {
                                return None;
                            }
                            if assoc_item.fn_has_self_parameter {
                                // Check if the given method takes `&self` within its first parameter's type.
                                // We already know the method takes `self` within its first parameter,
                                // so we only check whether the first parameter contains a reference.
                                // e.g. `&self`, `Box<&self>`, `Pin<&self>`, ..
                                let mut walker = fn_sig.inputs()[0].walk();
                                while let Some(node) = walker.next() {
                                    if let GenericArgKind::Type(ty) = node.unpack() {
                                        if let ty::TyKind::Ref(_, _, Mutability::Not) = ty.kind {
                                            return Some(FnType::TakeBorrowedSelf(fn_did));
                                        }
                                    }
                                }
                            } else {
                                // Check if the function return type equals `Self`.
                                if TyS::same_type(fn_sig.output(), adt_ty) {
                                    return Some(FnType::ConstructSelf(fn_did));
                                }
                            }
                        }
                        return None;
                    });

                // Since each `impl` block may assign different indices to equivalent generic parameters,
                // We need one translation map per `impl` block.
                let generic_param_idx_map =
                    generic_param_idx_mapper(adt_generic_params, impl_substs);

                // Inspect selected functions' input/output types to determine `AdtBehavior`.
                for fn_type in relevant_safe_fns {
                    match fn_type {
                        FnType::ConstructSelf(fn_did) => {
                            let fn_ctxt_pseudo_owned_param_idx_map =
                                find_pseudo_owned_in_fn_ctxt(tcx, fn_did);
                            let fn_sig = tcx.fn_sig(fn_did).skip_binder();
                            // Check inputs of the constructor
                            for input_ty in fn_sig.inputs() {
                                for owned_idx in owned_generic_params_in_ty(tcx, input_ty)
                                    .into_iter()
                                    .map(|mut idx| {
                                        *fn_ctxt_pseudo_owned_param_idx_map
                                            .get(&idx)
                                            .unwrap_or(&idx)
                                    })
                                {
                                    if let Some(&mapped_idx) = generic_param_idx_map.get(&owned_idx)
                                    {
                                        owned_generic_params.insert(mapped_idx);
                                    }
                                }
                            }
                        }
                        FnType::TakeBorrowedSelf(method_did) => {
                            let fn_ctxt_pseudo_owned_param_idx_map =
                                find_pseudo_owned_in_fn_ctxt(tcx, method_did);
                            let fn_sig = tcx.fn_sig(method_did).skip_binder();

                            // Check generic parameters that are passed as owned `T`.
                            for ty in fn_sig.inputs_and_output.iter() {
                                for owned_idx in owned_generic_params_in_ty(tcx, ty)
                                    .into_iter()
                                    .map(|mut idx| {
                                        *fn_ctxt_pseudo_owned_param_idx_map
                                            .get(&idx)
                                            .unwrap_or(&idx)
                                    })
                                {
                                    if let Some(&mapped_idx) = generic_param_idx_map.get(&owned_idx)
                                    {
                                        owned_generic_params.insert(mapped_idx);
                                    }
                                }
                            }

                            // Check whether any of the methods return either `&T` or `Option<&T>` or `Result<&T>`.
                            for peek_idx in borrowed_generic_params_in_ty(tcx, fn_sig.output())
                                .into_iter()
                                .map(|mut idx| {
                                    *fn_ctxt_pseudo_owned_param_idx_map.get(&idx).unwrap_or(&idx)
                                })
                            {
                                if let Some(&mapped_idx) = generic_param_idx_map.get(&peek_idx) {
                                    deref_generic_params.insert(mapped_idx);
                                }
                            }

                            // TODO: Check whether any of the method inputs are closures of type `Fn(&T) -> !`.
                            // for _closure_ty in fn_sig.inputs().iter().filter(|ty| ty.is_closure()) {}
                        }
                    }
                }
            }
        }
    }

    let all_generic_params: FxHashSet<PostMapIdx> = adt_generic_params
        .iter()
        .filter_map(|x| {
            if let GenericParamDefKind::Type { .. } = x.kind {
                Some(PostMapIdx(x.index))
            } else {
                None
            }
        })
        .collect();

    // cond_map: (idx of generic parameter `T`) => (`Cond`)
    let mut cond_map = FxHashMap::default();

    for &param_idx in deref_generic_params.iter() {
        cond_map
            .entry(param_idx)
            .or_insert(Cond::empty())
            .insert(Cond::DEREF);
    }
    for &param_idx in all_generic_params.difference(&deref_generic_params) {
        cond_map
            .entry(param_idx)
            .or_insert(Cond::empty())
            .insert(Cond::NO_DEREF);
    }
    for &param_idx in owned_generic_params.difference(&deref_generic_params) {
        cond_map
            .entry(param_idx)
            .or_insert(Cond::empty())
            .insert(Cond::PASS_OWNED);
    }

    // Map: (idx of generic parameter) => (AdtBehavior)
    let mut behavior_map = FxHashMap::default();
    for (param_idx, cond) in cond_map.into_iter() {
        behavior_map.insert(
            param_idx,
            if cond.is_concurrent_queue() {
                AdtBehavior::ConcurrentQueue
            } else if cond.is_undefined() {
                AdtBehavior::Undefined
            } else {
                AdtBehavior::Standard
            },
        );
    }
    return behavior_map;
}
