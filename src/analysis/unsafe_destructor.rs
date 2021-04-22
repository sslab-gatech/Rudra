//! Unsafe destructor detector
use rustc_hir::def_id::DefId;
use rustc_hir::intravisit::{self, NestedVisitorMap, Visitor};
use rustc_hir::{Block, BodyId, Expr, HirId, ImplItemId, ImplItemKind, ItemKind, Node, Unsafety};
use rustc_middle::ty::TyCtxt;

use snafu::{Backtrace, OptionExt, Snafu};

use crate::analysis::AnalysisKind;
use crate::iter::LocalTraitIter;
use crate::prelude::*;
use crate::report::{Report, ReportLevel};

#[derive(Debug, Snafu)]
pub enum UnsafeDestructorError {
    DropTraitNotFound,
    UnexpectedDropItem,
    InvalidHirId { backtrace: Backtrace },
    PushPopBlock { backtrace: Backtrace },
}

impl AnalysisError for UnsafeDestructorError {
    fn kind(&self) -> AnalysisErrorKind {
        use UnsafeDestructorError::*;
        match self {
            DropTraitNotFound => AnalysisErrorKind::Unreachable,
            UnexpectedDropItem => AnalysisErrorKind::Unreachable,
            InvalidHirId { .. } => AnalysisErrorKind::Unreachable,
            PushPopBlock { .. } => AnalysisErrorKind::Unreachable,
        }
    }
}

pub struct UnsafeDestructorChecker<'tcx> {
    rcx: RudraCtxt<'tcx>,
}

impl<'tcx> UnsafeDestructorChecker<'tcx> {
    pub fn new(rcx: RudraCtxt<'tcx>) -> Self {
        UnsafeDestructorChecker { rcx }
    }

    pub fn analyze(&mut self) {
        fn drop_trait_def_id<'tcx>(tcx: TyCtxt<'tcx>) -> AnalysisResult<'tcx, DefId> {
            convert!(tcx.lang_items().drop_trait().context(DropTraitNotFound))
        }

        // key is DefId of trait, value is vec of HirId
        let drop_trait_def_id = unwrap_or!(drop_trait_def_id(self.rcx.tcx()) => return);

        for impl_item in LocalTraitIter::new(self.rcx, drop_trait_def_id) {
            if inner::UnsafeDestructorVisitor::check_drop_unsafety(
                self.rcx,
                impl_item,
                drop_trait_def_id,
            ) {
                let tcx = self.rcx.tcx();
                rudra_report(Report::with_hir_id(
                    tcx,
                    ReportLevel::Warning,
                    AnalysisKind::UnsafeDestructor,
                    "unsafe block detected in drop",
                    impl_item,
                ));
            }
        }
    }
}

mod inner {
    use super::*;

    /// This struct finds the implementation for `Drop` trait implementation and
    /// checks if it contains any unsafe block.
    pub struct UnsafeDestructorVisitor<'tcx> {
        rcx: RudraCtxt<'tcx>,
        unsafe_nest_level: usize,
        unsafe_found: bool,
    }

    impl<'tcx> UnsafeDestructorVisitor<'tcx> {
        fn new(rcx: RudraCtxt<'tcx>) -> Self {
            UnsafeDestructorVisitor {
                rcx,
                unsafe_nest_level: 0,
                unsafe_found: false,
            }
        }

        /// Given an HIR ID of impl, checks whether `drop()` function contains
        /// unsafe or not. Returns false if the given HIR ID is invalid.
        pub fn check_drop_unsafety(
            rcx: RudraCtxt<'tcx>,
            hir_id: HirId,
            drop_trait_def_id: DefId,
        ) -> bool {
            let mut visitor = UnsafeDestructorVisitor::new(rcx);

            let map = visitor.rcx.tcx().hir();
            if_chain! {
                if let Some(node) = map.find(hir_id);
                if let Node::Item(item) = node;
                if let ItemKind::Impl { of_trait: Some(ref trait_ref), items, .. } = item.kind;
                if Some(drop_trait_def_id) == trait_ref.trait_def_id();
                then {
                    // `Drop` trait has only one required function.
                    if items.len() == 1 {
                        let drop_fn_item_ref = &items[0];
                        let drop_fn_impl_item_id = drop_fn_item_ref.id;
                        return visitor.check_impl_item(drop_fn_impl_item_id);
                    }
                    log_err!(UnexpectedDropItem);
                    return false;
                }
            }

            log_err!(InvalidHirId);
            false
        }

        fn check_impl_item(&mut self, impl_item_id: ImplItemId) -> bool {
            let impl_item = self.rcx.tcx().hir().impl_item(impl_item_id);
            if let ImplItemKind::Fn(_sig, body_id) = &impl_item.kind {
                self.check_body(*body_id)
            } else {
                log_err!(UnexpectedDropItem);
                false
            }
        }

        fn check_body(&mut self, body_id: BodyId) -> bool {
            self.unsafe_found = false;
            let body = self.rcx.tcx().hir().body(body_id);
            self.visit_body(body);
            self.unsafe_found
        }
    }

    impl<'tcx> Visitor<'tcx> for UnsafeDestructorVisitor<'tcx> {
        type Map = rustc_middle::hir::map::Map<'tcx>;

        fn nested_visit_map(&mut self) -> NestedVisitorMap<Self::Map> {
            NestedVisitorMap::OnlyBodies(self.rcx.tcx().hir())
        }

        fn visit_block(&mut self, block: &'tcx Block<'tcx>) {
            use rustc_hir::BlockCheckMode;
            match block.rules {
                BlockCheckMode::DefaultBlock => (),
                BlockCheckMode::UnsafeBlock(_unsafe_source) => {
                    self.unsafe_nest_level += 1;
                    intravisit::walk_block(self, block);
                    self.unsafe_nest_level -= 1;
                    return;
                }
                BlockCheckMode::PushUnsafeBlock(_) | BlockCheckMode::PopUnsafeBlock(_) => {
                    log_err!(PushPopBlock);
                }
            }
            intravisit::walk_block(self, block);
        }

        fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
            let tcx = self.rcx.tcx();
            if self.unsafe_nest_level > 0 {
                // If non-extern unsafe function call is detected in unsafe block
                if let Some(fn_def_id) = expr.ext().as_fn_def_id(tcx) {
                    let ty = tcx.type_of(fn_def_id);
                    if let Ok(Unsafety::Unsafe) = tcx.ext().fn_type_unsafety(ty) {
                        if !tcx.is_foreign_item(fn_def_id) {
                            self.unsafe_found = true;
                        }
                    }
                }
            }
            intravisit::walk_expr(self, expr);
        }
    }
}
