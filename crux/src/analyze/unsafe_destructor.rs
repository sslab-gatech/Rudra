//! Unsafe destructor detector
use rustc_hir::def_id::{DefId, LOCAL_CRATE};
use rustc_hir::intravisit::{self, NestedVisitorMap, Visitor};
use rustc_hir::{Block, HirId, ImplItemId, ItemKind, Node};
use rustc_middle::ty::TyCtxt;

use crate::error::Result;
use crate::prelude::*;

fn drop_trait_def_id(tcx: TyCtxt<'_>) -> DefId {
    tcx.lang_items()
        .drop_trait()
        .expect("Drop lang item should always exist")
}
pub struct UnsafeDestructor<'tcx> {
    ccx: CruxCtxt<'tcx>,
}

impl<'tcx> UnsafeDestructor<'tcx> {
    pub fn new(ccx: CruxCtxt<'tcx>) -> Self {
        UnsafeDestructor { ccx }
    }

    pub fn analyze(&mut self) -> Result<'tcx, ()> {
        // key is DefId of trait, value is vec of HirId
        let local_trait_impl_map = self.ccx.tcx().all_local_trait_impls(LOCAL_CRATE);
        let drop_trait_def_id = drop_trait_def_id(self.ccx.tcx());

        let mut visitor = visitor::UnsafeDestructorVisitor::new(self.ccx);

        for (trait_def_id, impl_vec) in local_trait_impl_map.iter() {
            if *trait_def_id == drop_trait_def_id {
                for impl_hir_id in impl_vec.iter() {
                    if visitor.check_drop_unsafety(*impl_hir_id).unwrap() {
                        // FIXME: correctly report error here
                        error!("Unsafe drop implementation detected!");
                    }
                }
            }
        }

        Ok(())
    }
}

mod visitor {
    use super::*;

    /// This struct finds the implementation for `Drop` trait implementation and
    /// checks if it contains any unsafe block. This approach will provide a lot of
    /// false positives, and heuristics to remove them will be added in the future.
    pub struct UnsafeDestructorVisitor<'tcx> {
        ccx: CruxCtxt<'tcx>,
        drop_is_unsafe: bool,
    }

    impl<'tcx> UnsafeDestructorVisitor<'tcx> {
        pub fn new(ccx: CruxCtxt<'tcx>) -> Self {
            UnsafeDestructorVisitor {
                ccx,
                drop_is_unsafe: false,
            }
        }

        /// Given an HIR ID of impl, checks whether `drop()` function contains
        /// unsafe or not. Returns `None` if the given HIR ID is not an impl for
        /// `Drop`.
        pub fn check_drop_unsafety(&mut self, hir_id: HirId) -> Option<bool> {
            let drop_trait_def_id = drop_trait_def_id(self.ccx.tcx());
            let map = self.ccx.tcx().hir();
            if_chain! {
                if let Some(node) = map.find(hir_id);
                if let Node::Item(item) = node;
                if let ItemKind::Impl { of_trait: Some(ref trait_ref), items, .. } = item.kind;
                if Some(drop_trait_def_id) == trait_ref.trait_def_id();
                then {
                    // `Drop` trait has only one required function.
                    assert_eq!(items.len(), 1);
                    let drop_fn_item_ref = &items[0];
                    let drop_fn_impl_item_id = drop_fn_item_ref.id;
                    return Some(self.check_drop_fn(drop_fn_impl_item_id));
                }
            }
            None
        }

        fn check_drop_fn(&mut self, drop_fn_impl_item_id: ImplItemId) -> bool {
            self.drop_is_unsafe = false;
            self.visit_nested_impl_item(drop_fn_impl_item_id);
            self.drop_is_unsafe
        }
    }

    impl<'tcx> Visitor<'tcx> for UnsafeDestructorVisitor<'tcx> {
        type Map = rustc_middle::hir::map::Map<'tcx>;

        fn nested_visit_map(&mut self) -> NestedVisitorMap<Self::Map> {
            NestedVisitorMap::All(self.ccx.tcx().hir())
        }

        fn visit_block(&mut self, block: &'tcx Block<'tcx>) {
            use rustc_hir::BlockCheckMode;
            match block.rules {
                BlockCheckMode::DefaultBlock => (),
                BlockCheckMode::UnsafeBlock(_source) => {
                    // TODO: implement heuristic analysis
                    self.drop_is_unsafe = true;
                }
                _ => panic!("push/pop unsafe should not exist"),
            }
            intravisit::walk_block(self, block);
        }
    }
}
