use rustc::ty::{self, subst::SubstsRef, Instance, ParamEnv, Ty, TyCtxt, TyKind};
use rustc_hir::def_id::DefId;
use rustc_hir::Unsafety;

use crate::MirBody;

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
    // TODO: use more fine-grained error handling than returning None
    fn find_fn(self, instance: Instance<'tcx>) -> MirBody<'tcx> {
        // TODO: apply hooks in rustc MIR evaluator based on this
        // https://github.com/rust-lang/miri/blob/1037f69bf6dcf73dfbe06453336eeae61ba7c51f/src/shims/mod.rs

        // currently we don't handle any foreign item
        if self.is_foreign_item(instance.def_id()) {
            return MirBody::Foreign(instance);
        }

        // based on rustc InterpCx's load_mir
        // https://doc.rust-lang.org/nightly/nightly-rustc/src/rustc_mir/interpret/eval_context.rs.html
        let def_id = instance.def.def_id();
        if def_id.is_local()
            && self.has_typeck_tables(def_id)
            && self.typeck_tables_of(def_id).tainted_by_errors
        {
            // type check failure
            panic!("Type check failed for an item: {:?}", &instance);
        }

        match instance.def {
            ty::InstanceDef::Item(_) => {
                if self.is_mir_available(def_id) {
                    MirBody::Static(self.optimized_mir(def_id).unwrap_read_only())
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
        Instance::resolve(self, ParamEnv::reveal_all(), callee_def_id, replaced_substs)
    }
}

pub trait TyExt<'tcx> {
    fn fn_unsafety(self, tcx: TyCtxt<'tcx>) -> Unsafety;
}

impl<'tcx> TyExt<'tcx> for Ty<'tcx> {
    // based on rustc Instance's fn_sig_for_fn_abi
    fn fn_unsafety(self, tcx: TyCtxt<'tcx>) -> Unsafety {
        match self.kind {
            TyKind::FnDef(..) | TyKind::FnPtr(_) => {
                let sig = self.fn_sig(tcx);
                sig.unsafety()
            }
            ty::Closure(def_id, substs) => {
                let sig = substs.as_closure().sig(def_id, tcx);
                sig.unsafety()
            }
            _ => panic!("unexpected type {:?} in TyExt::fn_unsafety", self),
        }
    }
}
