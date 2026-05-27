//! Topic index (MOC) generation. Rebuilds `Index/<Topic>.md` map-of-content
//! notes from the live graph so newly-added notes gain backlinks from their
//! topic hub instead of staying orphaned. Relatedness = full-text search hits
//! for the topic name, unioned with the topic's direct graph neighbors.

use crate::server::LibraryServer;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;

/// Extract the human description line from an existing MOC: the first prose
/// paragraph after the `# heading`, skipping the generated `**N related…`
/// line and section headers. Returns empty if none.
pub fn extract_description(content: &str) -> String {
    let mut seen_heading = false;
    for line in content.lines() {
        let t = line.trim();
        if t.starts_with("# ") {
            seen_heading = true;
            continue;
        }
        if !seen_heading || t.is_empty() {
            continue;
        }
        if t.starts_with("**") || t.starts_with("##") || t.starts_with("- ") || t.starts_with("[[") {
            return String::new();
        }
        return t.to_string();
    }
    String::new()
}

/// Build a MOC body for `topic`. Returns (body, related_count, dir_count).
pub fn generate_index_body(server: &LibraryServer, topic: &str, description: &str) -> (String, usize, usize) {
    let mut related: HashSet<String> = HashSet::new(); // relative paths

    {
        let cache = server.cache.lock().unwrap();

        // stem -> first relative path, for resolving graph neighbors to files
        let mut stem_to_rel: HashMap<String, String> = HashMap::new();
        for (_match_term, canonical, rel) in &cache.titles {
            stem_to_rel.entry(canonical.clone()).or_insert_with(|| rel.clone());
        }

        // 1. Full-text search hits for the topic name — this is what pulls in
        //    notes that mention the topic but were never linked to it.
        for (path, _content, _score) in cache.search_index.search(topic, 80) {
            related.insert(server.relative_path(&path));
        }

        // 2. Direct graph neighbors of the topic (both directions).
        let mut neighbor_stems: HashSet<String> = HashSet::new();
        if let Some(outs) = cache.outgoing.get(topic) {
            neighbor_stems.extend(outs.iter().cloned());
        }
        if let Some(ins) = cache.incoming.get(topic) {
            neighbor_stems.extend(ins.iter().cloned());
        }
        for stem in neighbor_stems {
            if let Some(rel) = stem_to_rel.get(&stem) {
                related.insert(rel.clone());
            }
        }
    }

    // Exclude other Index notes, and anything that would cross an isolated
    // folder boundary relative to the hub (hubs live in Index/).
    related.retain(|rel| {
        !rel.starts_with("Index/")
            && !server.crosses_isolation("Index", LibraryServer::top_folder(rel))
    });

    // Group by parent directory.
    let mut by_dir: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for rel in &related {
        let p = PathBuf::from(rel);
        let dir = p
            .parent()
            .map(|d| d.to_string_lossy().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "(root)".to_string());
        if let Some(stem) = p.file_stem().map(|s| s.to_string_lossy().to_string()) {
            if !stem.is_empty() && stem != topic {
                by_dir.entry(dir).or_default().push(stem);
            }
        }
    }
    for v in by_dir.values_mut() {
        v.sort();
        v.dedup();
    }

    let dir_count = by_dir.len();
    let related_count: usize = by_dir.values().map(|v| v.len()).sum();
    let date = chrono::Local::now().format("%Y-%m-%d").to_string();

    let mut body = String::new();
    body.push_str(&format!(
        "---\ntitle: \"{}\"\ntype: index\nauto-generated: true\ndate: {}\naliases: [{}]\n---\n\n",
        topic,
        date,
        topic.to_lowercase()
    ));
    body.push_str(&format!("# {}\n\n", topic));
    if !description.is_empty() {
        body.push_str(&format!("{}\n\n", description));
    }
    body.push_str(&format!(
        "**{} related notes** across {} directories.\n",
        related_count, dir_count
    ));
    for (dir, stems) in &by_dir {
        body.push_str(&format!("\n## {}\n\n", dir));
        for stem in stems {
            body.push_str(&format!("- [[{}]]\n", stem));
        }
    }

    (body, related_count, dir_count)
}
