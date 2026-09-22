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
            name: id.split(':').next_back().unwrap_or(id).to_string(),
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

    /// Resolve `call:` and `symbol:` placeholder edges onto real symbols.
    ///
    /// A name that matches more than one equally good definition is left unresolved.
    /// Fan-out would make `impact` report every same-named method as a caller.
    pub fn link(&mut self) {
        let mut by_name: HashMap<String, Vec<ResolvedSym>> = HashMap::new();
        let mut by_id: HashMap<String, ResolvedSym> = HashMap::new();

        for (id, &idx) in &self.node_indices {
            if is_placeholder_id(id) {
                continue;
            }
            let node = &self.graph[idx];
            if node.kind == "unknown" || node.kind == "module" {
                continue;
            }
            let sym = ResolvedSym {
                id: id.clone(),
                name: node.name.clone(),
                qualname: qualname_of(id).to_string(),
                file_path: node.file_path.clone(),
                kind: node.kind.clone(),
            };
            by_name.entry(sym.name.clone()).or_default().push(sym.clone());
            by_id.insert(id.clone(), sym);
        }

        let placeholders: Vec<(NodeIndex, NodeIndex, EdgeType, String)> = self
            .graph
            .edge_indices()
            .filter_map(|edge| {
                let (src, tgt) = self.graph.edge_endpoints(edge)?;
                let tgt_id = &self.graph[tgt].id;
                if !(tgt_id.starts_with("call:") || tgt_id.starts_with("symbol:")) {
                    return None;
                }
                let raw = tgt_id
                    .strip_prefix("call:")
                    .or_else(|| tgt_id.strip_prefix("symbol:"))
                    .unwrap_or(tgt_id.as_str())
                    .to_string();
                Some((src, tgt, self.graph[edge], raw))
            })
            .collect();

        let mut remove_pairs = Vec::new();
        for (src_idx, tgt_idx, edge_type, raw) in placeholders {
            let src_id = self.graph[src_idx].id.clone();
            let Some(target_id) = resolve_callee(&raw, &src_id, &by_id, &by_name) else {
                continue;
            };
            let Some(&resolved_idx) = self.node_indices.get(&target_id) else {
                continue;
            };
            if resolved_idx == src_idx {
                continue;
            }
            if !self.graph.contains_edge(src_idx, resolved_idx) {
                self.graph.add_edge(src_idx, resolved_idx, edge_type);
            }
            remove_pairs.push((src_idx, tgt_idx));
        }

        for (src, tgt) in remove_pairs {
            if let Some(edge) = self.graph.find_edge(src, tgt) {
                self.graph.remove_edge(edge);
            }
        }

        self.drop_orphan_placeholders();
    }

    fn drop_orphan_placeholders(&mut self) {
        let has_orphan = self.graph.node_indices().any(|idx| {
            is_placeholder_id(&self.graph[idx].id)
                && self.graph.neighbors_undirected(idx).next().is_none()
        });
        if !has_orphan {
            return;
        }

        let old = std::mem::take(&mut self.graph);
        self.node_indices.clear();
        let mut index_map: HashMap<NodeIndex, NodeIndex> = HashMap::new();

        for idx in old.node_indices() {
            let id = &old[idx].id;
            if is_placeholder_id(id) && old.neighbors_undirected(idx).next().is_none() {
                continue;
            }
            let new_idx = self.graph.add_node(old[idx].clone());
            index_map.insert(idx, new_idx);
            self.node_indices.insert(id.clone(), new_idx);
        }

        for edge in old.edge_indices() {
            if let Some((src, tgt)) = old.edge_endpoints(edge) {
                if let (Some(&new_src), Some(&new_tgt)) = (index_map.get(&src), index_map.get(&tgt)) {
                    self.graph.add_edge(new_src, new_tgt, old[edge]);
                }
            }
        }
    }
}

#[derive(Clone)]
struct ResolvedSym {
    id: String,
    name: String,
    qualname: String,
    file_path: String,
    kind: String,
}

fn is_placeholder_id(id: &str) -> bool {
    id.starts_with("call:") || id.starts_with("symbol:") || id.starts_with("import:")
}

fn qualname_of(id: &str) -> &str {
    id.rsplit_once(':').map(|(_, qual)| qual).unwrap_or(id)
}

/// Pick one callee. Ties return None so impact analysis stays precise.
fn resolve_callee(
    raw: &str,
    src_id: &str,
    by_id: &HashMap<String, ResolvedSym>,
    by_name: &HashMap<String, Vec<ResolvedSym>>,
) -> Option<String> {
    let bare = raw.rsplit('.').next().unwrap_or(raw);
    let candidates = by_name.get(bare)?;
    let src = by_id.get(src_id);
    let src_file = src.map(|sym| sym.file_path.as_str()).unwrap_or("");
    let src_class = src.and_then(|sym| {
        if sym.kind == "method" {
            sym.qualname
                .rsplit_once('.')
                .map(|(class_name, _)| class_name.to_string())
        } else {
            None
        }
    });
    let receiver_self = raw.starts_with("self.") || raw.starts_with("cls.");

    let mut best_score = 0i32;
    let mut best: Vec<&str> = Vec::new();

    for cand in candidates {
        if cand.id == src_id {
            continue;
        }
        if receiver_self {
            let same_class = src_class
                .as_deref()
                .is_some_and(|class_name| cand.qualname == format!("{class_name}.{bare}"));
            if !same_class {
                continue;
            }
        }

        let mut score = 10;
        if cand.qualname == raw || cand.name == raw {
            score += 100;
        }
        if !src_file.is_empty() && cand.file_path == src_file {
            score += 50;
        }
        if let Some(class_name) = src_class.as_deref() {
            if cand.qualname == format!("{class_name}.{bare}") {
                score += 80;
            }
        }
        if !raw.contains('.') && (cand.kind == "function" || cand.kind == "class") {
            score += 20;
        }

        if score > best_score {
            best_score = score;
            best.clear();
            best.push(cand.id.as_str());
        } else if score == best_score {
            best.push(cand.id.as_str());
        }
    }

    if best.len() == 1 {
        Some(best[0].to_string())
    } else {
        None
    }
}
