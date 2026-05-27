use std::collections::{HashMap, HashSet, VecDeque};

/// Merge outgoing + incoming into an undirected adjacency list.
pub fn to_undirected(
    outgoing: &HashMap<String, Vec<String>>,
    incoming: &HashMap<String, Vec<String>>,
) -> HashMap<String, HashSet<String>> {
    let mut adj: HashMap<String, HashSet<String>> = HashMap::new();
    for (node, targets) in outgoing {
        for t in targets {
            adj.entry(node.clone()).or_default().insert(t.clone());
            adj.entry(t.clone()).or_default().insert(node.clone());
        }
    }
    for (node, sources) in incoming {
        for s in sources {
            adj.entry(node.clone()).or_default().insert(s.clone());
            adj.entry(s.clone()).or_default().insert(node.clone());
        }
    }
    adj
}

/// Collect all unique edges as (a, b) pairs where a < b (undirected).
fn edge_list(adj: &HashMap<String, HashSet<String>>) -> Vec<(String, String)> {
    let mut edges = Vec::new();
    for (a, neighbors) in adj {
        for b in neighbors {
            if a < b {
                edges.push((a.clone(), b.clone()));
            }
        }
    }
    edges
}

// ── Community Detection (Louvain-style modularity optimization) ──────

/// Louvain-style community detection. Returns (node → community_id) map
/// and list of communities (each a vec of member node names).
pub fn detect_communities(
    outgoing: &HashMap<String, Vec<String>>,
    incoming: &HashMap<String, Vec<String>>,
) -> (HashMap<String, usize>, Vec<Vec<String>>) {
    let adj = to_undirected(outgoing, incoming);
    let edges = edge_list(&adj);
    let m = edges.len() as f64; // total edges

    if m == 0.0 {
        // No edges: every node is its own community
        let mut community_of = HashMap::new();
        let mut communities = Vec::new();
        for (i, node) in adj.keys().enumerate() {
            community_of.insert(node.clone(), i);
            communities.push(vec![node.clone()]);
        }
        return (community_of, communities);
    }

    // Degree of each node
    let degree: HashMap<&String, f64> = adj.iter()
        .map(|(k, v)| (k, v.len() as f64))
        .collect();

    // Initialize: each node in its own community. Sort for determinism —
    // HashMap key order is randomized per-run, and node processing order
    // changes the greedy Louvain result (and thus every downstream metric).
    let mut nodes: Vec<String> = adj.keys().cloned().collect();
    nodes.sort();
    let mut community_of: HashMap<String, usize> = HashMap::new();
    for (i, node) in nodes.iter().enumerate() {
        community_of.insert(node.clone(), i);
    }

    // Iterate: for each node, try moving to best neighbor community
    for _iteration in 0..20 {
        let mut changed = false;

        for node in &nodes {
            let node_comm = community_of[node];
            let node_deg = degree.get(node).copied().unwrap_or(0.0);
            let neighbors = match adj.get(node) {
                Some(n) => n,
                None => continue,
            };

            // Count edges to each neighboring community
            let mut comm_edges: HashMap<usize, f64> = HashMap::new();
            for neighbor in neighbors {
                let nc = community_of[neighbor];
                *comm_edges.entry(nc).or_default() += 1.0;
            }

            // Sum of degrees in each candidate community
            let mut comm_degree_sum: HashMap<usize, f64> = HashMap::new();
            for (n, &c) in &community_of {
                *comm_degree_sum.entry(c).or_default() += degree.get(n).copied().unwrap_or(0.0);
            }

            // Find best community (highest modularity gain)
            let mut best_comm = node_comm;
            let mut best_gain = 0.0f64;

            // Sorted candidate order so ties resolve to the lowest community
            // id deterministically (HashMap iteration order is randomized).
            let mut candidates: Vec<(usize, f64)> =
                comm_edges.iter().map(|(&c, &e)| (c, e)).collect();
            candidates.sort_by_key(|(c, _)| *c);
            for (candidate_comm, edges_to_comm) in candidates {
                if candidate_comm == node_comm {
                    continue;
                }
                let sigma_tot = comm_degree_sum.get(&candidate_comm).copied().unwrap_or(0.0);
                // Modularity gain of moving node to candidate_comm
                let gain = edges_to_comm / m - (sigma_tot * node_deg) / (2.0 * m * m);
                if gain > best_gain {
                    best_gain = gain;
                    best_comm = candidate_comm;
                }
            }

            if best_comm != node_comm {
                community_of.insert(node.clone(), best_comm);
                changed = true;
            }
        }

        if !changed {
            break;
        }
    }

    // Renumber communities to be contiguous 0..n
    let mut id_map: HashMap<usize, usize> = HashMap::new();
    let mut next_id = 0;
    for &c in community_of.values() {
        if !id_map.contains_key(&c) {
            id_map.insert(c, next_id);
            next_id += 1;
        }
    }
    for v in community_of.values_mut() {
        *v = id_map[v];
    }

    // Merge small communities (< 3 members) into nearest neighbor's community
    let mut members: HashMap<usize, Vec<String>> = HashMap::new();
    for (node, &comm) in &community_of {
        members.entry(comm).or_default().push(node.clone());
    }

    let small_communities: Vec<usize> = members.iter()
        .filter(|(_, m)| m.len() < 3)
        .map(|(&c, _)| c)
        .collect();

    for small_comm in small_communities {
        let small_nodes = members.remove(&small_comm).unwrap_or_default();
        for node in &small_nodes {
            // Find the most-connected neighboring community
            let mut neighbor_comm_counts: HashMap<usize, usize> = HashMap::new();
            if let Some(neighbors) = adj.get(node) {
                for n in neighbors {
                    let nc = community_of[n];
                    if nc != small_comm {
                        *neighbor_comm_counts.entry(nc).or_default() += 1;
                    }
                }
            }
            if let Some((&best_comm, _)) = neighbor_comm_counts.iter().max_by_key(|(_, &v)| v) {
                community_of.insert(node.clone(), best_comm);
                members.entry(best_comm).or_default().push(node.clone());
            }
        }
    }

    // Rebuild final communities list
    let mut final_members: HashMap<usize, Vec<String>> = HashMap::new();
    for (node, &comm) in &community_of {
        final_members.entry(comm).or_default().push(node.clone());
    }
    let mut communities: Vec<Vec<String>> = final_members.into_values().collect();
    communities.sort_by(|a, b| b.len().cmp(&a.len()));

    // Re-map community_of to match sorted order
    let mut sorted_map: HashMap<String, usize> = HashMap::new();
    for (i, comm) in communities.iter().enumerate() {
        for node in comm {
            sorted_map.insert(node.clone(), i);
        }
    }

    (sorted_map, communities)
}

// ── Betweenness Centrality (Brandes, sampled) ────────────────────────

/// Approximate betweenness centrality using Brandes' algorithm with sampling.
/// For graphs < 500 nodes, uses all nodes (exact). Otherwise samples 100.
pub fn betweenness_centrality(
    outgoing: &HashMap<String, Vec<String>>,
    incoming: &HashMap<String, Vec<String>>,
) -> HashMap<String, f64> {
    let adj = to_undirected(outgoing, incoming);
    let nodes: Vec<String> = adj.keys().cloned().collect();
    let n = nodes.len();
    if n == 0 { return HashMap::new(); }

    let mut centrality: HashMap<String, f64> = HashMap::new();
    for node in &nodes {
        centrality.insert(node.clone(), 0.0);
    }

    // Choose source nodes
    let sources: Vec<&String> = if n <= 500 {
        nodes.iter().collect()
    } else {
        // Deterministic sampling: pick every (n/100)th node
        let step = n / 100;
        nodes.iter().step_by(step.max(1)).take(100).collect()
    };

    for source in &sources {
        // BFS from source (Brandes)
        let mut stack: Vec<String> = Vec::new();
        let mut predecessors: HashMap<String, Vec<String>> = HashMap::new();
        let mut sigma: HashMap<String, f64> = HashMap::new(); // shortest path count
        let mut dist: HashMap<String, i64> = HashMap::new();
        let mut delta: HashMap<String, f64> = HashMap::new();

        for node in &nodes {
            sigma.insert(node.clone(), 0.0);
            dist.insert(node.clone(), -1);
            delta.insert(node.clone(), 0.0);
        }
        sigma.insert((*source).clone(), 1.0);
        dist.insert((*source).clone(), 0);

        let mut queue: VecDeque<String> = VecDeque::new();
        queue.push_back((*source).clone());

        while let Some(v) = queue.pop_front() {
            stack.push(v.clone());
            let v_dist = dist[&v];

            if let Some(neighbors) = adj.get(&v) {
                for w in neighbors {
                    let w_dist = dist[w];
                    if w_dist < 0 {
                        dist.insert(w.clone(), v_dist + 1);
                        queue.push_back(w.clone());
                    }
                    if dist[w] == v_dist + 1 {
                        *sigma.get_mut(w).unwrap() += sigma[&v];
                        predecessors.entry(w.clone()).or_default().push(v.clone());
                    }
                }
            }
        }

        // Accumulate
        while let Some(w) = stack.pop() {
            if let Some(preds) = predecessors.get(&w) {
                for v in preds {
                    let contribution = (sigma[v] / sigma[&w]) * (1.0 + delta[&w]);
                    *delta.get_mut(v).unwrap() += contribution;
                }
            }
            if &w != *source {
                *centrality.get_mut(&w).unwrap() += delta[&w];
            }
        }
    }

    // Normalize to 0-1
    let max_val = centrality.values().cloned().fold(0.0f64, f64::max);
    if max_val > 0.0 {
        for v in centrality.values_mut() {
            *v /= max_val;
        }
    }

    centrality
}

// ── PageRank ─────────────────────────────────────────────────────────

/// PageRank via power iteration. Damping factor 0.85, max 50 iterations.
pub fn pagerank(outgoing: &HashMap<String, Vec<String>>) -> HashMap<String, f64> {
    let mut all_nodes: HashSet<String> = HashSet::new();
    for (k, vs) in outgoing {
        all_nodes.insert(k.clone());
        for v in vs { all_nodes.insert(v.clone()); }
    }

    let nodes: Vec<String> = all_nodes.into_iter().collect();
    let n = nodes.len();
    if n == 0 { return HashMap::new(); }

    let d = 0.85;
    let init = 1.0 / n as f64;
    let mut rank: HashMap<String, f64> = HashMap::new();
    for node in &nodes {
        rank.insert(node.clone(), init);
    }

    for _iteration in 0..50 {
        let mut new_rank: HashMap<String, f64> = HashMap::new();
        for node in &nodes {
            new_rank.insert(node.clone(), (1.0 - d) / n as f64);
        }

        for (node, targets) in outgoing {
            if targets.is_empty() { continue; }
            let share = rank.get(node).copied().unwrap_or(0.0) * d / targets.len() as f64;
            for target in targets {
                *new_rank.entry(target.clone()).or_default() += share;
            }
        }

        rank = new_rank;
    }

    // Normalize to 0-1
    let max_val = rank.values().cloned().fold(0.0f64, f64::max);
    if max_val > 0.0 {
        for v in rank.values_mut() {
            *v /= max_val;
        }
    }

    rank
}

// ── Structural Importance (God Nodes) ────────────────────────────────

pub struct GodNode {
    pub name: String,
    pub score: f64,
    pub degree: usize,
    pub betweenness: f64,
    pub pagerank: f64,
}

/// Compute structural importance: 0.4*degree + 0.3*betweenness + 0.3*pagerank.
/// Returns top N nodes ranked by composite score.
pub fn god_nodes(
    outgoing: &HashMap<String, Vec<String>>,
    incoming: &HashMap<String, Vec<String>>,
    top_n: usize,
) -> Vec<GodNode> {
    let adj = to_undirected(outgoing, incoming);
    let bc = betweenness_centrality(outgoing, incoming);
    let pr = pagerank(outgoing);

    // Normalized degree
    let max_degree = adj.values().map(|v| v.len()).max().unwrap_or(1).max(1) as f64;

    let mut nodes: Vec<GodNode> = adj.iter().map(|(name, neighbors)| {
        let degree = neighbors.len();
        let norm_degree = degree as f64 / max_degree;
        let betweenness = bc.get(name).copied().unwrap_or(0.0);
        let pr_score = pr.get(name).copied().unwrap_or(0.0);
        let score = 0.4 * norm_degree + 0.3 * betweenness + 0.3 * pr_score;
        GodNode { name: name.clone(), score, degree, betweenness, pagerank: pr_score }
    }).collect();

    nodes.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    nodes.truncate(top_n);
    nodes
}

/// Find surprising connections: edges where endpoints are in different communities
/// and at least one endpoint has high betweenness centrality.
pub fn surprising_connections(
    outgoing: &HashMap<String, Vec<String>>,
    _incoming: &HashMap<String, Vec<String>>,
    community_of: &HashMap<String, usize>,
    bc: &HashMap<String, f64>,
    top_n: usize,
) -> Vec<(String, String, f64)> {
    let mut cross_edges: Vec<(String, String, f64)> = Vec::new();

    for (source, targets) in outgoing {
        let source_comm = community_of.get(source).copied().unwrap_or(usize::MAX);
        for target in targets {
            let target_comm = community_of.get(target).copied().unwrap_or(usize::MAX);
            if source_comm != target_comm {
                let score = bc.get(source).copied().unwrap_or(0.0)
                    + bc.get(target).copied().unwrap_or(0.0);
                cross_edges.push((source.clone(), target.clone(), score));
            }
        }
    }

    cross_edges.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
    cross_edges.truncate(top_n);
    cross_edges
}
