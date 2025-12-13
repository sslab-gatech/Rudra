use rustc_data_structures::fx::FxHashMap;
use rustc_hir::{
    def_id::{DefId, LocalDefId},
    intravisit::{self},
    Block, BodyId, HirId, Impl, ItemKind,
};
use rustc_middle::hir::nested_filter;
use rustc_middle::ty::{Ty, TyCtxt, TyKind};
use rustc_span::Span;

/// Maps `HirId` of a type to `BodyId` of related impls.
/// Free-standing (top level) functions and default trait impls have `None` as a key.
pub type RelatedItemMap = FxHashMap<Option<HirId>, Vec<(BodyId, Span)>>;

/// Creates `AdtItemMap` with the given HIR map.
/// You might want to use `RudraCtxt`'s `related_item_cache` field instead of
/// directly using this collector.
pub struct RelatedFnCollector<'tcx> {
    tcx: TyCtxt<'tcx>,
    hash_map: RelatedItemMap,
}

impl<'tcx> RelatedFnCollector<'tcx> {
    pub fn collect(tcx: TyCtxt<'tcx>) -> RelatedItemMap {
        let mut collector = RelatedFnCollector {
            tcx,
            hash_map: RelatedItemMap::default(),
        };

        // In newer rustc, we iterate over items directly
        for item_id in tcx.hir_crate_items(()).free_items() {
            let item = tcx.hir_item(item_id);
            collector.visit_item(item);
        }

        collector.hash_map
    }

    fn visit_item(&mut self, item: &'tcx rustc_hir::Item<'tcx>) {
        let tcx = self.tcx;
        match &item.kind {
            ItemKind::Impl(Impl {
                safety: _safety,
                generics: _generics,
                self_ty,
                items: impl_items,
                ..
            }) => {
                let key = Some(self_ty.hir_id);
                let entry = self.hash_map.entry(key).or_insert(Vec::new());
                entry.extend(impl_items.iter().filter_map(|impl_item_ref| {
                    let local_def_id = impl_item_ref.id.owner_id.def_id;
                    tcx
                        .hir_maybe_body_owned_by(local_def_id)
                        .map(|body| (body.id(), impl_item_ref.span))
                }));
            }
            // Free-standing (top level) functions and default trait impls have `None` as a key.
            ItemKind::Trait(_is_auto, _safety, _generics, _generic_bounds, trait_items) => {
                let key = None;
                let entry = self.hash_map.entry(key).or_insert(Vec::new());
                entry.extend(trait_items.iter().filter_map(|trait_item_ref| {
                    let local_def_id = trait_item_ref.id.owner_id.def_id;
                    tcx
                        .hir_maybe_body_owned_by(local_def_id)
                        .map(|body| (body.id(), trait_item_ref.span))
                }));
            }
            ItemKind::Fn { body, .. } => {
                let key = None;
                let entry = self.hash_map.entry(key).or_insert(Vec::new());
                entry.push((*body, item.span));
            }
            _ => (),
        }
    }
}

pub struct ContainsUnsafe<'tcx> {
    tcx: TyCtxt<'tcx>,
    contains_unsafe: bool,
}

impl<'tcx> ContainsUnsafe<'tcx> {
    /// Given a `BodyId`, returns if the corresponding body contains unsafe code in it.
    /// Note that it only checks the function body, so this function will return false for
    /// body ids of functions that are defined as unsafe.
    pub fn contains_unsafe(tcx: TyCtxt<'tcx>, body_id: BodyId) -> bool {
        use intravisit::Visitor;

        let mut visitor = ContainsUnsafe {
            tcx,
            contains_unsafe: false,
        };

        let body = visitor.tcx.hir_body(body_id);
        visitor.visit_body(body);

        visitor.contains_unsafe
    }
}

impl<'tcx> intravisit::Visitor<'tcx> for ContainsUnsafe<'tcx> {
    type NestedFilter = nested_filter::OnlyBodies;

    fn maybe_tcx(&mut self) -> Self::MaybeTyCtxt {
        self.tcx
    }

    fn visit_block(&mut self, block: &'tcx Block<'tcx>) {
        use rustc_hir::BlockCheckMode;
        if let BlockCheckMode::UnsafeBlock(_unsafe_source) = block.rules {
            self.contains_unsafe = true;
        }
        intravisit::walk_block(self, block);
    }
}

/// (`DefId` of ADT) => Vec<(HirId of relevant impl block, impl_self_ty)>
/// We use this map to quickly access associated impl blocks per ADT.
/// `impl_self_ty` in the return value may differ from `tcx.type_of(ADT.DefID)`,
/// as different instantiations of the same ADT are distinct `Ty`s.
/// (e.g. Foo<i32, i64>, Foo<String, i32>)
pub type AdtImplMap<'tcx> = FxHashMap<DefId, Vec<(LocalDefId, Ty<'tcx>)>>;

/// Create & initialize `AdtImplMap`.
/// `AdtImplMap` is initialized before analysis of each crate,
/// avoiding quadratic complexity of scanning all impl blocks for each ADT.
pub fn create_adt_impl_map<'tcx>(tcx: TyCtxt<'tcx>) -> AdtImplMap<'tcx> {
    let mut map = FxHashMap::default();

    for item_id in tcx.hir_crate_items(()).free_items() {
        let item = tcx.hir_item(item_id);
        if let ItemKind::Impl(Impl { self_ty, .. }) = item.kind {
            // `Self` type of the given impl block.
            let impl_self_ty = tcx.type_of(self_ty.hir_id.owner.def_id).instantiate_identity();

            if let TyKind::Adt(impl_self_adt_def, _impl_args) = impl_self_ty.kind() {
                // We use `AdtDef.did()` as key for `AdtImplMap`.
                // For any crazy instantiation of the same generic ADT (Foo<i32>, Foo<String>, etc..),
                // `AdtDef.did()` refers to the original ADT definition.
                // Thus it can be used to map & collect impls for all instantitations of the same ADT.

                map.entry(impl_self_adt_def.did())
                    .or_insert_with(|| Vec::new())
                    .push((item.owner_id.def_id, impl_self_ty));
            }
        }
    }

    map
}
