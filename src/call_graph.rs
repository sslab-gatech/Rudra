use std::collections::HashMap;

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
                    _ => (),
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
            println!("Function: {:?}", instance.def.def_id());
            self.tcx.find_fn(instance);
        }
    }
}
