use rustc_hir::{
    def::Res,
    def_id::{CrateNum, DefId},
    Expr, ExprKind, Safety,
};
use rustc_middle::ty::{self, GenericArg, Ty, TyCtxt};
use rustc_middle::ty::print::PrintTraitRefExt;

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
    pub fn fn_type_unsafety(self, ty: Ty<'tcx>) -> AnalysisResult<'tcx, Safety> {
        match ty.kind() {
            ty::FnDef(..) | ty::FnPtr(..) => {
                let sig = ty.fn_sig(self.tcx);
                Ok(sig.skip_binder().safety)
            }
            ty::Closure(_def_id, args) => {
                let sig = args.as_closure().sig();
                Ok(sig.skip_binder().safety)
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
    // The current version is based on nightly-2025-02-20
    pub fn get_def_path(&self, def_id: DefId) -> Vec<Symbol> {
        use rustc_hir::definitions::{DefPathData, DisambiguatedDefPathData};
        use rustc_middle::ty::print::{with_no_trimmed_paths, PrintError, Printer};

        pub struct AbsolutePathPrinter<'tcx> {
            pub tcx: TyCtxt<'tcx>,
            pub path: Vec<Symbol>,
        }

        impl<'tcx> Printer<'tcx> for AbsolutePathPrinter<'tcx> {
            fn tcx<'a>(&'a self) -> TyCtxt<'tcx> {
                self.tcx
            }

            fn print_region(&mut self, _region: ty::Region<'tcx>) -> Result<(), PrintError> {
                Ok(())
            }

            fn print_type(&mut self, _ty: Ty<'tcx>) -> Result<(), PrintError> {
                Ok(())
            }

            fn print_dyn_existential(
                &mut self,
                _predicates: &'tcx ty::List<ty::PolyExistentialPredicate<'tcx>>,
            ) -> Result<(), PrintError> {
                Ok(())
            }

            fn print_const(&mut self, _ct: ty::Const<'tcx>) -> Result<(), PrintError> {
                Ok(())
            }

            fn path_crate(&mut self, cnum: CrateNum) -> Result<(), PrintError> {
                self.path = vec![self.tcx.crate_name(cnum)];
                Ok(())
            }

            fn path_qualified(
                &mut self,
                self_ty: Ty<'tcx>,
                trait_ref: Option<ty::TraitRef<'tcx>>,
            ) -> Result<(), PrintError> {
                if trait_ref.is_none() {
                    if let ty::Adt(def, args) = self_ty.kind() {
                        return self.print_def_path(def.did(), args);
                    }
                }

                // This shouldn't ever be needed, but just in case:
                with_no_trimmed_paths!(self.path = vec![match trait_ref {
                    Some(trait_ref) => Symbol::intern(&format!("{:?}", trait_ref)),
                    None => Symbol::intern(&format!("<{}>", self_ty)),
                }]);
                Ok(())
            }

            fn path_append_impl(
                &mut self,
                print_prefix: impl FnOnce(&mut Self) -> Result<(), PrintError>,
                _disambiguated_data: &DisambiguatedDefPathData,
                self_ty: Ty<'tcx>,
                trait_ref: Option<ty::TraitRef<'tcx>>,
            ) -> Result<(), PrintError> {
                print_prefix(self)?;

                // This shouldn't ever be needed, but just in case:
                self.path.push(match trait_ref {
                    Some(trait_ref) => with_no_trimmed_paths!(Symbol::intern(&format!(
                        "<impl {} for {}>",
                        trait_ref.print_only_trait_path(),
                        self_ty
                    ))),
                    None => {
                        with_no_trimmed_paths!(Symbol::intern(&format!("<impl {}>", self_ty)))
                    }
                });

                Ok(())
            }

            fn path_append(
                &mut self,
                print_prefix: impl FnOnce(&mut Self) -> Result<(), PrintError>,
                disambiguated_data: &DisambiguatedDefPathData,
            ) -> Result<(), PrintError> {
                print_prefix(self)?;

                // Skip `::{{constructor}}` on tuple/unit structs.
                if let DefPathData::Ctor = disambiguated_data.data {
                    return Ok(());
                }

                self.path.push(Symbol::intern(&disambiguated_data.data.to_string()));
                Ok(())
            }

            fn path_generic_args(
                &mut self,
                print_prefix: impl FnOnce(&mut Self) -> Result<(), PrintError>,
                _args: &[GenericArg<'tcx>],
            ) -> Result<(), PrintError> {
                print_prefix(self)
            }
        }

        let mut printer = AbsolutePathPrinter { tcx: self.tcx, path: Vec::new() };
        printer.print_def_path(def_id, &[]).unwrap();
        printer.path
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
        if !tcx.has_typeck_results(self.expr.hir_id.owner.def_id) {
            log_err!(InvalidOwner);
            return None;
        }

        let typeck_tables = tcx.typeck(self.expr.hir_id.owner.def_id);
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
