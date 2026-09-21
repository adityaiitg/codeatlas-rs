use std::collections::{HashMap, HashSet, VecDeque};
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::Direction;
use serde::{Deserialize, Serialize};

use crate::models::{Edge, EdgeType, Symbol};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeData {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub file_path: String,
    pub start_line: usize,
}

#[derive(Default, Clone)]
pub struct CodeGraph {
    pub graph: DiGraph<NodeData, EdgeType>,
    pub node_indices: HashMap<String, NodeIndex>,
}

impl CodeGraph {
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            node_indices: HashMap::new(),
        }
    }

    pub fn add_symbol(&mut self, sym: &Symbol) -> NodeIndex {
        if let Some(&idx) = self.node_indices.get(&sym.node_id) {
            return idx;
        }

        let node_data = NodeData {
            id: sym.node_id.clone(),
            name: sym.name.clone(),
            kind: sym.kind.as_str().to_string(),
            file_path: sym.file_path.clone(),
            start_line: sym.start_line,
        };

        let idx = self.graph.add_node(node_data);
        self.node_indices.insert(sym.node_id.clone(), idx);
        idx
    }

    pub fn get_or_create_node(&mut self, id: &str) -> NodeIndex {
        if let Some(&idx) = self.node_indices.get(id) {
            return idx;
        }

        let node_data = NodeData {
            id: id.to_string(),
            name: id.split(':').last().unwrap_or(id).to_string(),
            kind: "unknown".to_string(),
            file_path: String::new(),
            start_line: 0,
        };

        let idx = self.graph.add_node(node_data);
        self.node_indices.insert(id.to_string(), idx);
        idx
    }

    pub fn add_edge(&mut self, edge: &Edge) {
        let src_idx = self.get_or_create_node(&edge.source_id);
        let tgt_idx = self.get_or_create_node(&edge.target_id);
        self.graph.add_edge(src_idx, tgt_idx, edge.edge_type);
    }

    pub fn expand_neighborhood(&self, roots: &[String], depth: usize) -> Vec<String> {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        for root in roots {
            if let Some(&idx) = self.node_indices.get(root) {
                visited.insert(idx);
                queue.push_back((idx, 0));
            }
        }

        while let Some((current, d)) = queue.pop_front() {
            if d >= depth {
                continue;
            }

            // Both incoming and outgoing neighbors
            for neighbor in self.graph.neighbors_undirected(current) {
                if visited.insert(neighbor) {
                    queue.push_back((neighbor, d + 1));
                }
            }
        }

        visited
            .into_iter()
            .map(|idx| self.graph[idx].id.clone())
            .collect()
    }

    pub fn impact_analysis(&self, target_id: &str) -> Vec<String> {
        // Reverse BFS: finding everything that depends on / calls target_id
        let target_idx = match self.node_indices.get(target_id) {
            Some(&idx) => idx,
            None => {
                // Try matching by suffix or name
                let found = self.node_indices.iter().find(|(k, _)| k.ends_with(target_id));
                match found {
                    Some((_, &idx)) => idx,
                    None => return Vec::new(),
                }
            }
        };

        let mut affected = Vec::new();
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        visited.insert(target_idx);
        queue.push_back(target_idx);

        while let Some(current) = queue.pop_front() {
            // Incoming edges: nodes that call, import, or inherit current
            for caller in self.graph.neighbors_directed(current, Direction::Incoming) {
                if visited.insert(caller) {
                    affected.push(self.graph[caller].id.clone());
                    queue.push_back(caller);
                }
            }
        }

        affected
    }

    pub fn export_json(&self) -> serde_json::Value {
        let nodes: Vec<_> = self.graph.node_weights().cloned().collect();
        let edges: Vec<_> = self
            .graph
            .edge_indices()
            .filter_map(|e| {
                let (src, tgt) = self.graph.edge_endpoints(e)?;
                let weight = self.graph[e];
                Some(serde_json::json!({
                    "source": self.graph[src].id,
                    "target": self.graph[tgt].id,
                    "type": weight.as_str(),
                }))
            })
            .collect();

        serde_json::json!({
            "nodes": nodes,
            "edges": edges,
        })
    }

    pub fn export_dot(&self) -> String {
        let mut dot = String::from("digraph CodeAtlas {\n  rankdir=LR;\n  node [shape=box, fontname=\"Helvetica\"];\n");
        for idx in self.graph.node_indices() {
            let n = &self.graph[idx];
            dot.push_str(&format!("  \"{}\" [label=\"{}\\n({})\"];\n", n.id, n.name, n.kind));
        }
        for e in self.graph.edge_indices() {
            if let Some((src, tgt)) = self.graph.edge_endpoints(e) {
                let weight = self.graph[e];
                dot.push_str(&format!(
                    "  \"{}\" -> \"{}\" [label=\"{}\"];\n",
                    self.graph[src].id,
                    self.graph[tgt].id,
                    weight.as_str()
                ));
            }
        }
        dot.push_str("}\n");
        dot
    }
}
