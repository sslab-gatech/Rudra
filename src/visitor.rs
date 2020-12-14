use rustc_data_structures::fx::FxHashMap;
use rustc_hir::{intravisit, itemlikevisit::ItemLikeVisitor, Block, BodyId, HirId, ItemKind};
use rustc_middle::ty::TyCtxt;

/// Creates `AdtItemMap` with the given HIR map.
/// You might want to use `RudraCtxt`'s `related_item_cache` field instead of
/// directly using this collector.
pub struct RelatedFnCollector<'tcx> {
    tcx: TyCtxt<'tcx>,
    hash_map: RelatedItemMap,
}

/// Maps `HirId` of a type to `BodyId` of related impls.
pub type RelatedItemMap = FxHashMap<HirId, Vec<BodyId>>;

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
        match &item.kind {
            ItemKind::Impl {
                unsafety: _unsafety,
                generics: _generics,
                self_ty,
                items: impl_items,
                ..
            } => {
                let hir_map = self.tcx.hir();
                let key = self_ty.hir_id;
                let entry = self.hash_map.entry(key).or_insert(Vec::new());
                entry.extend(impl_items.iter().filter_map(|impl_item_ref| {
                    let hir_id = impl_item_ref.id.hir_id;
                    hir_map.maybe_body_owned_by(hir_id)
                }));
            }
            // We currently don't collect freestanding functions and default implementations
            // in trait definitions as related items to the type. In long term
            // we should consider them, but it's probably okay for now since
            // they tend to use unsafe less often than other code.
            ItemKind::Trait(_is_auto, _unsafety, _generics, _generic_bounds, _trait_item_ref) => (),
            ItemKind::Fn(_fn_sig, _generics, _body_id) => (),
            _ => (),
        }
    }

    fn visit_trait_item(&mut self, _trait_item: &'tcx rustc_hir::TraitItem<'tcx>) {
        // We don't process items inside trait blocks here
    }

    fn visit_impl_item(&mut self, _impl_item: &'tcx rustc_hir::ImplItem<'tcx>) {
        // We don't process items inside impl blocks here
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
