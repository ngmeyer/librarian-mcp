use std::collections::HashMap;
use crate::graph::GodNode;

const TEMPLATE: &str = include_str!("templates/graph.html");

// Tokyo Night palette for cluster colors
const CLUSTER_PALETTE: &[&str] = &[
    "#7aa2f7", "#bb9af7", "#7dcfff", "#9ece6a", "#e0af68",
    "#f7768e", "#73daca", "#ff9e64", "#2ac3de", "#b4f9f8",
    "#c0caf5", "#a9b1d6", "#9aa5ce", "#565f89", "#414868",
];

pub fn generate_html(
    vault_name: &str,
    outgoing: &HashMap<String, Vec<String>>,
    community_of: &HashMap<String, usize>,
    communities: &[Vec<String>],
    god_nodes: &[GodNode],
) -> String {
    // Build importance map from god nodes
    let mut importance: HashMap<&str, f64> = HashMap::new();
    for gn in god_nodes {
        importance.insert(&gn.name, gn.score);
    }

    // Build degree map
    let mut degree: HashMap<String, usize> = HashMap::new();
    for (node, targets) in outgoing {
        *degree.entry(node.clone()).or_default() += targets.len();
        for t in targets {
            *degree.entry(t.clone()).or_default() += 1;
        }
    }

    // All nodes
    let mut all_nodes: std::collections::HashSet<String> = std::collections::HashSet::new();
    for (k, vs) in outgoing {
        all_nodes.insert(k.clone());
        for v in vs { all_nodes.insert(v.clone()); }
    }

    // Serialize nodes as JSON
    let nodes_json: Vec<String> = all_nodes.iter().map(|name| {
        let group = community_of.get(name).copied().unwrap_or(0);
        let size = importance.get(name.as_str()).copied().unwrap_or(0.0);
        let deg = degree.get(name).copied().unwrap_or(0);
        format!(
            r#"{{"id":"{}","label":"{}","group":{},"size":{:.3},"degree":{}}}"#,
            escape_json(name), escape_json(name), group, size, deg
        )
    }).collect();

    // Serialize edges as JSON
    let mut edges_json: Vec<String> = Vec::new();
    for (source, targets) in outgoing {
        for target in targets {
            edges_json.push(format!(
                r#"{{"from":"{}","to":"{}"}}"#,
                escape_json(source), escape_json(target)
            ));
        }
    }

    // Cluster colors
    let colors_json: Vec<String> = (0..communities.len().max(1))
        .map(|i| format!(r#""{}""#, CLUSTER_PALETTE[i % CLUSTER_PALETTE.len()]))
        .collect();

    // Cluster labels (most-connected node in each cluster)
    let labels_json: Vec<String> = communities.iter().map(|members| {
        let best = members.iter()
            .max_by_key(|m| degree.get(*m).copied().unwrap_or(0))
            .cloned()
            .unwrap_or_default();
        format!(r#""{}""#, escape_json(&best))
    }).collect();

    TEMPLATE
        .replace("{{VAULT_NAME}}", &escape_html(vault_name))
        .replace("{{GRAPH_NODES}}", &format!("[{}]", nodes_json.join(",")))
        .replace("{{GRAPH_EDGES}}", &format!("[{}]", edges_json.join(",")))
        .replace("{{CLUSTER_COLORS}}", &format!("[{}]", colors_json.join(",")))
        .replace("{{CLUSTER_LABELS}}", &format!("[{}]", labels_json.join(",")))
}

fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
