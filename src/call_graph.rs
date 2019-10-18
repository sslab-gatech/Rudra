use std::collections::{HashMap, HashSet};

use rustc::hir::Unsafety;
use rustc::mir::mono::MonoItem;
use rustc::ty::{Instance, TyCtxt};
use rustc_mir::monomorphize::collector::{collect_crate_mono_items, MonoItemCollectionMode};

use crate::TyCtxtExt;

// 'tcx: TyCtxt lifetime
pub struct CallGraph<'tcx> {
    tcx: TyCtxt<'tcx>,
    graph: HashMap<Instance<'tcx>, Vec<Instance<'tcx>>>,
}

impl<'tcx> CallGraph<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>) -> Self {
        // this graph is incomplete, and all edges should be added before returning it
        let mut result = CallGraph {
            tcx: tcx,
            graph: HashMap::new(),
        };

        // collect all mono items in crate
        // we will regard uninstantiated bug as out-of-scope
        let inlining_map = collect_crate_mono_items(tcx, MonoItemCollectionMode::Lazy).1;
        inlining_map.iter_accesses(|current_item, next_items| {
            // MonoItem implements Copy
            for next_item in next_items {
                match (current_item, next_item) {
                    // We only consider edges from a function to a function
                    (MonoItem::Fn(current_instance), &MonoItem::Fn(next_instance)) => {
                        result.add_edge(current_instance, next_instance);
                    }
                    _ => warn!("Unknown MonoItem {:?}", next_item),
                }
            }
        });

        result
    }

    fn add_edge(&mut self, from: Instance<'tcx>, to: Instance<'tcx>) {
        // in most case, the number of callees are small enough that
        // the cost of the linear lookup is smaller than using a hashmap
        let vec = self.graph.entry(from).or_insert(Vec::new());
        if !vec.contains(&to) {
            vec.push(to);
        }
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
