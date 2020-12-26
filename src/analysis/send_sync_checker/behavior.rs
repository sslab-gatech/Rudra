//! Identify behavior of ADT (per generic param)

use super::*;

// For each generic parameter of an ADT, ADT shows one of the behavior below.
// Use this info to strengthen/relax safety bar of trait bounds on generic params.
#[derive(Debug)]
pub(crate) enum AdtBehavior {
    // Acts like a shared pointer (e.g. Arc, Rc, Beef).
    // For `impl Send`, `T: Send + Sync` is needed. (Bar is strengthened)
    // Identify as this variant if the given ADT satisfies the conditions below:
    //
    // Condition #1 (`CondBit::RefRef`):
    //    any of the APIs have one of the following signatures:
    //    * `fn(&self) -> &T`
    //    * `fn(&self) -> &mut T`
    // Condition #2 (`CondBit::NotCloneParam`):
    //    Cloning the ADT does not clone `T`
    PtrLike,
    // Moves `T` without ever dereferencing `T`.
    // For `impl Sync`, `T: Send` is sufficient. (Bar is relaxed)
    ConcurrentQ,
    // Add more variants below..
    // Misc,
}

#[derive(Copy, Clone)]
#[repr(u8)]
enum CondBit {
    // At least 1 API of ADT takes `&self` within input and `&T`/`&mut T` within output.
    RefRef = 1,
    // `Clone` impl clones on generic param `T.
    Cloned = 1 << 1,
    // `T` only appears in ADT API input/output as owned `T`.
    // (Sender/Receiver side of Queue APIs)
    // e.g. `T`, `Box<T>`, `Option<T>`, `Result<T, !>`
    PassOwned = 1 << 2,
    // Add more conditions below if needed..
}
impl CondBit {
    // Upscale the return type if we need more than 8 bits to encode all conditions..
    fn repr(&self) -> u8 {
        *self as u8
    }
}

// Each bit of inner `u8` stands for whether a variant of `CondBit` holds true.
struct Cond(u8);
impl Cond {
    fn new() -> Self {
        Cond(0)
    }
    fn union(&mut self, x: CondBit) {
        self.0 |= x.repr();
    }
    fn is_ptrlike(&self) -> bool {
        // `RefRef` && !`Cloned`
        self.0 & 0b11 == 0b01
    }
    fn is_concurrent_q(&self) -> bool {
        // `PassOwned`
        self.0 & 0b100 == 0b100
    }
}

/// For each generic parameter (identified by index) of a given ADT,
/// inspect fn signature & body to identify `AdtBehavior`.
/// Inspects all methods of the given ADT, including methods from trait impls.
pub(crate) fn adt_behavior(tcx: TyCtxt, adt_did: DefId) -> FxHashMap<u32, AdtBehavior> {
    // Type of given ADT.
    let adt_ty = tcx.type_of(adt_did);

    // Map: (idx of generic parameter `T`) => (`Cond`)
    let mut cond_map = FxHashMap::default();

    // Set of all `T`s that appear as `&T` or `&mut T` in any of the API return types.
    let mut all_paramrefs_in_ret_ty = FxHashSet::default();

    // `clone`, `clone_from`
    let mut clone_method_dids: Option<Vec<DefId>> = None;
    let clone_trait_did = clone_trait_def_id(tcx).unwrap();

    // Set of `T`s that appear only as owned `T` in input||output of APIs.
    let mut owned_params = FxHashSet::default();
    // Set of `T`s that appear at least once as borrowed `T` in input||output of APIs.
    let mut ref_params = FxHashSet::default();

    // Inspect `impl`s relevant to the given ADT.
    for (impl_hir_id, item) in tcx.hir().krate().items.iter() {
        if let ItemKind::Impl {
            of_trait, self_ty, ..
        } = &item.kind
        {
            if !TyS::same_type(adt_ty, tcx.type_of(self_ty.hir_id.owner)) {
                continue;
            }

            // DefIds of methods within the given impl block.
            let method_dids = tcx
                .associated_items(impl_hir_id.owner)
                .in_definition_order()
                .filter_map(|assoc_item| {
                    if assoc_item.kind == AssocKind::Fn {
                        Some(assoc_item.def_id)
                    } else {
                        None
                    }
                });

            if let Some(trait_ref) = of_trait {
                if Some(clone_trait_did) == trait_ref.trait_def_id() {
                    clone_method_dids = Some(method_dids.collect());
                    continue;
                }
            }

            for fn_sig in method_dids.map(|did| tcx.fn_sig(did).skip_binder()) {
                if fn_takes_selfref(&fn_sig, adt_ty) {
                    // Check for `CondBit::RefRef`
                    for param_idx in paramrefs_in_ty(fn_sig.output()).into_iter() {
                        all_paramrefs_in_ret_ty.insert(param_idx);

                        cond_map
                            .entry(param_idx)
                            .or_insert(Cond::new())
                            .union(CondBit::RefRef);
                    }
                }

                // Bookkeeping for `CondBit::PassOwned`
                find_owned_params(tcx, &fn_sig, &mut owned_params, &mut ref_params);
            }
        }
    }

    // Inspect methods in `Clone` impl.
    if let Some(x) = clone_method_dids {
        let cloned_params = x
            .into_iter()
            .map(|method_did| find_cloned_params(tcx, method_did))
            .fold(FxHashSet::default(), |x, y| {
                x.union(&y).map(|item| *item).collect()
            });

        // Check for `CondBit::Cloned`
        for param_idx in cloned_params.into_iter() {
            if all_paramrefs_in_ret_ty.contains(&param_idx) {
                cond_map
                    .entry(param_idx)
                    .or_insert(Cond::new())
                    .union(CondBit::Cloned);
            }
        }
    }

    // Check for `CondBit::PassOwned`
    for &param_idx in owned_params.difference(&ref_params) {
        cond_map
            .entry(param_idx)
            .or_insert(Cond::new())
            .union(CondBit::PassOwned);
    }

    // Map: (idx of generic parameter) => (AdtBehavior)
    let mut behavior_map = FxHashMap::default();
    for (param_idx, cond) in cond_map.into_iter() {
        if cond.is_ptrlike() {
            behavior_map.insert(param_idx, AdtBehavior::PtrLike);
        } else if cond.is_concurrent_q() {
            behavior_map.insert(param_idx, AdtBehavior::ConcurrentQ);
        }
    }
    return behavior_map;
}

// Check if given fn takes `&self` within its first param type.
// e.g. `&self`, `Box<&self>`, `Pin<&self>`, ...
fn fn_takes_selfref(fn_sig: &FnSig, self_ty: Ty) -> bool {
    if fn_sig.inputs().is_empty() {
        return false;
    }

    let fn_1st_input = fn_sig.inputs()[0];
    let mut walker = fn_1st_input.walk();
    while let Some(node) = walker.next() {
        if let GenericArgKind::Type(ty) = node.unpack() {
            if let ty::TyKind::Ref(_, ty, Mutability::Not) = ty.kind {
                if TyS::same_type(self_ty, ty) {
                    return true;
                }
            }
        }
    }
    return false;
}

// Find all generic param `T`s that appear as `&T` or `&mut T` within given `Ty`.
fn paramrefs_in_ty(ty: Ty) -> FxHashSet<u32> {
    let mut ret = FxHashSet::default();

    let mut walker = ty.walk();
    while let Some(node) = walker.next() {
        if let GenericArgKind::Type(ty) = node.unpack() {
            if let ty::TyKind::Ref(_, ty, _) = ty.kind {
                if let ty::TyKind::Param(param_ty) = ty.kind {
                    ret.insert(param_ty.index);
                }
            }
        }
    }
    return ret;
}

// Inspect `clone` method body to find generic params that are actually cloned.
// This method works on `#[derive(Clone)]`, too.
fn find_cloned_params<'tcx>(tcx: TyCtxt<'tcx>, method_did: DefId) -> FxHashSet<u32> {
    const CLONE_FNS: [&str; 2] = ["std::clone::Clone::clone", "std::clone::Clone::clone_from"];

    let mut ret = FxHashSet::default();
    // Any fns called within `Clone` are also checked.
    let mut fn_stack = vec![method_did];

    while let Some(fn_did) = fn_stack.pop() {
        if let Ok(fn_body) = RudraCtxtOwner::find_fn(tcx, fn_did) {
            // Inspect terminators of each basic block to find calls to `clone()`.
            for bb in fn_body.basic_blocks() {
                let terminator = bb.terminator();
                if_chain! {
                    if let TerminatorKind::Call { func, args, .. } = &terminator.kind;
                    if let ty::TyKind::FnDef(callee_did, _) = func.ty(fn_body, tcx).kind;
                    if CLONE_FNS.contains(&&tcx.def_path_str(callee_did).as_str());
                    // Check type of the object being cloned.
                    let arg_ty = &args[0].ty(fn_body, tcx).kind;
                    if let ty::TyKind::Ref(_, ty, _) = arg_ty;
                    if let ty::TyKind::Param(param_ty) = ty.kind;
                    then {
                        ret.insert(param_ty.index);
                    }
                }
            }
        }
    }
    return ret;
}

// Params that appear at least once as owned `T` in API input/outputs => `owned_params`.
// Params that appear at least once as reference to `T` in API input/outputs => `ref_params`.
fn find_owned_params<'tcx>(
    tcx: TyCtxt<'tcx>,
    fn_sig: &FnSig,
    owned_params: &mut FxHashSet<u32>,
    ref_params: &mut FxHashSet<u32>,
) {
    let mut ty_stack = Vec::with_capacity(fn_sig.inputs().len() * 2);

    for paramty in fn_sig.inputs_and_output.iter() {
        match paramty.kind {
            ty::TyKind::Param(param_ty) => {
                owned_params.insert(param_ty.index);
            }
            ty::TyKind::Adt(def, substs) => {
                if def.is_box() {
                    let boxed_ty = substs.type_at(0);
                    if let ty::TyKind::Param(param_ty) = boxed_ty.kind {
                        // Box<T>
                        owned_params.insert(param_ty.index);
                    } else {
                        ty_stack.push(boxed_ty);
                    }
                }

                let def_path = tcx.def_path_str(def.did);
                // Check for `Option<T>` or `Result<T, ()>`.
                if def_path == "std::option::Option" || def_path == "std::result::Result" {
                    for x in substs.types() {
                        if let ty::TyKind::Param(param_ty) = x.kind {
                            owned_params.insert(param_ty.index);
                        }
                    }
                }
            }
            ty::TyKind::Array(ty, _) => {
                if let ty::TyKind::Param(param_ty) = ty.kind {
                    owned_params.insert(param_ty.index);
                } else {
                    ty_stack.push(ty);
                }
            }
            ty::TyKind::Tuple(substs) => {
                for ty in substs.types() {
                    if let ty::TyKind::Param(param_ty) = ty.kind {
                        owned_params.insert(param_ty.index);
                    } else {
                        ty_stack.push(ty);
                    }
                }
            }
            _ => {
                ty_stack.push(paramty);
            }
        }
    }

    while let Some(ty) = ty_stack.pop() {
        for param_idx in paramrefs_in_ty(ty).into_iter() {
            ref_params.insert(param_idx);
        }
    }
}
