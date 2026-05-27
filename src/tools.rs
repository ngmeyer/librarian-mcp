use rmcp::{
    ErrorData as McpError, ServerHandler,
    handler::server::wrapper::Parameters,
    model::*,
    schemars, tool, tool_handler, tool_router,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::server::LibraryServer;
use crate::graph;
use crate::viz;
use crate::report;

// ── Tool parameter types ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SearchParams {
    /// Text query to search for in vault files
    pub query: String,
    /// Maximum number of results (default 20)
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ReadParams {
    /// Relative path to file within the vault
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WriteParams {
    /// Relative path to file within the vault
    pub path: String,
    /// File content (markdown)
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ListParams {
    /// Subdirectory to list (omit for vault root)
    pub directory: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct LinksParams {
    /// Relative path to file to find links for
    pub path: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct IndexParams {
    /// Topic to regenerate (matches an Index/<Topic>.md note). Omit to
    /// regenerate every existing Index/*.md hub.
    pub topic: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct EvalParams {
    /// Probe queries to evaluate retrieval against. Omit to use the existing
    /// Index/<Topic>.md topic names as probes.
    pub queries: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OptimizeParams {
    /// Refinement rounds (default 3). Each round re-detects communities.
    pub iterations: Option<usize>,
    /// Minimum community size to get a hub / be optimized (default 4).
    pub min_community: Option<usize>,
    /// Max intra-community links to add per note during densification (default 3).
    pub max_links_per_note: Option<usize>,
    /// Minimum shared distinctive terms for a densify link (default 3; raise for
    /// fewer, higher-confidence links).
    pub min_shared_terms: Option<usize>,
    /// Generate community hub MOCs (default true).
    pub hubs: Option<bool>,
    /// Add intra-community Related(auto) links in note bodies (default true).
    pub densify: Option<bool>,
    /// Write changes to the vault. Default false (dry-run: report projection only).
    pub apply: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TagsParams {
    /// Optional tag prefix filter
    pub filter: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MetadataParams {
    /// Relative path to file
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DailyParams {
    /// Date in YYYY-MM-DD format (defaults to today)
    pub date: Option<String>,
    /// Text to append to the daily note
    pub append: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ImportParams {
    /// Path to the source file to convert (PDF, DOCX, XLSX, image, audio, etc.)
    pub source_path: String,
    /// Relative path in the library to save the converted markdown
    pub library_path: String,
    /// Optional title for the frontmatter
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TraverseParams {
    /// Note title (file stem) to start traversal from
    pub start: String,
    /// Maximum number of hops (default 2)
    pub depth: Option<usize>,
    /// Optional tag filter — only include notes with this tag
    pub tag_filter: Option<String>,
    /// Optional query for relevance-weighted traversal: reached notes are
    /// scored by relevance to this query and returned most-relevant first,
    /// so the noisy graph neighbourhood is reordered into a RAG-style result.
    pub query: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ShortestPathParams {
    /// Starting note title (file stem)
    pub from: String,
    /// Target note title (file stem)
    pub to: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct VisualizeParams {
    /// Output file path within the vault (defaults to GRAPH_VIZ.html in vault root)
    pub output_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ReportParams {
    /// Output file path within the vault (defaults to GRAPH_REPORT.md in vault root)
    pub output_path: Option<String>,
}

// ── Tool implementations ──────────────────────────────────────────────

impl LibraryServer {
    /// Public access to the generated tool router for server construction.
    pub fn new_tool_router() -> rmcp::handler::server::router::tool::ToolRouter<Self> {
        Self::tool_router()
    }
}

#[tool_router]
#[allow(dead_code)]
impl LibraryServer {
    #[tool(description = "Search library files by text query. Returns matching file paths with context snippets. Uses an in-memory index for fast results.")]
    async fn library_search(
        &self,
        params: Parameters<SearchParams>,
    ) -> Result<CallToolResult, McpError> {
        let limit = params.0.limit.unwrap_or(20);
        let query = &params.0.query;
        let query_lower = query.to_lowercase();

        let cache = self.cache.lock().unwrap();
        let matches = cache.search_index.search(query, limit);
        drop(cache);

        let results: Vec<_> = matches.iter()
            .map(|(path, content, score)| {
                let lower = content.to_lowercase();
                let snippet = if let Some(pos) = lower.find(&query_lower) {
                    let start = pos.saturating_sub(80);
                    let end = (pos + query_lower.len() + 80).min(content.len());
                    content[start..end].replace('\n', " ")
                } else {
                    String::new()
                };
                serde_json::json!({
                    "path": self.relative_path(path),
                    "snippet": snippet,
                    "score": format!("{:.2}", score),
                })
            })
            .collect();

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&results).unwrap_or_default(),
        )]))
    }

    #[tool(description = "Read a file from the library by relative path.")]
    async fn library_read(
        &self,
        params: Parameters<ReadParams>,
    ) -> Result<CallToolResult, McpError> {
        let full = self.resolve_path(&params.0.path);
        match std::fs::read_to_string(&full) {
            Ok(content) => Ok(CallToolResult::success(vec![Content::text(content)])),
            Err(e) => Ok(CallToolResult::error(vec![Content::text(
                format!("Error reading {}: {}", params.0.path, e),
            )])),
        }
    }

    #[tool(description = "Write or create a file in the library. Auto-links mentions of existing notes as [[wikilinks]] using canonical file names for Obsidian graph compatibility. Creates parent directories if needed.")]
    async fn library_write(
        &self,
        params: Parameters<WriteParams>,
    ) -> Result<CallToolResult, McpError> {
        let full = self.resolve_path(&params.0.path);
        if let Some(parent) = full.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let titles = {
            let cache = self.cache.lock().unwrap();
            cache.titles.clone()
        };
        let (linked_content, links_added) = self.auto_link_content(&params.0.content, &params.0.path, &titles);
        match std::fs::write(&full, &linked_content) {
            Ok(_) => {
                if let Ok(mut cache) = self.cache.lock() {
                    cache.update_single_file(&full, &linked_content, self);
                }
                let link_msg = if links_added.is_empty() {
                    String::new()
                } else {
                    format!(", auto-linked: {}", links_added.join(", "))
                };
                let mut msg = format!("Written: {} ({} bytes{})", params.0.path, linked_content.len(), link_msg);

                // Write-amplification: suggest backlinks in other notes that
                // mention this note's title but don't yet link to it.
                if let Some(stem) = full.file_stem().and_then(|s| s.to_str()) {
                    let escaped = regex::escape(stem);
                    if let Ok(re) = regex::RegexBuilder::new(&format!(r"\b{}\b", escaped))
                        .case_insensitive(true)
                        .build()
                    {
                        let wikilink_plain = format!("[[{}]]", stem);
                        let wikilink_alias_prefix = format!("[[{}|", stem);
                        let mut suggestions: Vec<(String, String)> = Vec::new();

                        if let Ok(cache) = self.cache.lock() {
                            for (path, content) in &cache.search_index.files {
                                if path == &full {
                                    continue;
                                }
                                let content_lower = content.to_lowercase();
                                let wl_plain_lower = wikilink_plain.to_lowercase();
                                let wl_alias_lower = wikilink_alias_prefix.to_lowercase();
                                if content_lower.contains(&wl_plain_lower)
                                    || content_lower.contains(&wl_alias_lower)
                                {
                                    continue;
                                }
                                if let Some(m) = re.find(content) {
                                    let start = m.start().saturating_sub(30);
                                    let end = (m.end() + 30).min(content.len());
                                    // Clamp to char boundaries
                                    let start = content[..start]
                                        .char_indices()
                                        .last()
                                        .map(|(i, _)| i)
                                        .unwrap_or(0);
                                    let end = content[end..]
                                        .char_indices()
                                        .next()
                                        .map(|(i, _)| i + end)
                                        .unwrap_or(content.len());
                                    let snippet = content[start..end].replace('\n', " ");
                                    let rel = self.relative_path(path);
                                    suggestions.push((rel, format!("...{}...", snippet.trim())));
                                    if suggestions.len() >= 5 {
                                        break;
                                    }
                                }
                            }
                        }

                        if !suggestions.is_empty() {
                            msg.push_str(&format!(
                                "\n\nBacklink suggestions ({} notes mention \"{}\" but don't link to it):",
                                suggestions.len(),
                                stem
                            ));
                            for (path, snippet) in &suggestions {
                                msg.push_str(&format!("\n  - {}: \"{}\"", path, snippet));
                            }
                        }
                    }
                }

                Ok(CallToolResult::success(vec![Content::text(msg)]))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(
                format!("Error writing {}: {}", params.0.path, e),
            )])),
        }
    }

    #[tool(description = "List files and directories in the library. Optionally filter by subdirectory. When multiple vaults are configured, shows all vaults at root level.")]
    async fn library_list(
        &self,
        params: Parameters<ListParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut entries = Vec::new();

        if params.0.directory.is_none() && self.library_paths.len() > 1 {
            for root in &self.library_paths {
                let name = root.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| root.to_string_lossy().to_string());
                entries.push(serde_json::json!({
                    "name": name,
                    "type": "vault",
                    "path": name,
                }));
            }
        } else {
            let bases: Vec<std::path::PathBuf> = match &params.0.directory {
                Some(d) => vec![self.resolve_path(d)],
                None => self.library_paths.clone(),
            };

            for base in &bases {
                if let Ok(read) = std::fs::read_dir(base) {
                    for entry in read.flatten() {
                        let name = entry.file_name().to_string_lossy().to_string();
                        if name.starts_with('.') { continue; }
                        let is_dir = entry.file_type().map_or(false, |t| t.is_dir());
                        entries.push(serde_json::json!({
                            "name": name,
                            "type": if is_dir { "directory" } else { "file" },
                            "path": self.relative_path(&entry.path()),
                        }));
                    }
                }
            }
        }

        entries.sort_by(|a, b| {
            let at = a["type"].as_str().unwrap_or("");
            let bt = b["type"].as_str().unwrap_or("");
            at.cmp(bt).then(a["name"].as_str().unwrap_or("").cmp(b["name"].as_str().unwrap_or("")))
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&entries).unwrap_or_default(),
        )]))
    }

    #[tool(description = "Get backlinks (files linking TO this file) and outgoing links (files this file links TO).")]
    async fn library_links(
        &self,
        params: Parameters<LinksParams>,
    ) -> Result<CallToolResult, McpError> {
        let target_name = Path::new(&params.0.path)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();

        let mut cache = self.cache.lock().unwrap();
        cache.check_and_refresh(self);
        let outgoing_links = cache.outgoing.get(&target_name).cloned().unwrap_or_default();
        let backlinks: Vec<String> = cache.incoming.get(&target_name).cloned().unwrap_or_default();
        drop(cache);

        let result = serde_json::json!({
            "file": params.0.path,
            "backlinks": backlinks,
            "outgoing": outgoing_links,
            "backlink_count": backlinks.len(),
            "outgoing_count": outgoing_links.len(),
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_default(),
        )]))
    }

    #[tool(description = "List all tags in the library, or filter by prefix.")]
    async fn library_tags(
        &self,
        params: Parameters<TagsParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut tag_counts: HashMap<String, usize> = HashMap::new();

        for path in self.all_md_files() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                for tag in Self::extract_tags(&content) {
                    *tag_counts.entry(tag).or_insert(0) += 1;
                }
            }
        }

        let mut tags: Vec<_> = tag_counts.into_iter()
            .filter(|(tag, _)| params.0.filter.as_ref().map_or(true, |f| tag.starts_with(f.as_str())))
            .collect();
        tags.sort_by(|a, b| b.1.cmp(&a.1));

        let result: Vec<_> = tags.iter()
            .map(|(tag, count)| serde_json::json!({ "tag": tag, "count": count }))
            .collect();

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_default(),
        )]))
    }

    #[tool(description = "Read YAML frontmatter metadata from a library file.")]
    async fn library_metadata(
        &self,
        params: Parameters<MetadataParams>,
    ) -> Result<CallToolResult, McpError> {
        let full = self.resolve_path(&params.0.path);
        match std::fs::read_to_string(&full) {
            Ok(content) => {
                let fm = Self::extract_frontmatter(&content)
                    .unwrap_or_else(|| "No frontmatter found".to_string());
                Ok(CallToolResult::success(vec![Content::text(fm)]))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(format!("Error: {}", e))])),
        }
    }

    #[tool(description = "Create or append to a daily note. Defaults to today. Saves to the first vault.")]
    async fn library_daily(
        &self,
        params: Parameters<DailyParams>,
    ) -> Result<CallToolResult, McpError> {
        let date_str = params.0.date.unwrap_or_else(|| {
            chrono::Local::now().format("%Y-%m-%d").to_string()
        });
        let year = &date_str[..4];
        let rel_path = format!("Journal/{}/{}.md", year, date_str);
        let full = self.resolve_path(&rel_path);

        if let Some(parent) = full.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        if let Some(text) = params.0.append {
            let existing = std::fs::read_to_string(&full).unwrap_or_default();
            let new_content = format!("{}\n\n{}", existing.trim_end(), text);
            std::fs::write(&full, &new_content)
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;
            if let Ok(mut cache) = self.cache.lock() {
                cache.update_single_file(&full, &new_content, self);
            }
            Ok(CallToolResult::success(vec![Content::text(format!("Appended to {}", rel_path))]))
        } else if full.exists() {
            let content = std::fs::read_to_string(&full).unwrap_or_default();
            Ok(CallToolResult::success(vec![Content::text(content)]))
        } else {
            let content = format!("# {}\n\n", date_str);
            std::fs::write(&full, &content)
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;
            if let Ok(mut cache) = self.cache.lock() {
                cache.update_single_file(&full, &content, self);
            }
            Ok(CallToolResult::success(vec![Content::text(format!("Created {}", rel_path))]))
        }
    }

    #[tool(description = "Get library statistics: file count, total words, link count, orphans, tags. Aggregates across all configured vaults.")]
    async fn library_stats(&self) -> Result<CallToolResult, McpError> {
        let files = self.all_md_files();
        let mut total_words = 0usize;
        let mut total_links = 0usize;
        let mut all_link_targets = HashSet::new();
        let mut all_file_stems = HashSet::new();
        let mut tag_count = HashSet::new();

        for path in &files {
            let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            all_file_stems.insert(stem);

            if let Ok(content) = std::fs::read_to_string(path) {
                total_words += content.split_whitespace().count();
                let links = Self::extract_wikilinks(&content);
                total_links += links.len();
                for link in links { all_link_targets.insert(link); }
                for tag in Self::extract_tags(&content) { tag_count.insert(tag); }
            }
        }

        let orphans: Vec<_> = files.iter()
            .filter(|p| {
                let stem = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                !all_link_targets.contains(&stem)
            })
            .map(|p| self.relative_path(p))
            .collect();

        let result = serde_json::json!({
            "vaults": self.library_paths.iter().map(|p| p.to_string_lossy().to_string()).collect::<Vec<_>>(),
            "files": files.len(),
            "total_words": total_words,
            "total_links": total_links,
            "unique_tags": tag_count.len(),
            "orphan_count": orphans.len(),
            "orphans_sample": &orphans[..orphans.len().min(10)],
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_default(),
        )]))
    }

    #[tool(description = "Suggest wikilinks for a file. Scans content for mentions of existing note titles (including aliases) that aren't already linked. Returns suggestions without modifying the file.")]
    async fn library_suggest_links(
        &self,
        params: Parameters<ReadParams>,
    ) -> Result<CallToolResult, McpError> {
        let full = self.resolve_path(&params.0.path);
        let content = match std::fs::read_to_string(&full) {
            Ok(c) => c,
            Err(e) => return Ok(CallToolResult::error(vec![Content::text(format!("Error: {}", e))])),
        };

        let titles = {
            let cache = self.cache.lock().unwrap();
            cache.titles.clone()
        };
        let (_, suggestions) = self.auto_link_content(&content, &params.0.path, &titles);

        if suggestions.is_empty() {
            return Ok(CallToolResult::success(vec![Content::text(
                "No new link suggestions found — all mentions are already linked or no matches.".to_string(),
            )]));
        }

        let result = serde_json::json!({
            "file": params.0.path,
            "suggestions": suggestions,
            "count": suggestions.len(),
            "message": format!("Found {} potential links to add", suggestions.len()),
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_default(),
        )]))
    }

    #[tool(description = "Traverse the knowledge graph from a starting note using BFS. Returns all notes reachable within N hops, with their distance and connections. Useful for exploring a topic neighborhood.")]
    async fn library_traverse(
        &self,
        params: Parameters<TraverseParams>,
    ) -> Result<CallToolResult, McpError> {
        let max_depth = params.0.depth.unwrap_or(2);
        let start = &params.0.start;
        let mut cache_guard = self.cache.lock().unwrap();
        cache_guard.check_and_refresh(self);
        let outgoing = cache_guard.outgoing.clone();
        let incoming = cache_guard.incoming.clone();
        drop(cache_guard);

        let mut visited: HashMap<String, usize> = HashMap::new();
        let mut queue: std::collections::VecDeque<(String, usize)> = std::collections::VecDeque::new();
        let mut edges: Vec<(String, String)> = Vec::new();

        visited.insert(start.clone(), 0);
        queue.push_back((start.clone(), 0));

        while let Some((node, depth)) = queue.pop_front() {
            if depth >= max_depth { continue; }

            let mut neighbors = Vec::new();
            if let Some(out) = outgoing.get(&node) {
                neighbors.extend(out.iter().cloned());
            }
            if let Some(inc) = incoming.get(&node) {
                neighbors.extend(inc.iter().cloned());
            }

            for neighbor in neighbors {
                edges.push((node.clone(), neighbor.clone()));
                if !visited.contains_key(&neighbor) {
                    visited.insert(neighbor.clone(), depth + 1);
                    queue.push_back((neighbor.clone(), depth + 1));
                }
            }
        }

        // Collect (node, depth), honoring the optional tag filter.
        let mut node_pairs: Vec<(String, usize)> = visited
            .iter()
            .filter(|(node, _)| match &params.0.tag_filter {
                None => true,
                Some(tag_filter) => self.all_md_files().iter().any(|p| {
                    let stem = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                    if &stem != *node { return false; }
                    if let Ok(content) = std::fs::read_to_string(p) {
                        Self::extract_tags(&content).iter().any(|t| t == tag_filter)
                    } else { false }
                }),
            })
            .map(|(node, depth)| (node.clone(), *depth))
            .collect();

        // Relevance-weighted traversal: when a query is given, score reached
        // notes by relevance to it and return most-relevant first (RAG order).
        let scores: HashMap<String, f64> = match &params.0.query {
            Some(q) => {
                let cache = self.cache.lock().unwrap();
                cache.search_index.search(q, 300).iter()
                    .filter_map(|(p, _, s)| {
                        p.file_stem().map(|st| (st.to_string_lossy().to_string(), *s))
                    })
                    .collect()
            }
            None => HashMap::new(),
        };

        if params.0.query.is_some() {
            node_pairs.sort_by(|a, b| {
                let sb = scores.get(&b.0).copied().unwrap_or(0.0);
                let sa = scores.get(&a.0).copied().unwrap_or(0.0);
                sb.partial_cmp(&sa).unwrap_or(std::cmp::Ordering::Equal).then(a.1.cmp(&b.1))
            });
        } else {
            node_pairs.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
        }

        let nodes: Vec<_> = node_pairs
            .iter()
            .map(|(node, depth)| {
                if params.0.query.is_some() {
                    serde_json::json!({
                        "note": node,
                        "depth": depth,
                        "relevance": format!("{:.2}", scores.get(node).copied().unwrap_or(0.0)),
                    })
                } else {
                    serde_json::json!({ "note": node, "depth": depth })
                }
            })
            .collect();

        let mut unique_edges: Vec<(String, String)> = edges;
        unique_edges.sort();
        unique_edges.dedup();

        let result = serde_json::json!({
            "start": start,
            "max_depth": max_depth,
            "nodes_found": nodes.len(),
            "nodes": nodes,
            "edges": unique_edges.iter()
                .map(|(a, b)| serde_json::json!({ "from": a, "to": b }))
                .collect::<Vec<_>>(),
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_default(),
        )]))
    }

    #[tool(description = "Find the shortest link chain between two notes in the knowledge graph. Returns the path as a list of note titles, or reports if no path exists.")]
    async fn library_shortest_path(
        &self,
        params: Parameters<ShortestPathParams>,
    ) -> Result<CallToolResult, McpError> {
        let from = &params.0.from;
        let to = &params.0.to;
        let mut cache_guard = self.cache.lock().unwrap();
        cache_guard.check_and_refresh(self);
        let outgoing = cache_guard.outgoing.clone();
        let incoming = cache_guard.incoming.clone();
        drop(cache_guard);

        let mut visited: HashMap<String, String> = HashMap::new();
        let mut queue: std::collections::VecDeque<String> = std::collections::VecDeque::new();

        visited.insert(from.clone(), String::new());
        queue.push_back(from.clone());

        let mut found = false;
        while let Some(node) = queue.pop_front() {
            if &node == to {
                found = true;
                break;
            }

            let mut neighbors = Vec::new();
            if let Some(out) = outgoing.get(&node) {
                neighbors.extend(out.iter().cloned());
            }
            if let Some(inc) = incoming.get(&node) {
                neighbors.extend(inc.iter().cloned());
            }

            for neighbor in neighbors {
                if !visited.contains_key(&neighbor) {
                    visited.insert(neighbor.clone(), node.clone());
                    queue.push_back(neighbor);
                }
            }
        }

        if !found {
            return Ok(CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&serde_json::json!({
                    "from": from,
                    "to": to,
                    "path": null,
                    "message": "No path found between these notes"
                })).unwrap_or_default(),
            )]));
        }

        let mut path = Vec::new();
        let mut current = to.clone();
        while !current.is_empty() {
            path.push(current.clone());
            current = visited.get(&current).cloned().unwrap_or_default();
        }
        path.reverse();

        let result = serde_json::json!({
            "from": from,
            "to": to,
            "hops": path.len() - 1,
            "path": path,
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_default(),
        )]))
    }

    #[tool(description = "Analyze the vault's knowledge graph structure. Returns connected components (clusters), hub notes (most connected), bridge notes (connect otherwise separate clusters), and orphan notes (no links at all).")]
    async fn library_graph_analysis(&self) -> Result<CallToolResult, McpError> {
        let mut cache_guard = self.cache.lock().unwrap();
        cache_guard.check_and_refresh(self);
        let outgoing = cache_guard.outgoing.clone();
        let incoming = cache_guard.incoming.clone();
        drop(cache_guard);

        let mut all_nodes: HashSet<String> = HashSet::new();
        for (k, vs) in &outgoing {
            all_nodes.insert(k.clone());
            for v in vs { all_nodes.insert(v.clone()); }
        }
        for (k, vs) in &incoming {
            all_nodes.insert(k.clone());
            for v in vs { all_nodes.insert(v.clone()); }
        }

        // Connected components via BFS
        let mut visited: HashSet<String> = HashSet::new();
        let mut components: Vec<Vec<String>> = Vec::new();

        for node in &all_nodes {
            if visited.contains(node) { continue; }
            let mut component = Vec::new();
            let mut queue: std::collections::VecDeque<String> = std::collections::VecDeque::new();
            queue.push_back(node.clone());
            visited.insert(node.clone());

            while let Some(current) = queue.pop_front() {
                component.push(current.clone());
                let mut neighbors = Vec::new();
                if let Some(out) = outgoing.get(&current) { neighbors.extend(out.iter().cloned()); }
                if let Some(inc) = incoming.get(&current) { neighbors.extend(inc.iter().cloned()); }
                for n in neighbors {
                    if !visited.contains(&n) {
                        visited.insert(n.clone());
                        queue.push_back(n);
                    }
                }
            }
            component.sort();
            components.push(component);
        }
        components.sort_by(|a, b| b.len().cmp(&a.len()));

        // Hub notes
        let mut connection_counts: Vec<(String, usize)> = all_nodes.iter()
            .map(|node| {
                let out_count = outgoing.get(node).map_or(0, |v| v.len());
                let in_count = incoming.get(node).map_or(0, |v| v.len());
                (node.clone(), out_count + in_count)
            })
            .collect();
        connection_counts.sort_by(|a, b| b.1.cmp(&a.1));

        // Orphans
        let orphans: Vec<_> = all_nodes.iter()
            .filter(|node| {
                let out = outgoing.get(*node).map_or(0, |v| v.len());
                let inc = incoming.get(*node).map_or(0, |v| v.len());
                out == 0 && inc == 0
            })
            .cloned()
            .collect();

        // Bridges (approximation)
        let bridges: Vec<_> = connection_counts.iter()
            .filter(|(_, count)| *count >= 3)
            .take(10)
            .map(|(node, count)| serde_json::json!({ "note": node, "connections": count }))
            .collect();

        let result = serde_json::json!({
            "total_nodes": all_nodes.len(),
            "components": components.len(),
            "largest_component": components.first().map_or(0, |c| c.len()),
            "component_sizes": components.iter().map(|c| c.len()).collect::<Vec<_>>(),
            "hubs": connection_counts.iter().take(10)
                .map(|(node, count)| serde_json::json!({ "note": node, "connections": count }))
                .collect::<Vec<_>>(),
            "bridges": bridges,
            "orphan_count": orphans.len(),
            "orphans_sample": &orphans[..orphans.len().min(15)],
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_default(),
        )]))
    }

    #[tool(description = "Import any document (PDF, DOCX, XLSX, image, audio, etc.) into the library as markdown. Uses MarkItDown to convert, then saves with frontmatter and auto-linked wikilinks.")]
    async fn library_import(
        &self,
        params: Parameters<ImportParams>,
    ) -> Result<CallToolResult, McpError> {
        let source = &params.0.source_path;
        let lib_path = &params.0.library_path;
        let title = params.0.title.as_deref();

        if !std::path::Path::new(source).exists() {
            return Ok(CallToolResult::error(vec![Content::text(
                format!("Source file not found: {}", source),
            )]));
        }

        let output = std::process::Command::new("markitdown")
            .arg(source)
            .output();

        let markdown = match output {
            Ok(out) if out.status.success() => {
                String::from_utf8_lossy(&out.stdout).to_string()
            }
            Ok(out) => {
                let err = String::from_utf8_lossy(&out.stderr);
                return Ok(CallToolResult::error(vec![Content::text(
                    format!("markitdown failed: {}", err),
                )]));
            }
            Err(e) => {
                return Ok(CallToolResult::error(vec![Content::text(
                    format!("Failed to run markitdown (is it installed?): {}", e),
                )]));
            }
        };

        let source_name = std::path::Path::new(source)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let doc_title = title.unwrap_or(&source_name);
        let now = chrono::Local::now().format("%Y-%m-%d").to_string();

        let content = format!(
            "---\ntitle: {}\nsource: {}\nimported: {}\n---\n\n{}",
            doc_title, source_name, now, markdown.trim()
        );

        let titles = {
            let cache = self.cache.lock().unwrap();
            cache.titles.clone()
        };
        let (linked_content, links_added) = self.auto_link_content(&content, lib_path, &titles);
        let full_path = self.resolve_path(lib_path);
        if let Some(parent) = full_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        match std::fs::write(&full_path, &linked_content) {
            Ok(_) => {
                if let Ok(mut cache) = self.cache.lock() {
                    cache.update_single_file(&full_path, &linked_content, self);
                }
                let link_msg = if links_added.is_empty() {
                    String::new()
                } else {
                    format!(", auto-linked {} notes", links_added.len())
                };
                Ok(CallToolResult::success(vec![Content::text(
                    format!(
                        "Imported {} → {} ({} bytes, {} words{})",
                        source_name,
                        lib_path,
                        linked_content.len(),
                        linked_content.split_whitespace().count(),
                        link_msg,
                    ),
                )]))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(
                format!("Failed to write {}: {}", lib_path, e),
            )])),
        }
    }
    #[tool(description = "Detect topic communities in the vault's knowledge graph using modularity optimization. Returns cluster assignments with member lists, sizes, and topic labels (most-connected node per cluster).")]
    async fn library_cluster(&self) -> Result<CallToolResult, McpError> {
        let mut cache_guard = self.cache.lock().unwrap();
        cache_guard.check_and_refresh(self);
        let outgoing = cache_guard.outgoing.clone();
        let incoming = cache_guard.incoming.clone();
        drop(cache_guard);
        let (_community_of, communities) = graph::detect_communities(&outgoing, &incoming);

        let adj = graph::to_undirected(&outgoing, &incoming);

        let clusters: Vec<_> = communities.iter().enumerate().map(|(i, members)| {
            let label_hint = members.iter()
                .max_by_key(|m| adj.get(*m).map_or(0, |n| n.len()))
                .cloned()
                .unwrap_or_default();
            serde_json::json!({
                "id": i,
                "size": members.len(),
                "label_hint": label_hint,
                "members": members,
            })
        }).collect();

        let result = serde_json::json!({
            "cluster_count": communities.len(),
            "clusters": clusters,
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_default(),
        )]))
    }

    #[tool(description = "Generate an interactive HTML visualization of the vault's knowledge graph. Nodes are colored by community and sized by structural importance. Returns the file path written.")]
    async fn library_visualize(
        &self,
        params: Parameters<VisualizeParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut cache_guard = self.cache.lock().unwrap();
        cache_guard.check_and_refresh(self);
        let outgoing = cache_guard.outgoing.clone();
        let incoming = cache_guard.incoming.clone();
        drop(cache_guard);
        let (community_of, communities) = graph::detect_communities(&outgoing, &incoming);
        let gn = graph::god_nodes(&outgoing, &incoming, 10);

        let vault_name = self.library_paths.first()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "Vault".to_string());

        let html = viz::generate_html(&vault_name, &outgoing, &community_of, &communities, &gn);

        let rel_path = params.0.output_path.unwrap_or_else(|| "GRAPH_VIZ.html".to_string());
        let full = self.resolve_path(&rel_path);
        if let Some(parent) = full.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        match std::fs::write(&full, &html) {
            Ok(_) => Ok(CallToolResult::success(vec![Content::text(
                format!(
                    "Visualization written: {} ({} nodes, {} communities, {} KB)",
                    rel_path,
                    community_of.len(),
                    communities.len(),
                    html.len() / 1024,
                ),
            )])),
            Err(e) => Ok(CallToolResult::error(vec![Content::text(
                format!("Failed to write {}: {}", rel_path, e),
            )])),
        }
    }

    #[tool(description = "Run full vault analysis and generate a GRAPH_REPORT.md with god nodes (most structurally important notes), topic communities, surprising cross-community connections, and suggested questions. Returns the file path and summary.")]
    async fn library_report(
        &self,
        params: Parameters<ReportParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut cache_guard = self.cache.lock().unwrap();
        cache_guard.check_and_refresh(self);
        let outgoing = cache_guard.outgoing.clone();
        let incoming = cache_guard.incoming.clone();
        drop(cache_guard);
        let (community_of, communities) = graph::detect_communities(&outgoing, &incoming);
        let bc = graph::betweenness_centrality(&outgoing, &incoming);
        let god_nodes = graph::god_nodes(&outgoing, &incoming, 10);
        let surprising = graph::surprising_connections(&outgoing, &incoming, &community_of, &bc, 10);

        // Count totals
        let total_edges: usize = outgoing.values().map(|v| v.len()).sum();
        let mut all_nodes: HashSet<String> = HashSet::new();
        for (k, vs) in &outgoing {
            all_nodes.insert(k.clone());
            for v in vs { all_nodes.insert(v.clone()); }
        }
        for (k, vs) in &incoming {
            all_nodes.insert(k.clone());
            for v in vs { all_nodes.insert(v.clone()); }
        }
        let orphan_count = all_nodes.iter()
            .filter(|n| {
                outgoing.get(*n).map_or(0, |v| v.len()) == 0
                    && incoming.get(*n).map_or(0, |v| v.len()) == 0
            })
            .count();

        let vault_name = self.library_paths.first()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "Vault".to_string());

        let report_md = report::generate_report(
            &vault_name,
            all_nodes.len(),
            total_edges,
            &communities,
            &god_nodes,
            &surprising,
            orphan_count,
            &community_of,
        );

        let rel_path = params.0.output_path.unwrap_or_else(|| "GRAPH_REPORT.md".to_string());
        let full = self.resolve_path(&rel_path);
        if let Some(parent) = full.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        match std::fs::write(&full, &report_md) {
            Ok(_) => {
                if let Ok(mut cache) = self.cache.lock() {
                    cache.update_single_file(&full, &report_md, self);
                }
                let god_names: Vec<_> = god_nodes.iter().take(3).map(|g| g.name.clone()).collect();
                Ok(CallToolResult::success(vec![Content::text(
                    format!(
                        "Report written: {}\n\nSummary: {} notes, {} links, {} communities, {} orphans\nTop god nodes: {}\nSurprising connections: {}",
                        rel_path,
                        all_nodes.len(),
                        total_edges,
                        communities.len(),
                        orphan_count,
                        god_names.join(", "),
                        surprising.len(),
                    ),
                )]))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(
                format!("Failed to write {}: {}", rel_path, e),
            )])),
        }
    }

    #[tool(description = "Regenerate topic Index/<Topic>.md map-of-content (MOC) notes from the live graph so newly-added notes gain backlinks from their topic hub instead of staying orphaned. Pass `topic` to rebuild one hub, or omit to rebuild every existing Index/*.md. Relatedness = full-text search for the topic name plus the topic's graph neighbors, grouped by directory. Backs up overwritten notes to Index/.bak-<date>/ first.")]
    async fn library_index(
        &self,
        params: Parameters<IndexParams>,
    ) -> Result<CallToolResult, McpError> {
        {
            let mut cache = self.cache.lock().unwrap();
            cache.check_and_refresh(self);
        }

        let root = &self.library_paths[0];
        let index_dir = root.join("Index");

        let topics: Vec<String> = match &params.0.topic {
            Some(t) => vec![t.clone()],
            None => {
                let mut v = Vec::new();
                if let Ok(entries) = std::fs::read_dir(&index_dir) {
                    for e in entries.flatten() {
                        let p = e.path();
                        if p.extension().map_or(false, |x| x == "md") {
                            if let Some(stem) = p.file_stem() {
                                v.push(stem.to_string_lossy().to_string());
                            }
                        }
                    }
                }
                v.sort();
                v
            }
        };

        if topics.is_empty() {
            return Ok(CallToolResult::error(vec![Content::text(
                "No Index/ topics found. Create Index/<Topic>.md notes first, or pass a `topic`."
                    .to_string(),
            )]));
        }

        let date = chrono::Local::now().format("%Y-%m-%d").to_string();
        let backup_dir = index_dir.join(format!(".bak-{}", date));
        let mut total_links = 0usize;
        let mut lines = Vec::new();

        for topic in &topics {
            let note_path = index_dir.join(format!("{}.md", topic));
            let existing = std::fs::read_to_string(&note_path).unwrap_or_default();
            let description = crate::index::extract_description(&existing);

            // Back up the existing note before overwriting (reversible).
            if !existing.is_empty() {
                let _ = std::fs::create_dir_all(&backup_dir);
                let _ = std::fs::write(backup_dir.join(format!("{}.md", topic)), &existing);
            }

            let (body, related, dirs) = crate::index::generate_index_body(self, topic, &description);
            match std::fs::write(&note_path, &body) {
                Ok(_) => {
                    if let Ok(mut cache) = self.cache.lock() {
                        cache.update_single_file(&note_path, &body, self);
                    }
                    total_links += related;
                    lines.push(format!("  {} — {} notes / {} dirs", topic, related, dirs));
                }
                Err(e) => lines.push(format!("  {} — FAILED: {}", topic, e)),
            }
        }

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Regenerated {} index MOC(s); {} total backlinks written.\nBackup: Index/.bak-{}/\n\n{}",
            topics.len(),
            total_links,
            date,
            lines.join("\n"),
        ))]))
    }

    #[tool(description = "Evaluate the knowledge graph as RAG-for-the-brain. Reports link relevancy (share of edges within one topic community), connectivity (largest connected component), and traversal-to-relevant recall: for each probe query, land on the top search hit and measure what fraction of the top-20 search-relevant notes is reachable within 1 and 2 hops, plus mean hop distance. Pass `queries`, or omit to probe with the Index/ topic names.")]
    async fn library_eval(
        &self,
        params: Parameters<EvalParams>,
    ) -> Result<CallToolResult, McpError> {
        {
            let mut cache = self.cache.lock().unwrap();
            cache.check_and_refresh(self);
        }

        let queries: Vec<String> = match &params.0.queries {
            Some(q) if !q.is_empty() => q.clone(),
            _ => {
                let mut v = Vec::new();
                let index_dir = self.library_paths[0].join("Index");
                if let Ok(entries) = std::fs::read_dir(&index_dir) {
                    for e in entries.flatten() {
                        let p = e.path();
                        if p.extension().map_or(false, |x| x == "md") {
                            if let Some(s) = p.file_stem() {
                                v.push(s.to_string_lossy().to_string());
                            }
                        }
                    }
                }
                v.sort();
                v
            }
        };

        let report = crate::eval::evaluate(self, &queries, 20, 2);

        let mut out = String::new();
        out.push_str(&format!(
            "Graph retrieval quality (RAG simulation)\n\
             ── Structure ──\n\
             Edges: {} | Intra-community (relevant) links: {:.0}% | Largest connected component: {:.0}% of nodes\n\
             ── Traversal-to-relevant (top-20 search hits; seed = top hit; up to 2 hops) ──\n\
             Mean recall@1hop: {:.0}% | recall@2hops: {:.0}% | mean hops-to-relevant: {:.2}\n\
             ── Expansion precision@10 (relevant share of the 2-hop neighbourhood) ──\n\
             Raw BFS order: {:.0}% → relevance-ranked: {:.0}%  (lift +{:.0} pts) | probes: {}\n",
            report.total_edges,
            report.intra_community_pct,
            report.largest_component_pct,
            report.mean_recall1 * 100.0,
            report.mean_recall2 * 100.0,
            report.mean_hops,
            report.mean_raw_precision * 100.0,
            report.mean_ranked_precision * 100.0,
            (report.mean_ranked_precision - report.mean_raw_precision) * 100.0,
            report.per_query.len(),
        ));
        out.push_str("\nPer query  (recall@1 / recall@2 / hops | prec raw→ranked):\n");
        for p in &report.per_query {
            out.push_str(&format!(
                "  {:<26} {:>3.0}% / {:>3.0}% / {:.2} | {:>3.0}%→{:>3.0}%  (rel={})\n",
                p.query,
                p.recall1 * 100.0,
                p.recall2 * 100.0,
                p.mean_hops,
                p.raw_precision * 100.0,
                p.ranked_precision * 100.0,
                p.relevant
            ));
        }
        Ok(CallToolResult::success(vec![Content::text(out)]))
    }

    #[tool(description = "Autoresearch loop that optimizes the vault graph for retrieval (generic; works on any vault). Measures link relevancy (intra-community edge %) and traversal recall, then iteratively adds only intra-community edges via two graph-derived moves — community hub MOCs and same-community Related(auto) links — and re-measures the projected lift. Dry-run by default (reports projection + proposed actions); pass apply:true to write (backs up touched notes first).")]
    async fn library_optimize(
        &self,
        params: Parameters<OptimizeParams>,
    ) -> Result<CallToolResult, McpError> {
        {
            let mut cache = self.cache.lock().unwrap();
            cache.check_and_refresh(self);
        }
        let iterations = params.0.iterations.unwrap_or(3);
        let min_community = params.0.min_community.unwrap_or(4);
        let max_links = params.0.max_links_per_note.unwrap_or(3);
        let min_shared = params.0.min_shared_terms.unwrap_or(3);
        let do_hubs = params.0.hubs.unwrap_or(true);
        let do_densify = params.0.densify.unwrap_or(true);
        let apply = params.0.apply.unwrap_or(false);

        let (report, plan) = crate::optimize::optimize(
            self, iterations, min_community, max_links, min_shared, do_hubs, do_densify, apply,
        );

        let mut applied_note = String::new();
        if apply {
            let root = &self.library_paths[0];
            let date = chrono::Local::now().format("%Y-%m-%d").to_string();
            let backup = root.join(format!(".optimize-bak-{}", date));

            // stem -> relative path for resolving densify targets.
            let stem_to_rel: HashMap<String, String> = {
                let c = self.cache.lock().unwrap();
                let mut m = HashMap::new();
                for (_t, canonical, rel) in &c.titles {
                    m.entry(canonical.clone()).or_insert_with(|| rel.clone());
                }
                m
            };

            // Hubs: write a MOC linking exactly the planned community members.
            let hub_date = chrono::Local::now().format("%Y-%m-%d").to_string();
            for (label, members) in &plan.hubs {
                let note_path = root.join("Index").join(format!("{}.md", label));
                if let Ok(existing) = std::fs::read_to_string(&note_path) {
                    let bak = backup.join("Index").join(format!("{}.md", label));
                    if let Some(p) = bak.parent() { let _ = std::fs::create_dir_all(p); }
                    let _ = std::fs::write(bak, existing);
                }
                let mut by_dir: std::collections::BTreeMap<String, Vec<String>> = std::collections::BTreeMap::new();
                for m in members {
                    let rel = stem_to_rel.get(m).cloned().unwrap_or_default();
                    let dir = std::path::Path::new(&rel)
                        .parent()
                        .map(|d| d.to_string_lossy().to_string())
                        .filter(|s| !s.is_empty())
                        .unwrap_or_else(|| "(root)".to_string());
                    by_dir.entry(dir).or_default().push(m.clone());
                }
                let mut body = format!(
                    "---\ntitle: \"{}\"\ntype: index\nauto-generated: true\ndate: {}\nsource: library_optimize\n---\n\n# {}\n\n**{} related notes** (auto-clustered by community).\n",
                    label, hub_date, label, members.len()
                );
                for (dir, stems) in &by_dir {
                    body.push_str(&format!("\n## {}\n\n", dir));
                    let mut s = stems.clone();
                    s.sort();
                    for st in s {
                        body.push_str(&format!("- [[{}]]\n", st));
                    }
                }
                let _ = std::fs::create_dir_all(note_path.parent().unwrap());
                if std::fs::write(&note_path, &body).is_ok() {
                    if let Ok(mut c) = self.cache.lock() {
                        c.update_single_file(&note_path, &body, self);
                    }
                }
            }

            // Densify: upsert a managed Related(auto) block per note.
            for (stem, peers) in &plan.densify {
                let Some(rel) = stem_to_rel.get(stem) else { continue };
                let path = root.join(rel);
                let Ok(content) = std::fs::read_to_string(&path) else { continue };
                let bak = backup.join(rel);
                if let Some(p) = bak.parent() { let _ = std::fs::create_dir_all(p); }
                let _ = std::fs::write(&bak, &content);
                let updated = Self::upsert_related_block(&content, peers);
                if std::fs::write(&path, &updated).is_ok() {
                    if let Ok(mut c) = self.cache.lock() {
                        c.update_single_file(&path, &updated, self);
                    }
                }
            }
            applied_note = format!(
                "\nAPPLIED. Backed up touched notes to .optimize-bak-{}/\n",
                date
            );
        }

        let mut out = String::new();
        out.push_str(&format!(
            "Graph optimizer ({} mode, {} iteration(s))\n\n\
             Metric            before   after\n\
             Intra-community   {:>5.0}%  {:>5.0}%   ({:+.0} pts, link relevancy)\n\
             recall@2hops      {:>5.0}%  {:>5.0}%   ({:+.0} pts, retrieval)\n\
             Orphans           {:>6}  {:>6}   ({:+})\n\
             Edges             {:>6}  {:>6}\n\n\
             Proposed: {} community hub(s), {} intra-community link(s).\n{}",
            if apply { "APPLY" } else { "dry-run" },
            report.iterations,
            report.before.intra_pct, report.after.intra_pct,
            report.after.intra_pct - report.before.intra_pct,
            report.before.recall2 * 100.0, report.after.recall2 * 100.0,
            (report.after.recall2 - report.before.recall2) * 100.0,
            report.before.orphans, report.after.orphans,
            report.after.orphans as i64 - report.before.orphans as i64,
            report.before.edges, report.after.edges,
            report.hubs.len(), report.links_added,
            applied_note,
        ));
        if !report.hubs.is_empty() {
            out.push_str("\nHubs:\n");
            for (label, n) in report.hubs.iter().take(20) {
                out.push_str(&format!("  [[{}]] — {} members\n", label, n));
            }
        }
        if !report.link_examples.is_empty() {
            out.push_str("\nExample links:\n");
            for (a, b) in &report.link_examples {
                out.push_str(&format!("  {} → {}\n", a, b));
            }
        }
        if !apply {
            out.push_str("\nRe-run with apply:true to write these changes.\n");
        }
        Ok(CallToolResult::success(vec![Content::text(out)]))
    }

    /// Insert or replace a managed `## Related (auto)` block of wikilinks.
    fn upsert_related_block(content: &str, peers: &[String]) -> String {
        const MARK: &str = "## Related (auto)";
        let links = peers
            .iter()
            .map(|p| format!("- [[{}]]", p))
            .collect::<Vec<_>>()
            .join("\n");
        let block = format!("{}\n\n{}\n", MARK, links);
        if let Some(pos) = content.find(MARK) {
            let rest = &content[pos + MARK.len()..];
            let end = rest.find("\n## ").map(|i| pos + MARK.len() + i + 1).unwrap_or(content.len());
            format!("{}{}{}", &content[..pos], block, &content[end..])
        } else {
            let sep = if content.ends_with('\n') { "\n" } else { "\n\n" };
            format!("{}{}{}", content, sep, block)
        }
    }
}

// ── Server handler ────────────────────────────────────────────────────

#[tool_handler]
impl ServerHandler for LibraryServer {
    // `#[tool_handler]` generates call_tool/list_tools but NOT get_info, so the
    // default get_info() advertises empty capabilities. Clients that honor the
    // advertised capabilities (Claude Code logs `hasTools:false`) then skip
    // tools/list entirely and load zero tools. Declare the tools capability here.
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::VaultCache;
    use std::sync::{Arc, Mutex};

    // Regression guard: a refactor or rmcp bump must not silently drop the
    // advertised tools capability. Without it, clients log `hasTools:false`
    // and never call tools/list, so zero tools load.
    #[test]
    fn advertises_tools_capability() {
        let server = LibraryServer {
            library_paths: vec![],
            default_ignores: vec![],
            link_stoplist: vec![],
            cache: Arc::new(Mutex::new(VaultCache::default())),
            tool_router: LibraryServer::new_tool_router(),
        };
        assert!(
            server.get_info().capabilities.tools.is_some(),
            "server must advertise the tools capability"
        );
    }

    // The auto-linker must skip stoplisted generic stems (e.g. "claude") while
    // still linking real topic notes, so generic words stop polluting the graph.
    #[test]
    fn auto_link_respects_stoplist() {
        let server = LibraryServer {
            library_paths: vec![],
            default_ignores: vec![],
            link_stoplist: vec!["claude".to_string()],
            cache: Arc::new(Mutex::new(VaultCache::default())),
            tool_router: LibraryServer::new_tool_router(),
        };
        let titles = vec![
            ("Claude".to_string(), "Claude".to_string(), "Index/Claude.md".to_string()),
            ("QuantFlow".to_string(), "QuantFlow".to_string(), "Index/QuantFlow.md".to_string()),
        ];
        let (out, added) =
            server.auto_link_content("Ask Claude about QuantFlow tuning.", "note.md", &titles);
        assert!(!added.iter().any(|l| l == "Claude"), "stoplisted stem must not link");
        assert!(added.iter().any(|l| l == "QuantFlow"), "real topic must still link");
        assert!(out.contains("[[QuantFlow]]") && !out.contains("[[Claude]]"));
    }
}
