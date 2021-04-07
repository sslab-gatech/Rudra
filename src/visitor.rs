use rustc_data_structures::fx::FxHashMap;
use rustc_hir::{
    def_id::DefId, intravisit, itemlikevisit::ItemLikeVisitor, Block, BodyId, HirId, ItemKind,
};
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

        tcx.hir().krate().visit_all_item_likes(&mut collector);

        collector.hash_map
    }
}

impl<'tcx> ItemLikeVisitor<'tcx> for RelatedFnCollector<'tcx> {
    fn visit_item(&mut self, item: &'tcx rustc_hir::Item<'tcx>) {
        let hir_map = self.tcx.hir();
        match &item.kind {
            ItemKind::Impl {
                unsafety: _unsafety,
                generics: _generics,
                self_ty,
                items: impl_items,
                ..
            } => {
                let key = Some(self_ty.hir_id);
                let entry = self.hash_map.entry(key).or_insert(Vec::new());
                entry.extend(impl_items.iter().filter_map(|impl_item_ref| {
                    let hir_id = impl_item_ref.id.hir_id;
                    hir_map
                        .maybe_body_owned_by(hir_id)
                        .map(|body_id| (body_id, impl_item_ref.span))
                }));
            }
            // Free-standing (top level) functions and default trait impls have `None` as a key.
            ItemKind::Trait(_is_auto, _unsafety, _generics, _generic_bounds, trait_items) => {
                let key = None;
                let entry = self.hash_map.entry(key).or_insert(Vec::new());
                entry.extend(trait_items.iter().filter_map(|trait_item_ref| {
                    let hir_id = trait_item_ref.id.hir_id;
                    hir_map
                        .maybe_body_owned_by(hir_id)
                        .map(|body_id| (body_id, trait_item_ref.span))
                }));
            }
            ItemKind::Fn(_fn_sig, _generics, body_id) => {
                let key = None;
                let entry = self.hash_map.entry(key).or_insert(Vec::new());
                entry.push((*body_id, item.span));
            }
            _ => (),
        }
    }

    fn visit_trait_item(&mut self, _trait_item: &'tcx rustc_hir::TraitItem<'tcx>) {
        // We don't process items inside trait blocks
    }

    fn visit_impl_item(&mut self, _impl_item: &'tcx rustc_hir::ImplItem<'tcx>) {
        // We don't process items inside impl blocks
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

        let body = visitor.tcx.hir().body(body_id);
        visitor.visit_body(body);

        visitor.contains_unsafe
    }
}

impl<'tcx> intravisit::Visitor<'tcx> for ContainsUnsafe<'tcx> {
    type Map = rustc_middle::hir::map::Map<'tcx>;

    fn nested_visit_map(&mut self) -> intravisit::NestedVisitorMap<Self::Map> {
        intravisit::NestedVisitorMap::OnlyBodies(self.tcx.hir())
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
pub type AdtImplMap<'tcx> = FxHashMap<DefId, Vec<(&'tcx HirId, Ty<'tcx>)>>;

/// Create & initialize `AdtImplMap`.
/// `AdtImplMap` is initialized before analysis of each crate,
/// avoiding quadratic complexity of scanning all impl blocks for each ADT.
pub fn create_adt_impl_map<'tcx>(tcx: TyCtxt<'tcx>) -> AdtImplMap<'tcx> {
    let mut map = FxHashMap::default();

    for (impl_hir_id, item) in tcx.hir().krate().items.iter() {
        if let ItemKind::Impl { self_ty, .. } = &item.kind {
            // `Self` type of the given impl block.
            let impl_self_ty = tcx.type_of(self_ty.hir_id.owner);

            if let TyKind::Adt(impl_self_adt_def, _impl_substs) = impl_self_ty.kind {
                // We use `AdtDef.did` as key for `AdtImplMap`.
                // For any crazy instantiation of the same generic ADT (Foo<i32>, Foo<String>, etc..),
                // `AdtDef.did` refers to the original ADT definition.
                // Thus it can be used to map & collect impls for all instantitations of the same ADT.

                map.entry(impl_self_adt_def.did)
                    .or_insert_with(|| Vec::new())
                    .push((impl_hir_id, impl_self_ty));
            }
        }
    }

    map
}
