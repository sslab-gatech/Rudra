use rustc_hir::{
    def::Res,
    def_id::{CrateNum, DefId},
    Expr, ExprKind, Unsafety,
};
use rustc_middle::ty::{self, subst::GenericArg, Ty, TyCtxt};

use rustc_span::Symbol;
use snafu::{Backtrace, Snafu};
pub use snafu::{Error, ErrorCompat, IntoError, OptionExt, ResultExt};

pub use crate::analysis::{AnalysisError, AnalysisErrorKind, AnalysisResult};
pub use crate::context::RudraCtxt;
pub use crate::report::rudra_report;

#[derive(Debug, Snafu)]
pub enum ExtError {
    NonFunctionType { backtrace: Backtrace },
    InvalidOwner { backtrace: Backtrace },
    UnsupportedCall { backtrace: Backtrace },
    UnhandledCall { backtrace: Backtrace },
}

impl AnalysisError for ExtError {
    fn kind(&self) -> AnalysisErrorKind {
        use ExtError::*;
        match self {
            NonFunctionType { .. } => AnalysisErrorKind::Unreachable,
            InvalidOwner { .. } => AnalysisErrorKind::Unreachable,
            UnsupportedCall { .. } => AnalysisErrorKind::OutOfScope,
            UnhandledCall { .. } => AnalysisErrorKind::Unimplemented,
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
    pub fn fn_type_unsafety(self, ty: Ty<'tcx>) -> AnalysisResult<'tcx, Unsafety> {
        match ty.kind() {
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

    // `clippy_lints::utils::match_def_path` + rustc's `LateContext::match_def_path`
    /// Checks if the given def_id matches the path string.
    /// Prefer [`crate::paths::PathSet`] when comparing a single definition against multiple paths.
    pub fn match_def_path(self, def_id: DefId, syms: &[&str]) -> bool {
        let syms = syms
            .iter()
            .map(|p| Symbol::intern(p))
            .collect::<Vec<Symbol>>();

        let names = self.get_def_path(def_id);
        names.len() == syms.len() && names.into_iter().zip(syms.iter()).all(|(a, &b)| a == b)
    }

    // rustc's `LateContext::get_def_path`
    // This code is compiler version dependent, so it needs to be updated when we upgrade a compiler.
    // The current version is based on nightly-2021-10-21
    pub fn get_def_path(&self, def_id: DefId) -> Vec<Symbol> {
        use rustc_hir::definitions::{DefPathData, DisambiguatedDefPathData};
        use rustc_middle::ty::print::with_no_trimmed_paths;
        use ty::print::Printer;

        pub struct AbsolutePathPrinter<'tcx> {
            pub tcx: TyCtxt<'tcx>,
        }

        impl<'tcx> Printer<'tcx> for AbsolutePathPrinter<'tcx> {
            type Error = !;

            type Path = Vec<Symbol>;
            type Region = ();
            type Type = ();
            type DynExistential = ();
            type Const = ();

            fn tcx(&self) -> TyCtxt<'tcx> {
                self.tcx
            }

            fn print_region(self, _region: ty::Region<'_>) -> Result<Self::Region, Self::Error> {
                Ok(())
            }

            fn print_type(self, _ty: Ty<'tcx>) -> Result<Self::Type, Self::Error> {
                Ok(())
            }

            fn print_dyn_existential(
                self,
                _predicates: &'tcx ty::List<ty::Binder<'tcx, ty::ExistentialPredicate<'tcx>>>,
            ) -> Result<Self::DynExistential, Self::Error> {
                Ok(())
            }

            fn print_const(self, _ct: &'tcx ty::Const<'tcx>) -> Result<Self::Const, Self::Error> {
                Ok(())
            }

            fn path_crate(self, cnum: CrateNum) -> Result<Self::Path, Self::Error> {
                Ok(vec![self.tcx.crate_name(cnum)])
            }

            fn path_qualified(
                self,
                self_ty: Ty<'tcx>,
                trait_ref: Option<ty::TraitRef<'tcx>>,
            ) -> Result<Self::Path, Self::Error> {
                if trait_ref.is_none() {
                    if let ty::Adt(def, substs) = self_ty.kind() {
                        return self.print_def_path(def.did, substs);
                    }
                }

                // This shouldn't ever be needed, but just in case:
                with_no_trimmed_paths(|| {
                    Ok(vec![match trait_ref {
                        Some(trait_ref) => Symbol::intern(&format!("{:?}", trait_ref)),
                        None => Symbol::intern(&format!("<{}>", self_ty)),
                    }])
                })
            }

            fn path_append_impl(
                self,
                print_prefix: impl FnOnce(Self) -> Result<Self::Path, Self::Error>,
                _disambiguated_data: &DisambiguatedDefPathData,
                self_ty: Ty<'tcx>,
                trait_ref: Option<ty::TraitRef<'tcx>>,
            ) -> Result<Self::Path, Self::Error> {
                let mut path = print_prefix(self)?;

                // This shouldn't ever be needed, but just in case:
                path.push(match trait_ref {
                    Some(trait_ref) => with_no_trimmed_paths(|| {
                        Symbol::intern(&format!(
                            "<impl {} for {}>",
                            trait_ref.print_only_trait_path(),
                            self_ty
                        ))
                    }),
                    None => {
                        with_no_trimmed_paths(|| Symbol::intern(&format!("<impl {}>", self_ty)))
                    }
                });

                Ok(path)
            }

            fn path_append(
                self,
                print_prefix: impl FnOnce(Self) -> Result<Self::Path, Self::Error>,
                disambiguated_data: &DisambiguatedDefPathData,
            ) -> Result<Self::Path, Self::Error> {
                let mut path = print_prefix(self)?;

                // Skip `::{{constructor}}` on tuple/unit structs.
                if let DefPathData::Ctor = disambiguated_data.data {
                    return Ok(path);
                }

                path.push(Symbol::intern(&disambiguated_data.data.to_string()));
                Ok(path)
            }

            fn path_generic_args(
                self,
                print_prefix: impl FnOnce(Self) -> Result<Self::Path, Self::Error>,
                _args: &[GenericArg<'tcx>],
            ) -> Result<Self::Path, Self::Error> {
                print_prefix(self)
            }
        }

        AbsolutePathPrinter { tcx: self.tcx }
            .print_def_path(def_id, &[])
            .unwrap()
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
                ExprKind::Field(..) => {
                    // Example: (self.0)(self.1, self.2);
                    log_err!(UnsupportedCall);
                    None
                }
                _ => {
                    log_err!(UnhandledCall);
                    None
                }
            },
            ExprKind::MethodCall(..) => typeck_tables.type_dependent_def_id(self.expr.hir_id),
            // expected failure, silent
            _ => None,
        }
    }
}
