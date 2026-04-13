use std::collections::HashMap;
use crate::graph::GodNode;

/// Generate a GRAPH_REPORT.md markdown string from analysis results.
pub fn generate_report(
    vault_name: &str,
    total_nodes: usize,
    total_edges: usize,
    communities: &[Vec<String>],
    god_nodes: &[GodNode],
    surprising: &[(String, String, f64)],
    orphan_count: usize,
    community_of: &HashMap<String, usize>,
) -> String {
    let date = chrono::Local::now().format("%Y-%m-%d").to_string();

    let mut report = String::new();

    // Frontmatter
    report.push_str(&format!(
        "---\ntitle: \"Graph Report: {}\"\ndate: {}\ntype: report\ntags:\n  - vault/report\n  - auto-generated\n---\n\n",
        vault_name, date
    ));

    // Overview
    report.push_str(&format!("# Graph Report: {}\n\n", vault_name));
    report.push_str(&format!(
        "**Generated:** {} | **Notes:** {} | **Links:** {} | **Communities:** {} | **Orphans:** {}\n\n",
        date, total_nodes, total_edges, communities.len(), orphan_count
    ));

    // God Nodes
    report.push_str("## God Nodes\n\n");
    report.push_str("The most structurally important notes in your vault (by degree + betweenness centrality + PageRank).\n\n");
    report.push_str("| Rank | Note | Score | Connections | Betweenness | PageRank |\n");
    report.push_str("|------|------|-------|-------------|-------------|----------|\n");
    for (i, gn) in god_nodes.iter().enumerate() {
        report.push_str(&format!(
            "| {} | [[{}]] | {:.2} | {} | {:.2} | {:.2} |\n",
            i + 1, gn.name, gn.score, gn.degree, gn.betweenness, gn.pagerank
        ));
    }
    report.push('\n');

    // Communities
    report.push_str("## Communities\n\n");
    report.push_str("Topic clusters detected by modularity optimization.\n\n");
    for (i, members) in communities.iter().enumerate() {
        let label = members.iter()
            .max_by_key(|m| {
                // Use the member that appears most in god_nodes, or just first
                god_nodes.iter().position(|g| &g.name == *m).map(|p| 100 - p).unwrap_or(0)
            })
            .cloned()
            .unwrap_or_default();
        report.push_str(&format!(
            "### Community {} — [[{}]] ({} notes)\n\n",
            i + 1, label, members.len()
        ));
        let display_members: Vec<String> = members.iter()
            .take(15)
            .map(|m| format!("[[{}]]", m))
            .collect();
        report.push_str(&format!("{}", display_members.join(", ")));
        if members.len() > 15 {
            report.push_str(&format!(" ... and {} more", members.len() - 15));
        }
        report.push_str("\n\n");
    }

    // Surprising Connections
    if !surprising.is_empty() {
        report.push_str("## Surprising Connections\n\n");
        report.push_str("High-betweenness edges between different communities — these are the cross-topic bridges.\n\n");
        for (source, target, score) in surprising {
            let s_comm = community_of.get(source).copied().unwrap_or(0);
            let t_comm = community_of.get(target).copied().unwrap_or(0);
            report.push_str(&format!(
                "- [[{}]] (community {}) → [[{}]] (community {}) — bridge score: {:.2}\n",
                source, s_comm + 1, target, t_comm + 1, score
            ));
        }
        report.push('\n');
    }

    // Suggested Questions
    report.push_str("## Suggested Questions\n\n");
    report.push_str("Questions this graph is uniquely positioned to answer:\n\n");

    if god_nodes.len() >= 2 {
        report.push_str(&format!(
            "1. How does [[{}]] connect to [[{}]]? (top two god nodes)\n",
            god_nodes[0].name, god_nodes[1].name
        ));
    }
    if communities.len() >= 2 {
        let c1_label = communities[0].first().cloned().unwrap_or_default();
        let c2_label = communities[1].first().cloned().unwrap_or_default();
        report.push_str(&format!(
            "2. What bridges the topics around [[{}]] and [[{}]]?\n",
            c1_label, c2_label
        ));
    }
    if !surprising.is_empty() {
        let (s, t, _) = &surprising[0];
        report.push_str(&format!(
            "3. Why are [[{}]] and [[{}]] connected across communities?\n",
            s, t
        ));
    }
    if orphan_count > 0 {
        report.push_str(&format!(
            "4. Which of the {} orphan notes should be linked into the graph?\n",
            orphan_count
        ));
    }
    report.push_str("5. What topic clusters are emerging that could become their own index note?\n");

    report
}
