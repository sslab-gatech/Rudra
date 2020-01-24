use std::cmp::min;

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
        use ir::TerminatorKind::*;
        match self.basic_blocks[id].terminator.kind {
            Goto(n) => vec![n],
            StaticCall {
                cleanup,
                destination: (_, destination_block),
                ..
            } => match cleanup {
                Some(cleanup_block) => vec![destination_block, cleanup_block],
                None => vec![destination_block],
            },
            Dummy(_) => vec![],
        }
    }
}

/// Strongly Connected Component (SCC) using Tarjan's algorithm
pub struct Scc<'a, G: Graph> {
    graph: &'a G,
    /// group number of each item (indexed by node)
    group_of_node: Vec<usize>,
    /// adjacency list of groups (indexed by group)
    group_graph: Vec<Vec<usize>>,
}

/// Temporary state variable used in SCC construction
struct SccConstructionState {
    // intermediate state
    current_index: usize,
    stack: Vec<usize>,
    index: Vec<usize>,
    low_link: Vec<usize>,
    // result
    group_number: Vec<usize>,
    nodes_in_group: Vec<Vec<usize>>,
}

impl SccConstructionState {
    fn new(size: usize) -> Self {
        SccConstructionState {
            current_index: 0,
            stack: Vec::new(),
            index: vec![0; size],
            low_link: vec![0; size],
            group_number: vec![0; size],
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
                let from_group = state.group_number[from];
                let to_group = state.group_number[to];
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

        Scc {
            graph,
            group_of_node: state.group_number,
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
            } else if state.group_number[next] == 0 {
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
                state.group_number[now] = group_num;
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

    pub fn group_of_node(&self, idx: usize) -> usize {
        self.group_of_node[idx]
    }

    pub fn next_groups(&self, group_idx: usize) -> &[usize] {
        &self.group_graph[group_idx]
    }
}
