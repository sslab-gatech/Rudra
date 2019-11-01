use std::collections::{HashMap, HashSet};

use rustc::hir::Unsafety;
use rustc::mir;
use rustc::mir::mono::MonoItem;
use rustc::ty::{Instance, ParamEnv, TyCtxt};
use rustc_mir::monomorphize::collector::{collect_crate_mono_items, MonoItemCollectionMode};

use crate::utils;
use crate::TyCtxtExt;

type Graph<'tcx> = HashMap<Instance<'tcx>, Vec<Instance<'tcx>>>;

// 'tcx: TyCtxt lifetime
pub struct CallGraph<'tcx> {
    tcx: TyCtxt<'tcx>,
    // this HashSet contains local mono items, which will be starting points of our analysis
    _entry: HashSet<Instance<'tcx>>,
    // this HashMap contains a call graph of all reachable instances
    graph: Graph<'tcx>,
}

impl<'tcx> CallGraph<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>) -> Self {
        let mut entry = HashSet::new();
        let mut graph = HashMap::new();

        // collect all mono items in the crate
        let inlining_map = collect_crate_mono_items(tcx, MonoItemCollectionMode::Lazy).1;
        inlining_map.iter_accesses(|entry_mono_item, _| {
            if entry_mono_item.is_instantiable(tcx) {
                if let MonoItem::Fn(entry_instance) = entry_mono_item {
                    entry.insert(entry_instance);
                    CallGraph::traverse(tcx, entry_instance, &mut graph);
                } else {
                    warn!("Unhandled mono item: {:?}", entry_mono_item);
                }
            }
        });

        CallGraph {
            tcx,
            _entry: entry,
            graph,
        }
    }

    fn traverse(tcx: TyCtxt<'tcx>, start: Instance<'tcx>, graph: &mut Graph<'tcx>) {
        use std::collections::hash_map::Entry;

        if let Entry::Vacant(entry) = graph.entry(start) {
            // early insert to prevent infinite recursion
            let vec = entry.insert(Vec::new());
            if let Some(mir_body) = tcx.find_fn(start) {
                utils::print_mir(tcx, start);
                // remove the duplication and copy the result into the hashmap
                // in most case, the number of callees are small enough that
                // the cost of the linear lookup is smaller than using a hashmap
                for instance in CallGraph::collect_all_calls(tcx, mir_body).into_iter() {
                    if !vec.contains(&instance) {
                        vec.push(instance);
                    }
                }

                // clone to drop the reference to graph
                for next_instance in vec.clone().into_iter() {
                    let msg = format!("Call into {} -> {}", start, next_instance);
                    dbg!(msg);
                    CallGraph::traverse(tcx, next_instance, graph);
                }
            } else {
                warn!("MIR for `{:?}` is not available!", start);
            }
        }
    }

    /// Collects all function calls inside MIR body.
    /// Note that the same function can appear multiple times in the result.
    fn collect_all_calls(tcx: TyCtxt<'tcx>, body: &'tcx mir::Body<'tcx>) -> Vec<Instance<'tcx>> {
        use mir::{Operand, TerminatorKind};
        use rustc::ty::TyKind;

        let mut result = Vec::new();
        for bb in body.basic_blocks().iter() {
            if let Some(terminator) = &bb.terminator {
                if let TerminatorKind::Call {
                    func: Operand::Constant(box func),
                    ..
                } = &terminator.kind
                {
                    let func_ty = func.literal.ty;
                    match func_ty.kind {
                        TyKind::FnDef(def_id, substs) => {
                            dbg!(def_id, substs);
                            // FIXME: this ParamEnv does not contain enough information
                            if let Some(instance) =
                                Instance::resolve(tcx, ParamEnv::reveal_all(), def_id, substs)
                            {
                                result.push(instance);
                            }
                        }
                        TyKind::FnPtr(_) => {
                            error!("Dynamic dispatch is not supported yet");
                        }
                        _ => panic!("invalid callee of type {:?}", func_ty),
                    }
                }
            }
        }
        result
    }

    pub fn print_mir_availability(&self) {
        for (&instance, _) in self.graph.iter() {
            if let None = self.tcx.find_fn(instance) {
                info!("MIR not available for {:?}", instance.def.def_id());
            }
        }
        info!("Found {} functions", self.graph.len());
    }

    pub fn local_safe_fn_iter(&self) -> impl Iterator<Item = Instance<'tcx>> + '_ {
        let tcx = self.tcx;
        self.graph.iter().filter_map(move |(&instance, _)| {
            let def_id = instance.def.def_id();
            // check if it is local and safe function
            if def_id.is_local() && instance.fn_sig(tcx).unsafety() == Unsafety::Normal {
                return Some(instance);
            }
            None
        })
    }

    /// A function that returns reachable instances starting from the provided instance.
    /// If the given instance is not found in the call graph,
    /// it will return a HashSet with a single element.
    pub fn reachable_set(&self, instance: Instance<'tcx>) -> HashSet<Instance<'tcx>> {
        let mut stack = vec![instance];
        let mut result = HashSet::new();
        result.insert(instance);

        while let Some(cur) = stack.pop() {
            if let Some(next_vec) = self.graph.get(&cur) {
                for &next in next_vec.iter() {
                    if result.insert(next) {
                        stack.push(next);
                    }
                }
            }
        }

        result
    }
}
