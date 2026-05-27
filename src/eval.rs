//! Retrieval-quality metrics — treats the vault graph as "RAG for the brain"
//! and measures how well links serve retrieval, rather than just counting
//! orphans. Two families:
//!
//!   * Link relevancy: share of edges that stay inside one topic community
//!     (intra-community = relevant; cross-community = likely spurious bridge).
//!   * Traversal-to-relevant: for probe queries, land on the single best
//!     search hit (seed) and measure what fraction of the search-relevant set
//!     is reachable within 1 and 2 hops, plus the mean hop distance. This is
//!     the graph's RAG lift: relevant neighbours you reach by following links.

use crate::graph;
use crate::server::LibraryServer;
use std::collections::{HashMap, HashSet, VecDeque};

pub struct PerQuery {
    pub query: String,
    pub relevant: usize,
    pub recall1: f64,
    pub recall2: f64,
    pub mean_hops: f64,
    /// Precision@10 of the 2-hop expansion in raw BFS order.
    pub raw_precision: f64,
    /// Precision@10 of the 2-hop expansion re-ranked by relevance to the query.
    pub ranked_precision: f64,
}

pub struct EvalReport {
    pub total_edges: usize,
    pub intra_community_pct: f64,
    pub largest_component_pct: f64,
    pub mean_recall1: f64,
    pub mean_recall2: f64,
    pub mean_hops: f64,
    pub mean_raw_precision: f64,
    pub mean_ranked_precision: f64,
    pub per_query: Vec<PerQuery>,
}

/// BFS over the undirected graph from `seed`, capped at `max_hops`. Returns
/// nodes in discovery order (excluding the seed) plus a depth lookup.
fn bfs_ordered(
    adj: &HashMap<String, HashSet<String>>,
    seed: &str,
    max_hops: usize,
) -> (Vec<String>, HashMap<String, usize>) {
    let mut depths = HashMap::new();
    let mut order = Vec::new();
    let mut queue = VecDeque::new();
    depths.insert(seed.to_string(), 0usize);
    queue.push_back(seed.to_string());
    while let Some(node) = queue.pop_front() {
        let d = depths[&node];
        if d >= max_hops {
            continue;
        }
        if let Some(nbrs) = adj.get(&node) {
            for n in nbrs {
                if !depths.contains_key(n) {
                    depths.insert(n.clone(), d + 1);
                    order.push(n.clone());
                    queue.push_back(n.clone());
                }
            }
        }
    }
    (order, depths)
}

/// Size of the largest connected component in the undirected graph.
fn largest_component(adj: &HashMap<String, HashSet<String>>) -> usize {
    let mut seen: HashSet<&String> = HashSet::new();
    let mut best = 0usize;
    for start in adj.keys() {
        if seen.contains(start) {
            continue;
        }
        let mut size = 0usize;
        let mut queue = VecDeque::new();
        queue.push_back(start);
        seen.insert(start);
        while let Some(node) = queue.pop_front() {
            size += 1;
            if let Some(nbrs) = adj.get(node) {
                for n in nbrs {
                    if !seen.contains(n) {
                        seen.insert(n);
                        queue.push_back(n);
                    }
                }
            }
        }
        best = best.max(size);
    }
    best
}

pub fn evaluate(
    server: &LibraryServer,
    queries: &[String],
    relevant_k: usize,
    max_hops: usize,
) -> EvalReport {
    let (outgoing, incoming) = {
        let c = server.cache.lock().unwrap();
        (c.outgoing.clone(), c.incoming.clone())
    };
    let adj = graph::to_undirected(&outgoing, &incoming);
    let (community_of, _) = graph::detect_communities(&outgoing, &incoming);

    // --- Link relevancy: intra-community share of undirected edges ---
    let mut total_edges = 0usize;
    let mut intra = 0usize;
    let mut seen_edges: HashSet<(String, String)> = HashSet::new();
    for (a, nbrs) in &adj {
        for b in nbrs {
            let key = if a <= b {
                (a.clone(), b.clone())
            } else {
                (b.clone(), a.clone())
            };
            if !seen_edges.insert(key) {
                continue;
            }
            total_edges += 1;
            if let (Some(ca), Some(cb)) = (community_of.get(a), community_of.get(b)) {
                if ca == cb {
                    intra += 1;
                }
            }
        }
    }
    let intra_pct = if total_edges > 0 {
        100.0 * intra as f64 / total_edges as f64
    } else {
        0.0
    };

    let n_nodes = adj.len();
    let cc_pct = if n_nodes > 0 {
        100.0 * largest_component(&adj) as f64 / n_nodes as f64
    } else {
        0.0
    };

    // --- Traversal-to-relevant per probe query ---
    let prec_k = 10usize;
    let mut per_query = Vec::new();
    for q in queries {
        // Broad scored search: relevant set = top `relevant_k`, score map for ranking.
        let scored: Vec<(String, f64)> = {
            let c = server.cache.lock().unwrap();
            c.search_index
                .search(q, 300)
                .iter()
                .filter_map(|(p, _, s)| {
                    p.file_stem().map(|st| (st.to_string_lossy().to_string(), *s))
                })
                .collect()
        };
        if scored.len() < 2 {
            continue;
        }
        let relevant: HashSet<String> =
            scored.iter().take(relevant_k).map(|(s, _)| s.clone()).collect();
        let score_of: HashMap<String, f64> = scored.iter().cloned().collect();
        let seed = scored[0].0.clone();

        let (order, depths) = bfs_ordered(&adj, &seed, max_hops);
        let total_rel = relevant.len() as f64;
        let reached1 = relevant
            .iter()
            .filter(|s| depths.get(*s).map_or(false, |d| *d <= 1))
            .count();
        let reached2 = relevant
            .iter()
            .filter(|s| depths.get(*s).map_or(false, |d| *d <= max_hops))
            .count();
        let hop_vals: Vec<usize> = relevant
            .iter()
            .filter_map(|s| depths.get(s).copied())
            .filter(|d| *d > 0)
            .collect();
        let mean_hop = if hop_vals.is_empty() {
            0.0
        } else {
            hop_vals.iter().sum::<usize>() as f64 / hop_vals.len() as f64
        };

        // Precision@k of the expansion: raw BFS order vs relevance re-ranked.
        let raw_top: Vec<&String> = order.iter().take(prec_k).collect();
        let raw_hit = raw_top.iter().filter(|s| relevant.contains(**s)).count();
        let raw_precision = if raw_top.is_empty() {
            0.0
        } else {
            raw_hit as f64 / raw_top.len() as f64
        };

        let mut ranked = order.clone();
        ranked.sort_by(|a, b| {
            score_of
                .get(b)
                .unwrap_or(&0.0)
                .partial_cmp(score_of.get(a).unwrap_or(&0.0))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let ranked_top: Vec<&String> = ranked.iter().take(prec_k).collect();
        let ranked_hit = ranked_top.iter().filter(|s| relevant.contains(**s)).count();
        let ranked_precision = if ranked_top.is_empty() {
            0.0
        } else {
            ranked_hit as f64 / ranked_top.len() as f64
        };

        per_query.push(PerQuery {
            query: q.clone(),
            relevant: relevant.len(),
            recall1: reached1 as f64 / total_rel,
            recall2: reached2 as f64 / total_rel,
            mean_hops: mean_hop,
            raw_precision,
            ranked_precision,
        });
    }

    let mean = |f: &dyn Fn(&PerQuery) -> f64| -> f64 {
        if per_query.is_empty() {
            0.0
        } else {
            per_query.iter().map(f).sum::<f64>() / per_query.len() as f64
        }
    };

    EvalReport {
        total_edges,
        intra_community_pct: intra_pct,
        largest_component_pct: cc_pct,
        mean_recall1: mean(&|p| p.recall1),
        mean_recall2: mean(&|p| p.recall2),
        mean_hops: mean(&|p| p.mean_hops),
        mean_raw_precision: mean(&|p| p.raw_precision),
        mean_ranked_precision: mean(&|p| p.ranked_precision),
        per_query,
    }
}
