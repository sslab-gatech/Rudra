use std::collections::{HashSet, VecDeque};

use super::{Constraint, ConstraintSet, NodeId};

type NodeSet = HashSet<NodeId>;

struct WorkList {
    /// node ID that should be handled
    work_list: VecDeque<NodeId>,
    in_list: Vec<bool>,
}

impl WorkList {
    fn new(num_node: usize) -> Self {
        WorkList {
            work_list: VecDeque::new(),
            in_list: vec![false; num_node],
        }
    }

    fn push(&mut self, target: NodeId) {
        if self.in_list[target] == false {
            self.in_list[target] = true;
            self.work_list.push_back(target);
        }
    }

    fn pop(&mut self) -> Option<NodeId> {
        let result = self.work_list.pop_front();
        if let Some(idx) = result {
            self.in_list[idx] = false;
        }
        result
    }
}

struct StateW1 {
    work_list: WorkList,
    /// idx -> to
    from: Vec<NodeSet>,
    /// idx <- from
    to: Vec<NodeSet>,
}

impl StateW1 {
    fn new(num_node: usize) -> Self {
        StateW1 {
            work_list: WorkList::new(num_node),
            from: vec![NodeSet::new(); num_node],
            to: vec![NodeSet::new(); num_node],
        }
    }

    fn add_edge(&mut self, src: NodeId, dst: NodeId) {
        self.from[src].insert(dst);
        self.to[dst].insert(src);
    }

    fn contains_edge(&mut self, src: NodeId, dst: NodeId) -> bool {
        self.from[src].contains(&dst)
    }
}

/// Algorithm W1 from "Field-sensitive pointer analysis for C"
pub struct SolverW1 {
    state: StateW1,
    /// direct address set
    sol: Vec<NodeSet>,
    /// target >= *idx
    load: Vec<NodeSet>,
    /// *idx >= target
    store: Vec<NodeSet>,
    /// *idx >= {target}
    store_addr: Vec<NodeSet>,
}

impl SolverW1 {
    fn add_sol(&mut self, ptr: NodeId, loc: NodeId) {
        self.sol[ptr].insert(loc);
    }

    fn add_load(&mut self, src: NodeId, dst: NodeId) {
        self.load[src].insert(dst);
    }

    fn add_store(&mut self, src: NodeId, dst: NodeId) {
        self.store[dst].insert(src);
    }

    fn add_store_addr(&mut self, src: NodeId, dst: NodeId) {
        self.store_addr[dst].insert(src);
    }

    pub fn solve<T: ConstraintSet>(set: &T) -> Self {
        let num_node = set.num_locations();
        let mut solver = SolverW1 {
            sol: vec![NodeSet::new(); num_node],
            state: StateW1::new(num_node),
            load: vec![NodeSet::new(); num_node],
            store: vec![NodeSet::new(); num_node],
            store_addr: vec![NodeSet::new(); num_node],
        };

        for (src, constraint) in set.constraints() {
            use Constraint::*;
            match constraint {
                AddrOf(dst) => solver.add_sol(src, dst),
                Copy(dst) => solver.state.add_edge(src, dst),
                Load(dst) => solver.add_load(src, dst),
                Store(dst) => solver.add_store(src, dst),
                StoreAddr(dst) => solver.add_store_addr(src, dst),
            }
        }

        // initialize work list
        for node in 0..num_node {
            solver.state.work_list.push(node);
        }

        // process work list
        while let Some(idx) = solver.state.work_list.pop() {
            for &target in solver.load[idx].iter() {
                for &k in solver.sol[idx].iter() {
                    if !solver.state.contains_edge(k, target) {
                        solver.state.add_edge(k, target);
                        solver.state.work_list.push(k);
                    }
                }
            }

            for &target in solver.store[idx].iter() {
                for &k in solver.sol[idx].iter() {
                    if !solver.state.contains_edge(target, k) {
                        solver.state.add_edge(target, k);
                        solver.state.work_list.push(target);
                    }
                }
            }

            for &target in solver.store_addr[idx].iter() {
                let mut delayed = Vec::new();
                for &k in solver.sol[idx].iter() {
                    if !solver.sol[k].contains(&target) {
                        delayed.push((k, target));
                    }
                }

                for &(k, target) in delayed.iter() {
                    solver.sol[k].insert(target);
                    solver.state.work_list.push(k);
                }
            }

            for &target in solver.state.from[idx].iter() {
                let union: HashSet<_> = solver.sol[target]
                    .union(&solver.sol[idx])
                    .map(|&id| id)
                    .collect();

                if union.len() > solver.sol[target].len() {
                    solver.sol[target] = union;
                    solver.state.work_list.push(target);
                }
            }
        }

        solver
    }
}
