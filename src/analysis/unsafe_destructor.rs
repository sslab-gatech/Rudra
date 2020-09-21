//! Unsafe destructor detector
use rustc_hir::def_id::DefId;
use rustc_hir::intravisit::{self, NestedVisitorMap, Visitor};
use rustc_hir::{Block, HirId, ImplItemId, ItemKind, Node};
use rustc_middle::ty::TyCtxt;

use snafu::{Backtrace, OptionExt, Snafu};

use crate::algorithm::LocalTraitIter;
use crate::prelude::*;
use crate::report::{Report, ReportLevel};

#[derive(Debug, Snafu)]
pub enum UnsafeDestructorError {
    DropTraitNotFound,
    InvalidHirId { backtrace: Backtrace },
    PushPopBlock { backtrace: Backtrace },
}

impl AnalysisError for UnsafeDestructorError {
    fn kind(&self) -> AnalysisErrorKind {
        use AnalysisErrorKind::*;
        use UnsafeDestructorError::*;
        match self {
            DropTraitNotFound => BrokenInvariant,
            InvalidHirId { .. } => BrokenInvariant,
            PushPopBlock { .. } => BrokenInvariant,
        }
    }
}

pub struct UnsafeDestructor<'tcx> {
    ccx: CruxCtxt<'tcx>,
}

impl<'tcx> UnsafeDestructor<'tcx> {
    pub fn new(ccx: CruxCtxt<'tcx>) -> Self {
        UnsafeDestructor { ccx }
    }

    pub fn analyze(&mut self) -> AnalysisOutputVec<'tcx> {
        fn drop_trait_def_id<'tcx>(tcx: TyCtxt<'tcx>) -> AnalysisResult<'tcx, DefId> {
            convert!(tcx.lang_items().drop_trait().context(DropTraitNotFound))
        }

        let mut vec = Vec::new();

        // key is DefId of trait, value is vec of HirId
        let mut visitor = visitor::UnsafeDestructorVisitor::new(self.ccx);

        let drop_trait_def_id =
            unwrap_or_return!(vec, drop_trait_def_id(self.ccx.tcx()), return vec);
        for impl_item in LocalTraitIter::new(self.ccx, drop_trait_def_id) {
            match visitor.check_drop_unsafety(impl_item, drop_trait_def_id) {
                Ok(true) => {
                    let tcx = self.ccx.tcx();
                    vec.push(Ok(Report::with_span(
                        tcx,
                        ReportLevel::Warning,
                        "UnsafeDestructor",
                        "unsafe block detected in drop",
                        tcx.hir().span(impl_item),
                    )));
                }
                Ok(false) => (),
                Err(e) => vec.push(Err(e)),
            }
        }

        vec.append(visitor.errors());
        vec
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
        errors: AnalysisOutputVec<'tcx>,
    }

    impl<'tcx> UnsafeDestructorVisitor<'tcx> {
        pub fn new(ccx: CruxCtxt<'tcx>) -> Self {
            UnsafeDestructorVisitor {
                ccx,
                drop_is_unsafe: false,
                errors: Vec::new(),
            }
        }

        pub fn errors(&mut self) -> &mut AnalysisOutputVec<'tcx> {
            &mut self.errors
        }

        /// Given an HIR ID of impl, checks whether `drop()` function contains
        /// unsafe or not. Returns `None` if the given HIR ID is not an impl for
        /// `Drop`.
        pub fn check_drop_unsafety(
            &mut self,
            hir_id: HirId,
            drop_trait_def_id: DefId,
        ) -> AnalysisResult<'tcx, bool> {
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
                    return Ok(self.check_drop_fn(drop_fn_impl_item_id));
                }
            }
            convert!(InvalidHirId.fail())
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
                _ => self.errors.push(convert!(PushPopBlock.fail())),
            }
            intravisit::walk_block(self, block);
        }
    }
}
