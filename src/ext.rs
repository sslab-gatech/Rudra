use rustc::hir::def_id::DefId;
use rustc::mir;
use rustc::ty::{self, subst::SubstsRef, Instance, ParamEnv, TyCtxt};

pub trait TyCtxtExt<'tcx> {
    fn find_fn(self, instance: Instance<'tcx>) -> Option<&'tcx mir::Body<'tcx>>;
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
    fn find_fn(self, instance: Instance<'tcx>) -> Option<&'tcx mir::Body<'tcx>> {
        // TODO: apply hooks in rustc MIR evaluator based on this
        // https://github.com/rust-lang/miri/blob/1037f69bf6dcf73dfbe06453336eeae61ba7c51f/src/shims/mod.rs

        // currently we don't handle any foreign item
        if self.is_foreign_item(instance.def_id()) {
            info!("Unsupported foreign item: {:?}", &instance);
            return None;
        }

        // based on rustc InterpCx's load_mir
        // https://doc.rust-lang.org/nightly/nightly-rustc/src/rustc_mir/interpret/eval_context.rs.html
        let def_id = instance.def.def_id();
        if def_id.is_local()
            && self.has_typeck_tables(def_id)
            && self.typeck_tables_of(def_id).tainted_by_errors
        {
            // type check failure
            info!("Type check failed for an item: {:?}", &instance);
            return None;
        }

        match instance.def {
            ty::InstanceDef::Item(_) => {
                if self.is_mir_available(def_id) {
                    Some(self.optimized_mir(def_id))
                } else {
                    info!(
                        "Skipping an item {:?}, no MIR available for this item",
                        &instance
                    );
                    None
                }
            }
            _ => {
                // usually `real_drop_in_place`
                Some(self.instance_mir(instance.def))
            }
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
