use std::fmt;

use rustc_hir::def_id::DefId;
use rustc_hir::Unsafety;
use rustc_middle::mir;
use rustc_middle::ty::{self, subst::SubstsRef, Instance, ParamEnv, Ty, TyCtxt};

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
    fn find_fn(self, instance: Instance<'tcx>) -> MirBody<'tcx>;
    fn monomorphic_resolve(
        self,
        callee_def_id: DefId,
        callee_substs: SubstsRef<'tcx>,
        caller_substs: SubstsRef<'tcx>,
    ) -> Option<Instance<'tcx>>;
}

impl<'tcx> TyCtxtExt<'tcx> for TyCtxt<'tcx> {
    /// Try to find MIR function body with given Instance
    /// this is a combined version of MIRI's find_fn + Rust InterpCx's load_mir
    fn find_fn(self, instance: Instance<'tcx>) -> MirBody<'tcx> {
        // TODO: apply hooks in rustc MIR evaluator based on this
        // https://github.com/rust-lang/miri/blob/1037f69bf6dcf73dfbe06453336eeae61ba7c51f/src/shims/mod.rs

        // currently we don't handle any foreign item
        if self.is_foreign_item(instance.def_id()) {
            return MirBody::Foreign(instance);
        }

        // based on rustc InterpCx's `load_mir()`
        // https://doc.rust-lang.org/nightly/nightly-rustc/src/rustc_mir/interpret/eval_context.rs.html
        let def_id = instance.def.def_id();
        if let Some(def_id) = def_id.as_local() {
            if self.has_typeck_tables(def_id)
                && self.typeck_tables_of(def_id).tainted_by_errors.is_some()
            {
                // type check failure; shouldn't happen since we already ran `cargo check`
                panic!("Type check failed for an item: {:?}", &instance);
            }
        }

        match instance.def {
            ty::InstanceDef::Item(_) => {
                if self.is_mir_available(def_id) {
                    MirBody::Static(self.optimized_mir(def_id))
                } else {
                    info!(
                        "Skipping an item {:?}, no MIR available for this item",
                        &instance
                    );
                    MirBody::NotAvailable(instance)
                }
            }
            ty::InstanceDef::DropGlue(_, _) => MirBody::Static(self.instance_mir(instance.def)),
            ty::InstanceDef::Virtual(_, _) => MirBody::Virtual(instance),
            _ => MirBody::Unknown(instance),
        }
    }

    fn monomorphic_resolve(
        self,
        callee_def_id: DefId,
        callee_substs: SubstsRef<'tcx>,
        caller_substs: SubstsRef<'tcx>,
    ) -> Option<Instance<'tcx>> {
        let replaced_substs = self.subst_and_normalize_erasing_regions(
            caller_substs,
            ParamEnv::reveal_all(),
            &callee_substs,
        );
        Instance::resolve(self, ParamEnv::reveal_all(), callee_def_id, replaced_substs).unwrap()
    }
}

pub trait TyExt<'tcx> {
    fn fn_unsafety(self, tcx: TyCtxt<'tcx>) -> Unsafety;
}

impl<'tcx> TyExt<'tcx> for Ty<'tcx> {
    // based on rustc Instance's `fn_sig_for_fn_abi()`
    fn fn_unsafety(self, tcx: TyCtxt<'tcx>) -> Unsafety {
        match self.kind {
            ty::FnDef(..) | ty::FnPtr(_) => {
                let sig = self.fn_sig(tcx);
                sig.unsafety()
            }
            ty::Closure(_def_id, substs) => {
                let sig = substs.as_closure().sig();
                sig.unsafety()
            }
            _ => panic!("unexpected type {:?} in TyExt::fn_unsafety", self),
        }
    }
}
