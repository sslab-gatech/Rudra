use std::fmt;

use rustc_hir::def_id::DefId;
use rustc_hir::Unsafety;
use rustc_middle::mir;
use rustc_middle::ty::{self, subst::SubstsRef, Instance, ParamEnv, TyCtxt};

pub enum MirBody<'tcx> {
    Static(&'tcx mir::Body<'tcx>),
    Foreign(Instance<'tcx>),
    Virtual(Instance<'tcx>),
    Unknown(Instance<'tcx>),
    NotAvailable(Instance<'tcx>),
}

impl<'tcx> MirBody<'tcx> {
    pub fn body(&self) -> Option<&'tcx mir::Body<'tcx>> {
        if let MirBody::Static(body) = self {
            Some(&body)
        } else {
            None
        }
    }
}

impl<'tcx> fmt::Debug for MirBody<'tcx> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            MirBody::Static(_) => write!(f, "static body"),
            MirBody::Foreign(instance) => write!(f, "Foreign instance {:?}", instance),
            MirBody::Virtual(instance) => write!(f, "Virtual instance {:?}", instance),
            MirBody::Unknown(instance) => write!(f, "Unknown instance {:?}", instance),
            MirBody::NotAvailable(instance) => {
                write!(f, "MIR not avaiable for instance {:?}", instance)
            }
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
    /// Try to find MIR function body with given Instance
    /// this is a combined version of MIRI's find_fn + Rust InterpCx's load_mir
    pub fn find_fn(self, instance: Instance<'tcx>) -> MirBody<'tcx> {
        // TODO: apply hooks in rustc MIR evaluator based on this
        // https://github.com/rust-lang/miri/blob/1037f69bf6dcf73dfbe06453336eeae61ba7c51f/src/shims/mod.rs

        // currently we don't handle any foreign item
        if self.tcx.is_foreign_item(instance.def_id()) {
            return MirBody::Foreign(instance);
        }

        // based on rustc InterpCx's `load_mir()`
        // https://doc.rust-lang.org/nightly/nightly-rustc/src/rustc_mir/interpret/eval_context.rs.html
        let def_id = instance.def.with_opt_param();
        if let Some(def) = def_id.as_local() {
            if self.tcx.has_typeck_results(def.did) {
                if let Some(_) = self.tcx.typeck_opt_const_arg(def).tainted_by_errors {
                    // type check failure; shouldn't happen since we already ran `cargo check`
                    panic!("Type check failed for an item: {:?}", &instance);
                }
            }
        }

        match instance.def {
            ty::InstanceDef::Item(def) => {
                if self.tcx.is_mir_available(def.did) {
                    if let Some((did, param_did)) = def.as_const_arg() {
                        MirBody::Static(self.tcx.optimized_mir_of_const_arg((did, param_did)))
                    } else {
                        MirBody::Static(self.tcx.optimized_mir(def.did))
                    }
                } else {
                    debug!(
                        "Skipping an item {:?}, no MIR available for this item",
                        &instance
                    );
                    MirBody::NotAvailable(instance)
                }
            }
            ty::InstanceDef::DropGlue(_, _) => MirBody::Static(self.tcx.instance_mir(instance.def)),
            ty::InstanceDef::Virtual(_, _) => MirBody::Virtual(instance),
            _ => MirBody::Unknown(instance),
        }
    }

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
