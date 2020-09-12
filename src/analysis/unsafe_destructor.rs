//! Unsafe destructor detector
use rustc_hir::def_id::DefId;
use rustc_hir::intravisit::{self, NestedVisitorMap, Visitor};
use rustc_hir::{Block, HirId, ImplItemId, ItemKind, Node};
use rustc_middle::ty::TyCtxt;

use crate::algorithm::LocalTraitIter;
use crate::error::Result;
use crate::prelude::*;
use crate::report::{crux_report, Report, ReportLevel};

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
        let mut visitor = visitor::UnsafeDestructorVisitor::new(self.ccx);

        let drop_trait_def_id = drop_trait_def_id(self.ccx.tcx());
        for impl_item in LocalTraitIter::new(self.ccx, drop_trait_def_id) {
            if visitor.check_drop_unsafety(impl_item).unwrap() {
                let tcx = self.ccx.tcx();
                crux_report(Report::with_span(
                    self.ccx.tcx(),
                    ReportLevel::Warning,
                    "UnsafeDestructor",
                    "unsafe block detected in drop",
                    tcx.hir().span(impl_item),
                ));
            }
        }

        Ok(())
    }
}

mod visitor {
    use super::*;

    /// This struct finds the implementation for `Drop` trait implementation and
    /// checks if it contains any unsafe block. This approach will provide false
    /// positives, which are pruned by heuristics.
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
                BlockCheckMode::UnsafeBlock(_unsafe_source) => {
                    // TODO: implement heuristic analysis
                    self.drop_is_unsafe = true;
                }
                _ => panic!("push/pop unsafe should not exist"),
            }
            intravisit::walk_block(self, block);
        }
    }
}
