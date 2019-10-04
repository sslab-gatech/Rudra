use std::collections::HashMap;

use rustc::hir::def_id::DefId;
use rustc::mir;
use rustc::ty::TyCtxt;

// 'a: analyze function lifetime
// 'tcx: TyCtxt lifetime
pub struct CallGraph<'a, 'tcx> {
    tcx: &'a TyCtxt<'tcx>,
    graph: HashMap<DefId, Vec<DefId>>,
}

impl<'a, 'tcx> CallGraph<'a, 'tcx> {
    pub fn new(tcx: &'a TyCtxt<'tcx>) -> Self {
        CallGraph {
            tcx,
            graph: HashMap::new(),
        }
    }

    pub fn traverse(&mut self, node: DefId) {
        use std::collections::hash_map::Entry;

        // do not traverse the same node twice
        if let Entry::Vacant(entry) = self.graph.entry(node) {
            // early insert to prevent infinite recursion
            let vec = entry.insert(Vec::new());
            if !self.tcx.is_mir_available(node) {
                println!("Missing MIR for `{:?}`", node);
                return;
            }
            let body = self.tcx.optimized_mir(node);

            // remove the duplication and copy the result into the hashmap
            // in most case, the number of callees are small enough that
            // the cost of the linear lookup is smaller than using a hashmap
            for callee_def_id in CallGraph::collect_all_calls(body).into_iter() {
                if !vec.contains(&callee_def_id) {
                    vec.push(callee_def_id);
                }
            }

            // mutable borrow of self ends here (thanks to NLL)
            let clone = vec.clone();
            for calee_def_id in clone.iter() {
                self.traverse(*calee_def_id);
            }
        }
    }

    // collects all function calls inside MIR body
    // same function can appear multiple times in the result
    fn collect_all_calls(body: &'tcx mir::Body<'tcx>) -> Vec<DefId> {
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
                    match &func_ty.kind {
                        TyKind::FnDef(callee_def_id, _) => {
                            result.push(*callee_def_id);
                        }
                        TyKind::FnPtr(_) => {
                            unimplemented!("Dynamic dispatch is not supported yet");
                        }
                        _ => panic!("invalid callee of type {:?}", func_ty),
                    }
                }
            }
        }
        result
    }
}
