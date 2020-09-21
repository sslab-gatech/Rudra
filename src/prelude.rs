use rustc_hir::def_id::DefId;
use rustc_hir::Unsafety;
use rustc_middle::ty::{self, subst::SubstsRef, Instance, ParamEnv, TyCtxt};

pub use snafu::{Error, ErrorCompat, IntoError, ResultExt};

pub use crate::analysis::{AnalysisError, AnalysisErrorKind, AnalysisOutputVec, AnalysisResult};
pub use crate::context::CruxCtxt;

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

    pub fn fn_type_unsafety(self, instance: Instance<'tcx>) -> Unsafety {
        let ty = instance.ty(self.tcx, ParamEnv::reveal_all());
        match ty.kind {
            ty::FnDef(..) | ty::FnPtr(_) => {
                let sig = ty.fn_sig(self.tcx);
                sig.unsafety()
            }
            ty::Closure(_def_id, substs) => {
                let sig = substs.as_closure().sig();
                sig.unsafety()
            }
            _ => panic!("Non-function type {:?} in `TyExt::ty_fn_unsafety`", ty),
        }
    }
}
