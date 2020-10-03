use rustc_hir::def::Res;
use rustc_hir::def_id::DefId;
use rustc_hir::{Expr, ExprKind, Unsafety};
use rustc_middle::ty::{self, subst::SubstsRef, Instance, ParamEnv, Ty, TyCtxt};

use snafu::{Backtrace, Snafu};
pub use snafu::{Error, ErrorCompat, IntoError, ResultExt};

pub use crate::analysis::{AnalysisError, AnalysisErrorKind, AnalysisResult};
pub use crate::context::RudraCtxt;
pub use crate::report::rudra_report;

#[derive(Debug, Snafu)]
pub enum ExtError {
    PathExpected { backtrace: Backtrace },
    NonFunctionType { backtrace: Backtrace },
    InvalidOwner { backtrace: Backtrace },
    UnhandledCall { backtrace: Backtrace },
}

impl AnalysisError for ExtError {
    fn kind(&self) -> AnalysisErrorKind {
        use AnalysisErrorKind::*;
        use ExtError::*;
        match self {
            PathExpected { .. } => Unreachable,
            NonFunctionType { .. } => Unreachable,
            InvalidOwner { .. } => Unreachable,
            UnhandledCall { .. } => Unimplemented,
        }
    }
}

pub trait TyCtxtExt<'tcx> {
    fn ext(self) -> TyCtxtExtension<'tcx>;
}

impl<'tcx> TyCtxtExt<'tcx> for TyCtxt<'tcx> {
    fn ext(self) -> TyCtxtExtension<'tcx> {
        TyCtxtExtension { tcx: self }
    }
}

#[derive(Clone, Copy)]
pub struct TyCtxtExtension<'tcx> {
    tcx: TyCtxt<'tcx>,
}

impl<'tcx> TyCtxtExtension<'tcx> {
    pub fn monomorphic_resolve(
        self,
        callee_def_id: DefId,
        callee_substs: SubstsRef<'tcx>,
        caller_substs: SubstsRef<'tcx>,
    ) -> Option<Instance<'tcx>> {
        let replaced_substs = self.tcx.subst_and_normalize_erasing_regions(
            caller_substs,
            ParamEnv::reveal_all(),
            &callee_substs,
        );
        Instance::resolve(
            self.tcx,
            ParamEnv::reveal_all(),
            callee_def_id,
            replaced_substs,
        )
        .unwrap()
    }

    pub fn fn_type_unsafety_instance(
        self,
        instance: Instance<'tcx>,
    ) -> AnalysisResult<'tcx, Unsafety> {
        self.fn_type_unsafety(instance.ty(self.tcx, ParamEnv::reveal_all()))
    }

    pub fn fn_type_unsafety(self, ty: Ty<'tcx>) -> AnalysisResult<'tcx, Unsafety> {
        match ty.kind {
            ty::FnDef(..) | ty::FnPtr(_) => {
                let sig = ty.fn_sig(self.tcx);
                Ok(sig.unsafety())
            }
            ty::Closure(_def_id, substs) => {
                let sig = substs.as_closure().sig();
                Ok(sig.unsafety())
            }
            _ => convert!(NonFunctionType.fail()),
        }
    }
}

pub trait ExprExt<'tcx> {
    fn ext(self) -> ExprExtension<'tcx>;
}

impl<'tcx> ExprExt<'tcx> for &'tcx Expr<'tcx> {
    fn ext(self) -> ExprExtension<'tcx> {
        ExprExtension { expr: self }
    }
}

#[derive(Clone, Copy)]
pub struct ExprExtension<'tcx> {
    expr: &'tcx Expr<'tcx>,
}

impl<'tcx> ExprExtension<'tcx> {
    /// Returns `Some(def_id)` if expression is a function
    /// Returns `None` if expression is not a function or error happens
    pub fn as_fn_def_id(self, tcx: TyCtxt<'tcx>) -> Option<DefId> {
        if !tcx.has_typeck_results(self.expr.hir_id.owner) {
            log_err!(InvalidOwner);
            return None;
        }

        let typeck_tables = tcx.typeck(self.expr.hir_id.owner);
        trace!("as_fn_def_id() on {:?}", self.expr);
        match self.expr.kind {
            ExprKind::Call(path_expr, _args) => match &path_expr.kind {
                ExprKind::Path(path) => {
                    let res = typeck_tables.qpath_res(path, path_expr.hir_id);
                    match res {
                        Res::Def(_def_kind, def_id) => Some(def_id),
                        _ => {
                            log_err!(UnhandledCall);
                            None
                        }
                    }
                }
                _ => {
                    log_err!(PathExpected);
                    None
                }
            },
            ExprKind::MethodCall(..) => typeck_tables.type_dependent_def_id(self.expr.hir_id),
            // expected failure, silent
            _ => None,
        }
    }
}
