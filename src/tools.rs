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

        let nodes: Vec<_> = if let Some(ref tag_filter) = params.0.tag_filter {
            visited.iter()
                .filter(|(node, _)| {
                    self.all_md_files().iter().any(|p| {
                        let stem = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                        if &stem != *node { return false; }
                        if let Ok(content) = std::fs::read_to_string(p) {
                            Self::extract_tags(&content).iter().any(|t| t == tag_filter)
                        } else { false }
                    })
                })
                .map(|(node, depth)| serde_json::json!({ "note": node, "depth": depth }))
                .collect()
        } else {
            visited.iter()
                .map(|(node, depth)| serde_json::json!({ "note": node, "depth": depth }))
                .collect()
        };

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
            cache: Arc::new(Mutex::new(VaultCache::default())),
            tool_router: LibraryServer::new_tool_router(),
        };
        assert!(
            server.get_info().capabilities.tools.is_some(),
            "server must advertise the tools capability"
        );
    }
}
