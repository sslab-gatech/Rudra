//! Call graph and reachability calculation
use std::collections::{HashMap, HashSet};

use rustc_hir::Unsafety;
use rustc_middle::mir::mono::MonoItem;
use rustc_middle::ty::Instance;
use rustc_mir::monomorphize::collector::{collect_crate_mono_items, MonoItemCollectionMode};

use crate::ir;
use crate::prelude::*;

type Graph<'tcx> = HashMap<Instance<'tcx>, Vec<Instance<'tcx>>>;

// 'tcx: TyCtxt lifetime
pub struct CallGraph<'tcx> {
    ccx: CruxCtxt<'tcx>,
    // this HashSet contains local mono items, which will be starting points of our analysis
    _entry: HashSet<Instance<'tcx>>,
    // this HashMap contains a call graph of all reachable instances
    graph: Graph<'tcx>,
}

impl<'tcx> CallGraph<'tcx> {
    pub fn new(ccx: CruxCtxt<'tcx>) -> Self {
        let mut entry = HashSet::new();
        let mut graph = HashMap::new();

        // collect all mono items in the crate
        let inlining_map = collect_crate_mono_items(ccx.tcx(), MonoItemCollectionMode::Lazy).1;
        inlining_map.iter_accesses(|entry_mono_item, _| {
            if let MonoItem::Fn(entry_instance) = entry_mono_item {
                entry.insert(entry_instance);
                CallGraph::traverse(ccx, entry_instance, &mut graph);
            } else {
                trace!("Unhandled mono item: {:?}", entry_mono_item);
            }
        });

        CallGraph {
            ccx,
            _entry: entry,
            graph,
        }
    }

    fn traverse(ccx: CruxCtxt<'tcx>, caller: Instance<'tcx>, graph: &mut Graph<'tcx>) {
        use std::collections::hash_map::Entry;

        if let Entry::Vacant(entry) = graph.entry(caller) {
            // early insert to prevent infinite recursion
            let vec = entry.insert(Vec::new());
            match ccx.instance_body(caller).as_ref() {
                Ok(ir_body) => {
                    trace!("Instance: {:?}", caller);

                    for callee in CallGraph::collect_all_callees(ir_body).into_iter() {
                        // in most case, the number of callees are small enough that
                        // the cost of the linear lookup is smaller than using a hashmap
                        if !vec.contains(&callee) {
                            vec.push(callee);
                        }
                    }

                    // clone here to make the borrow checker happy with the recursive call
                    for next_instance in vec.clone().into_iter() {
                        trace!("Call into {} -> {}", caller, next_instance);
                        CallGraph::traverse(ccx, next_instance, graph);
                    }
                }
                Err(e) => debug!("Cannot instantiate MIR body: {:?}", e),
            }
        }
    }

    /// Collects all function calls inside MIR body.
    /// Note that the same function can appear multiple times in the result.
    fn collect_all_callees(body: &ir::Body<'tcx>) -> Vec<Instance<'tcx>> {
        let mut result = Vec::new();
        for bb in body.basic_blocks.iter() {
            let terminator = &bb.terminator;
            match terminator.kind {
                ir::TerminatorKind::StaticCall { target, .. } => {
                    result.push(target);
                }
                _ => (),
            }
        }
        result
    }

    pub fn num_functions(&self) -> usize {
        self.graph.len()
    }

    /// Local safe functions are potential entry points to our analysis
    pub fn local_safe_fn_iter(&self) -> impl Iterator<Item = Instance<'tcx>> + '_ {
        let tcx = self.ccx.tcx();
        self.graph.iter().filter_map(move |(&instance, _)| {
            let def_id = instance.def.def_id();
            // check if it is local and safe function
            if def_id.is_local() {
                if let Ok(Unsafety::Normal) = tcx.ext().fn_type_unsafety_instance(instance) {
                    return Some(instance);
                }
            }
            None
        })
    }

    /// A function that returns reachable instances starting from the provided instance.
    /// If the given instance is not found in the call graph,
    /// it will return an empty set.
    pub fn reachable_set(&self, instance: Instance<'tcx>) -> HashSet<Instance<'tcx>> {
        let mut stack = vec![instance];
        let mut result = HashSet::new();

        while let Some(cur) = stack.pop() {
            if let Some(next_vec) = self.graph.get(&cur) {
                result.insert(cur);
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
