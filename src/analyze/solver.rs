use std::collections::{HashSet, VecDeque};

use super::{Constraint, ConstraintSet};

type NodeSet = HashSet<usize>;

/// Algorithm W1 from "Field-sensitive pointer analysis for C"
pub struct SolverW1 {
    /// node ID that should be handled
    work_list: VecDeque<usize>,
    num_node: usize,
    /// direct address set
    sol: Vec<NodeSet>,
    /// idx -> to
    from: Vec<NodeSet>,
    /// idx <- from
    to: Vec<NodeSet>,
    /// target >= *idx
    load: Vec<NodeSet>,
    /// *idx >= target
    store: Vec<NodeSet>,
    /// *idx >= {target}
    store_addr: Vec<NodeSet>,
}

impl SolverW1 {
    fn add_sol(&mut self, ptr: usize, loc: usize) {
        self.sol[ptr].insert(loc);
    }

    fn add_edge(&mut self, src: usize, dst: usize) {
        self.from[src].insert(dst);
        self.to[dst].insert(src);
    }

    fn add_load(&mut self, src: usize, dst: usize) {
        self.load[src].insert(dst);
    }

    fn add_store(&mut self, src: usize, dst: usize) {
        self.store[dst].insert(src);
    }

    fn add_store_addr(&mut self, src: usize, dst: usize) {
        self.store_addr[dst].insert(src);
    }

    fn contains_edge(&mut self, src: usize, dst: usize) -> bool {
        self.from[src].contains(&dst)
    }

    pub fn solve<T: ConstraintSet>(set: &T) -> Self {
        let num_node = set.num_locations();
        let mut solver = SolverW1 {
            work_list: VecDeque::new(),
            num_node,
            sol: vec![NodeSet::new(); num_node],
            from: vec![NodeSet::new(); num_node],
            to: vec![NodeSet::new(); num_node],
            load: vec![NodeSet::new(); num_node],
            store: vec![NodeSet::new(); num_node],
            store_addr: vec![NodeSet::new(); num_node],
        };

        for (src, constraint) in set.constraints() {
            use Constraint::*;
            match constraint {
                AddrOf(dst) => solver.add_sol(src, dst),
                Copy(dst) => solver.add_edge(src, dst),
                Load(dst) => solver.add_load(src, dst),
                Store(dst) => solver.add_store(src, dst),
                StoreAddr(dst) => solver.add_store_addr(src, dst),
            }
        }

        todo!();

        solver
    }
}
