//! Unsafe Send/Sync impl detector (strict)

use super::*;

impl<'tcx> SendSyncChecker<'tcx> {
    /// Detect suspicious Sync with strict rules.
    /// Report if any of the generic parameters of `impl Sync` aren't Sync.
    pub fn suspicious_sync(
        &self,
        // HirId of the `Impl Sync` item
        hir_id: HirId,
        sync_trait_def_id: DefId,
    ) -> Option<DefId> {
        let map = self.rcx.tcx().hir();
        if_chain! {
            if let Some(node) = map.find(hir_id);
            if let Node::Item(item) = node;
            if let ItemKind::Impl {
                ref generics,
                of_trait: Some(ref trait_ref),
                self_ty,
                ..
            } = item.kind;
            if Some(sync_trait_def_id) == trait_ref.trait_def_id();
            if let Some((struct_def_id, struct_fields)) = fetch_structfields(&map, &self_ty);
            then {
                // If `impl Sync` doesn't involve generic parameters, don't catch it.
                if generics.params.len() == 0 {
                    return None;
                }

                // Find indices of generic params which are enclosed inside PhantomType<T>
                let phantom_indices = self.phantom_indices(struct_fields, struct_def_id);

                // At the end, this set contains `Symbol.as_u32()`s of generic params that aren't `Sync`
                let mut suspicious_generic_params = FxHashSet::default();

                // Inspect immediate trait bounds on generic parameters
                // to initialize set of suspects that may not be `Sync`
                self.initialize_suspects(
                    &[sync_trait_def_id],
                    generics.params,
                    &mut suspicious_generic_params,
                    &phantom_indices[..],
                );

                // Inspect trait bounds in where clause.
                // Filter out suspects that have `Sync` bound in where clause.
                self.filter_suspects(
                    &[sync_trait_def_id],
                    generics.where_clause.predicates,
                    &mut suspicious_generic_params,
                );

                return if suspicious_generic_params.is_empty() {
                    None
                } else {
                    Some(struct_def_id)
                };
            }
        }
        return None;
    }

    pub fn suspicious_send(
        &self,
        hir_id: HirId,
        send_trait_def_id: DefId,
        sync_trait_def_id: DefId,
        copy_trait_def_id: DefId,
    ) -> Option<DefId> {
        let map = self.rcx.tcx().hir();
        if_chain! {
            if let Some(node) = map.find(hir_id);
            if let Node::Item(item) = node;
            if let ItemKind::Impl {
                ref generics,
                of_trait: Some(ref trait_ref),
                self_ty,
                ..
            } = item.kind;
            if Some(send_trait_def_id) == trait_ref.trait_def_id();
            if let Some((struct_def_id, struct_fields)) = fetch_structfields(&map, &self_ty);
            then {
                // If `impl Send` doesn't involve generic parameters, don't catch it.
                if generics.params.len() == 0 {
                    return None;
                }

                // Find indices of generic params which are enclosed inside PhantomType<T>
                let phantom_indices = self.phantom_indices(struct_fields, struct_def_id);

                // At the end, this set should only contain `Symbol.as_u32()`s of generic params
                // which may cause safety issues in the `Send` impl.
                let mut suspicious_generic_params = FxHashSet::default();

                // Inspect immediate trait bounds on generic parameters
                // to initialize set of suspects that may not be `Send`
                self.initialize_suspects(
                    &[send_trait_def_id, sync_trait_def_id, copy_trait_def_id],
                    generics.params,
                    &mut suspicious_generic_params,
                    &phantom_indices[..]
                );

                // Inspect trait bounds in `where` clause.
                // Filter out suspects that are `Send` or `Copy` in where clause.
                self.filter_suspects(
                    &[send_trait_def_id, sync_trait_def_id, copy_trait_def_id],
                    generics.where_clause.predicates,
                    &mut suspicious_generic_params
                );

                return if suspicious_generic_params.is_empty() {
                    None
                } else {
                    Some(struct_def_id)
                };
            }
        }
        return None;
    }

    /// To `suspicious_generic_params`,
    /// insert generic parameters that don't have a bound included in `target_trait_def_ids`
    fn initialize_suspects(
        &self,
        target_trait_def_ids: &[DefId],
        generic_params: &[GenericParam],
        suspicious_generic_params: &mut FxHashSet<u32>,
        phantom_indices: &[u32],
    ) {
        // Inspect immediate trait bounds on generic parameters
        for (idx, generic_param) in generic_params.iter().enumerate() {
            if phantom_indices.contains(&(idx as u32)) {
                continue;
            }
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

    /// For a given struct,
    /// return the indices of `T`s that are inside `PhantomData<T>`
    fn phantom_indices(&self, struct_fields: &[StructField], struct_did: DefId) -> Vec<u32> {

        // dirty helper fn to find generic parameters nested within given `TyKind`s.
        // returns a vector that include DefIds of embedded generic parameters.
        // note: return vector can include DefIds of non-leaf inner nodes
        //       (e.g. DefId of `Option<T>` from `Box<Option<T>>`)
        fn dig(mut ty_kinds: Vec<&TyKind>) -> Vec<DefId> {
            let mut phantom_params = vec![];

            use TyKind::*;
            while let Some(ty_kind) = ty_kinds.pop() {
                match ty_kind {
                    Ptr(mut_ty) | Rptr(_, mut_ty) => ty_kinds.push(&mut_ty.ty.kind),
                    Slice(ty) | Array(ty, _) => ty_kinds.push(&ty.kind),
                    Tup(ty_arr) => {
                        for ty in ty_arr.iter() {
                            ty_kinds.push(&ty.kind);
                        }
                    }
                    Path(QPath::Resolved(_, path)) => {
                        if let Def(x, did) = path.res {
                            phantom_params.push(did);

                            use DefKind::*;
                            if x == Struct || x == Enum || x == Union {
                                for segment in path.segments {
                                    for gen_arg in segment.generic_args().args {
                                        if let GenericArg::Type(ty) = gen_arg {
                                            ty_kinds.push(&ty.kind);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            return phantom_params;
        }

        let mut ty_kinds = vec![];
        for x in struct_fields {
            if let TyKind::Path(QPath::Resolved(_, b)) = x.ty.kind {
                if let Def(DefKind::Struct, phantom_did) = b.res {
                    let type_name = self.rcx.tcx().item_name(phantom_did).to_ident_string();
                    if type_name == "PhantomData" {
                        for segment in b.segments {
                            for gen_arg in segment.generic_args().args {
                                if let GenericArg::Type(ty) = gen_arg {
                                    ty_kinds.push(&ty.kind);
                                }
                            }
                        }
                    }
                }
            }
        }
        let phantom_params = dig(ty_kinds);
        let mut phantom_indices = vec![];
        for struct_generic_param in &self.rcx.tcx().generics_of(struct_did).params {
            if phantom_params.contains(&struct_generic_param.def_id) {
                phantom_indices.push(struct_generic_param.index);
            }
        }
        return phantom_indices;
    }
}

/// Using the given HIR map & type info,
/// return Option<(`DefId` of struct, &[StructField])>
fn fetch_structfields<'tcx>(
    map: &'tcx Map,
    struct_ty: &Ty,
) -> Option<(DefId, &'tcx [StructField<'tcx>])> {
    if_chain! {
        if let TyKind::Path(QPath::Resolved(_, path)) = struct_ty.kind;
        if let rustc_hir::def::Res::Def(_, struct_def_id) = path.res;
        if let Some(local_def_id) = struct_def_id.as_local();
        let hir_id_of_struct = map.local_def_id_to_hir_id(local_def_id);
        if let Some(Node::Item(ref struct_item)) = map.find(hir_id_of_struct);
        if let ItemKind::Struct(x, _) = &struct_item.kind;
        then {
            match x {
                VariantData::Struct(struct_fields, _) | VariantData::Tuple(struct_fields, _) => {
                    Some((struct_def_id, struct_fields))
                }
                _ => None
            }
        } else {
            None
        }
    }
}
