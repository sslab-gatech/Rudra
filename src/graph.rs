use std::{cmp::min, collections::VecDeque};

use crate::analysis::UDBypassKind;
use crate::ir;

pub trait Graph {
    fn len(&self) -> usize;
    fn next(&self, id: usize) -> Vec<usize>;
}

impl<'tcx> Graph for ir::Body<'tcx> {
    fn len(&self) -> usize {
        self.basic_blocks.len()
    }

    fn next(&self, id: usize) -> Vec<usize> {
        self.basic_blocks[id]
            .terminator
            .original
            .successors()
            .map(|block| block.index())
            .collect()
    }
}

pub struct Reachability<'a, G: Graph> {
    graph: &'a G,
    len: usize,
    sources: Vec<Option<UDBypassKind>>,
    sinks: Vec<bool>,
}

impl<'a, G: Graph> Reachability<'a, G> {
    pub fn new(graph: &'a G) -> Self {
        let graph_len = graph.len();
        Reachability {
            graph,
            len: graph_len,
            sources: vec![None; graph_len],
            sinks: vec![false; graph_len],
        }
    }

    pub fn graph(&self) -> &G {
        &self.graph
    }

    pub fn mark_source(&mut self, id: usize, udkind: UDBypassKind) {
        self.sources[id] = Some(udkind);
    }

    pub fn unmark_source(&mut self, id: usize) {
        self.sources[id] = None;
    }

    pub fn mark_sink(&mut self, id: usize) {
        self.sinks[id] = true;
    }

    pub fn unmark_sink(&mut self, id: usize) {
        self.sinks[id] = false;
    }

    // Unmark all sources and sinks
    pub fn clear(&mut self) {
        self.sources = vec![None; self.len];
        self.sinks = vec![false; self.len];
    }

    // Checks reachability between `self.sources` & `self.sinks`.
    // Returns a Vec of `UDBypassKind`s (of `self.sources`) that can reach `self.sinks`.
    // Returns an empty Vec if there are no reachable path.
    pub fn is_reachable(&self) -> Vec<UDBypassKind> {
        let mut visited = vec![Vec::new(); self.len];
        let mut work_list = VecDeque::new();

        // Initialize work list
        for id in 0..self.len {
            if let Some(udkind) = self.sources[id] {
                visited[id].push(udkind);
                work_list.push_back((id, udkind));
            }
        }

        // Breadth-first propagation
        while let Some((current, udkind)) = work_list.pop_front() {
            for next in self.graph.next(current) {
                visited[next].push(udkind);
                work_list.push_back((next, udkind));
            }
        }

        // Check the result
        for id in 0..self.len {
            if self.sinks[id] && !visited[id].is_empty() {
                return visited.swap_remove(id);
            }
        }

        return Vec::new();
    }
}

/// Strongly Connected Component (SCC) using Tarjan's algorithm
pub struct Scc<'a, G: Graph> {
    graph: &'a G,
    /// group number of each item (indexed by node)
    group_of_node: Vec<usize>,
    /// nodes in each SCC group (indexed by group)
    nodes_in_group: Vec<Vec<usize>>,
    /// adjacency list of groups (indexed by group)
    group_graph: Vec<Vec<usize>>,
}

/// Temporary state variable used in SCC construction
struct SccConstructionState {
    // intermediate state
    current_index: usize,
    stack: Vec<usize>,
    index: Vec<usize>,
    // result
    group_of_node: Vec<usize>,
    nodes_in_group: Vec<Vec<usize>>,
}

impl SccConstructionState {
    fn new(size: usize) -> Self {
        SccConstructionState {
            current_index: 0,
            stack: Vec::new(),
            index: vec![0; size],
            group_of_node: vec![0; size],
            nodes_in_group: Vec::new(),
        }
    }

    fn assign_id(&mut self, node: usize) {
        self.current_index += 1;
        self.index[node] = self.current_index;
    }
}

struct SccTopologicalOrderState {
    visited: Vec<bool>,
    order: Vec<usize>,
}

impl SccTopologicalOrderState {
    fn new(size: usize) -> Self {
        SccTopologicalOrderState {
            visited: vec![false; size],
            order: Vec::new(),
        }
    }
}

impl<'a, G: Graph> Scc<'a, G> {
    pub fn construct(graph: &'a G) -> Self {
        let num_node = graph.len();
        let mut state = SccConstructionState::new(num_node);

        // construct SCC
        for node in 0..num_node {
            if state.index[node] == 0 {
                Scc::traverse(graph, &mut state, node);
            }
        }

        // collect all inter-group edges
        let num_group = state.nodes_in_group.len();
        let mut group_graph = vec![Vec::new(); num_group];
        for from in 0..num_node {
            for to in graph.next(from).into_iter() {
                let from_group = state.group_of_node[from];
                let to_group = state.group_of_node[to];
                if from_group != to_group {
                    group_graph[from_group].push(to_group);
                }
            }
        }

        // remove duplicated edges
        for group in 0..num_group {
            let edges = &mut group_graph[group];
            edges.sort();
            edges.dedup();
        }

        let SccConstructionState {
            group_of_node,
            nodes_in_group,
            ..
        } = state;

        Scc {
            graph,
            group_of_node,
            nodes_in_group,
            group_graph,
        }
    }

    // returns the lowest reachable node
    fn traverse(graph: &'a G, state: &mut SccConstructionState, node: usize) -> usize {
        state.assign_id(node);
        state.stack.push(node);

        let mut low_link = state.index[node];
        for next in graph.next(node).into_iter() {
            if state.index[next] == 0 {
                // not visited yet
                low_link = min(low_link, Scc::traverse(graph, state, next));
            } else if state.group_of_node[next] == 0 {
                // already in stack
                low_link = min(low_link, state.index[next]);
            }
        }

        // SCC boundary found
        if low_link == state.index[node] {
            // all nodes in the stack after this node belongs to the same group
            let mut new_group = Vec::new();
            let group_num = state.nodes_in_group.len() + 1;
            loop {
                let now = state.stack.pop().unwrap();
                state.group_of_node[now] = group_num;
                new_group.push(now);

                if now == node {
                    break;
                }
            }
            state.nodes_in_group.push(new_group);
        }

        low_link
    }

    fn topological_dfs(&self, state: &mut SccTopologicalOrderState, group: usize) {
        state.visited[group] = true;
        state.order.push(group);
        for &next_group in self.next_groups(group).iter() {
            if !state.visited[next_group] {
                self.topological_dfs(state, next_group)
            }
        }
    }

    pub fn topological_order(&self) -> Vec<usize> {
        let num_group = self.group_graph.len();
        let mut state = SccTopologicalOrderState::new(num_group);

        for group in 0..num_group {
            if !state.visited[group] {
                self.topological_dfs(&mut state, group);
            }
        }

        let mut result = state.order;
        result.reverse();
        result
    }

    pub fn graph(&self) -> &G {
        &self.graph
    }

    pub fn group_of_node(&self, idx: usize) -> usize {
        self.group_of_node[idx]
    }

    pub fn nodes_in_group(&self, idx: usize) -> &[usize] {
        &self.nodes_in_group[idx]
    }

    pub fn next_groups(&self, group_idx: usize) -> &[usize] {
        &self.group_graph[group_idx]
    }
}
