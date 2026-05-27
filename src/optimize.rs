//! Generic autoresearch loop that optimizes a vault's graph for retrieval.
//! Measure -> act -> simulate -> re-measure, driving up the two metrics that
//! matter for RAG: link relevancy (intra-community edge share) and
//! traversal-to-relevant recall. Everything is derived from the graph itself
//! (Louvain communities, BM25 similarity) so it works on any vault.
//!
//! Two moves, both of which only ever add *intra-community* edges, so they
//! cannot reintroduce the cross-domain noise the stoplist removed:
//!   * Hub generation — give every sizable community a map-of-content note.
//!   * Densification — link each note to its most-similar same-community peers.
//!
//! Dry-run by default: proposed actions are simulated on a cloned graph and the
//! projected metric delta is reported; nothing is written unless `apply`.

use crate::graph;
use crate::server::LibraryServer;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

pub struct Snapshot {
    pub edges: usize,
    pub intra_pct: f64,
    pub recall2: f64,
    pub orphans: usize,
}

pub struct OptimizeReport {
    pub before: Snapshot,
    pub after: Snapshot,
    pub iterations: usize,
    pub hubs: Vec<(String, usize)>,
    pub links_added: usize,
    pub link_examples: Vec<(String, String)>,
    pub applied: bool,
}

type Graph = HashMap<String, Vec<String>>;

fn add_edge(out: &mut Graph, inc: &mut Graph, a: &str, b: &str) {
    let outs = out.entry(a.to_string()).or_default();
    if !outs.iter().any(|x| x == b) {
        outs.push(b.to_string());
    }
    inc.entry(b.to_string()).or_default().push(a.to_string());
}

fn already_linked(out: &Graph, inc: &Graph, a: &str, b: &str) -> bool {
    out.get(a).map_or(false, |v| v.iter().any(|x| x == b))
        || out.get(b).map_or(false, |v| v.iter().any(|x| x == a))
        || inc.get(a).map_or(false, |v| v.iter().any(|x| x == b))
}

/// Metrics on a given (possibly simulated) graph. `file_stems` is the set of
/// stems that are real notes (for the orphan = no-inbound-link count).
fn snapshot(
    server: &LibraryServer,
    out: &Graph,
    inc: &Graph,
    file_stems: &HashSet<String>,
    queries: &[String],
) -> Snapshot {
    let adj = graph::to_undirected(out, inc);
    let (community_of, _) = graph::detect_communities(out, inc);

    let mut total = 0usize;
    let mut intra = 0usize;
    let mut seen: HashSet<(String, String)> = HashSet::new();
    for (a, nbrs) in &adj {
        for b in nbrs {
            let key = if a <= b { (a.clone(), b.clone()) } else { (b.clone(), a.clone()) };
            if !seen.insert(key) {
                continue;
            }
            total += 1;
            if let (Some(x), Some(y)) = (community_of.get(a), community_of.get(b)) {
                if x == y {
                    intra += 1;
                }
            }
        }
    }
    let intra_pct = if total > 0 { 100.0 * intra as f64 / total as f64 } else { 0.0 };

    // Orphans: file notes with no inbound link.
    let mut linked_to: HashSet<&String> = HashSet::new();
    for targets in out.values() {
        for t in targets {
            linked_to.insert(t);
        }
    }
    let orphans = file_stems.iter().filter(|s| !linked_to.contains(*s)).count();

    // recall@2hops averaged over probe queries.
    let mut recalls = Vec::new();
    for q in queries {
        let hits: Vec<String> = {
            let c = server.cache.lock().unwrap();
            c.search_index
                .search(q, 20)
                .iter()
                .filter_map(|(p, _, _)| p.file_stem().map(|s| s.to_string_lossy().to_string()))
                .collect()
        };
        if hits.len() < 2 {
            continue;
        }
        let relevant: HashSet<String> = hits.iter().cloned().collect();
        let seed = &hits[0];
        // BFS 2 hops over `adj`.
        let mut depth = HashMap::new();
        let mut queue = VecDeque::new();
        depth.insert(seed.clone(), 0usize);
        queue.push_back(seed.clone());
        while let Some(n) = queue.pop_front() {
            let d = depth[&n];
            if d >= 2 {
                continue;
            }
            if let Some(ns) = adj.get(&n) {
                for x in ns {
                    if !depth.contains_key(x) {
                        depth.insert(x.clone(), d + 1);
                        queue.push_back(x.clone());
                    }
                }
            }
        }
        let reached = relevant.iter().filter(|s| depth.contains_key(*s)).count();
        recalls.push(reached as f64 / relevant.len() as f64);
    }
    let recall2 = if recalls.is_empty() {
        0.0
    } else {
        recalls.iter().sum::<f64>() / recalls.len() as f64
    };

    Snapshot { edges: total, intra_pct, recall2, orphans }
}

/// Highest-degree member of a community, sanitized as a hub note name.
fn community_label(members: &[String], adj: &HashMap<String, HashSet<String>>) -> String {
    members
        .iter()
        .max_by_key(|m| adj.get(*m).map_or(0, |n| n.len()))
        .cloned()
        .unwrap_or_default()
}

pub struct PlannedAction {
    /// Hub note stem -> member stems to link from it.
    pub hubs: BTreeMap<String, Vec<String>>,
    /// Note stem -> peers to add under its Related(auto) block.
    pub densify: BTreeMap<String, Vec<String>>,
}

pub fn optimize(
    server: &LibraryServer,
    iterations: usize,
    min_community: usize,
    max_links_per_note: usize,
    do_hubs: bool,
    do_densify: bool,
    apply: bool,
) -> (OptimizeReport, PlannedAction) {
    // Base graph + file stems + stem->rel map + distinctive term sets (built
    // once; densification scores similarity locally instead of re-searching).
    let (mut out, mut inc, file_stems, stem_to_rel, term_sets) = {
        let c = server.cache.lock().unwrap();
        let mut stems = HashSet::new();
        let mut s2r = HashMap::new();
        for (_m, canonical, rel) in &c.titles {
            stems.insert(canonical.clone());
            s2r.entry(canonical.clone()).or_insert_with(|| rel.clone());
        }
        let total = c.search_index.total_docs.max(1);
        let mut ts: HashMap<String, HashSet<String>> = HashMap::new();
        for (path, content) in &c.search_index.files {
            let Some(stem) = path.file_stem().map(|s| s.to_string_lossy().to_string()) else {
                continue;
            };
            let mut terms = HashSet::new();
            for w in content.to_lowercase().split(|ch: char| !ch.is_alphanumeric()) {
                if w.len() < 4 {
                    continue;
                }
                let df = c.search_index.doc_freq.get(w).copied().unwrap_or(0);
                // Skip words absent from the index and very common ones (low signal).
                if df == 0 || df * 2 > total {
                    continue;
                }
                terms.insert(w.to_string());
            }
            ts.insert(stem, terms);
        }
        (c.outgoing.clone(), c.incoming.clone(), stems, s2r, ts)
    };

    // Existing Index hubs (don't recreate).
    let existing_hubs: HashSet<String> = stem_to_rel
        .iter()
        .filter(|(_, rel)| rel.starts_with("Index/"))
        .map(|(stem, _)| stem.clone())
        .collect();

    // Probe queries: community labels on the base graph (generic, graph-derived).
    let (_base_co, base_comms) = graph::detect_communities(&out, &inc);
    let base_adj = graph::to_undirected(&out, &inc);
    let queries: Vec<String> = base_comms
        .iter()
        .filter(|m| m.len() >= min_community)
        .map(|m| community_label(m, &base_adj))
        .filter(|l| !l.is_empty())
        .collect();

    let before = snapshot(server, &out, &inc, &file_stems, &queries);

    let mut hubs: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut densify: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut links_added = 0usize;
    let mut link_examples = Vec::new();
    let mut created_hub_stems: HashSet<String> = HashSet::new();

    for _ in 0..iterations.max(1) {
        let (community_of, communities) = graph::detect_communities(&out, &inc);
        let adj = graph::to_undirected(&out, &inc);

        // --- Hub generation ---
        if do_hubs {
            for members in &communities {
                if members.len() < min_community {
                    continue;
                }
                let label = community_label(members, &adj);
                if label.is_empty()
                    || existing_hubs.contains(&label)
                    || created_hub_stems.contains(&label)
                {
                    continue;
                }
                // Link the hub to community members that are real notes.
                let linked: Vec<String> = members
                    .iter()
                    .filter(|m| **m != label && file_stems.contains(*m))
                    .cloned()
                    .collect();
                if linked.len() < min_community {
                    continue;
                }
                for m in &linked {
                    add_edge(&mut out, &mut inc, &label, m);
                }
                created_hub_stems.insert(label.clone());
                hubs.insert(label.clone(), linked.clone());
            }
        }

        // --- Intra-community densification (community-local, no search) ---
        if do_densify {
            let mut by_comm: HashMap<usize, Vec<String>> = HashMap::new();
            for s in &file_stems {
                if let Some(&c) = community_of.get(s) {
                    by_comm.entry(c).or_default().push(s.clone());
                }
            }
            for members in by_comm.values() {
                if members.len() < 2 {
                    continue;
                }
                for a in members {
                    let Some(ta) = term_sets.get(a) else { continue };
                    if ta.is_empty() {
                        continue;
                    }
                    // Rank same-community peers by shared distinctive terms.
                    let mut scored: Vec<(usize, &String)> = members
                        .iter()
                        .filter(|b| *b != a)
                        .filter_map(|b| {
                            let shared = term_sets.get(b).map_or(0, |tb| ta.intersection(tb).count());
                            if shared >= 3 { Some((shared, b)) } else { None }
                        })
                        .collect();
                    scored.sort_by(|x, y| y.0.cmp(&x.0).then_with(|| x.1.cmp(y.1)));

                    let mut added = 0usize;
                    for (_shared, b) in scored {
                        if added >= max_links_per_note {
                            break;
                        }
                        if already_linked(&out, &inc, a, b) {
                            continue;
                        }
                        add_edge(&mut out, &mut inc, a, b);
                        densify.entry(a.clone()).or_default().push(b.clone());
                        if link_examples.len() < 8 {
                            link_examples.push((a.clone(), b.clone()));
                        }
                        links_added += 1;
                        added += 1;
                    }
                }
            }
        }
    }

    let after = snapshot(server, &out, &inc, &file_stems, &queries);

    let report = OptimizeReport {
        before,
        after,
        iterations: iterations.max(1),
        hubs: hubs.iter().map(|(k, v)| (k.clone(), v.len())).collect(),
        links_added,
        link_examples,
        applied: apply,
    };
    (report, PlannedAction { hubs, densify })
}
