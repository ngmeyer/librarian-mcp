use rmcp::{
    ErrorData as McpError, ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    schemars, tool, tool_handler, tool_router,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Librarian MCP Server — give Claude a librarian for your markdown vault
#[derive(Clone)]
struct LibraryServer {
    /// One or more vault roots
    library_paths: Vec<PathBuf>,
    /// Default exclusion patterns when no .librarianignore exists
    default_ignores: Vec<String>,
    /// In-memory search index (trigram -> set of file indices)
    search_index: std::sync::Arc<Mutex<SearchIndex>>,
    tool_router: ToolRouter<Self>,
}

// ── Search index ─────────────────────────────────────────────────────

#[derive(Default)]
struct SearchIndex {
    /// All indexed files: (absolute path, content)
    files: Vec<(PathBuf, String)>,
    /// Trigram -> indices into `files`
    trigrams: HashMap<[u8; 3], Vec<usize>>,
}

impl SearchIndex {
    fn build(paths: &[(PathBuf, String)]) -> Self {
        let mut idx = SearchIndex {
            files: paths.to_vec(),
            trigrams: HashMap::new(),
        };
        for (i, (_path, content)) in paths.iter().enumerate() {
            let lower = content.to_lowercase();
            let bytes = lower.as_bytes();
            for window in bytes.windows(3) {
                let tri = [window[0], window[1], window[2]];
                idx.trigrams.entry(tri).or_default().push(i);
            }
        }
        // Deduplicate file indices per trigram
        for indices in idx.trigrams.values_mut() {
            indices.sort_unstable();
            indices.dedup();
        }
        idx
    }

    fn search(&self, query: &str, limit: usize) -> Vec<(PathBuf, String)> {
        let query_lower = query.to_lowercase();
        let query_bytes = query_lower.as_bytes();

        if query_bytes.len() < 3 {
            // Fall back to linear scan for very short queries
            return self.files.iter()
                .filter(|(_, content)| content.to_lowercase().contains(&query_lower))
                .take(limit)
                .map(|(p, c)| (p.clone(), c.clone()))
                .collect();
        }

        // Find candidate files that contain all trigrams from the query
        let mut candidate_sets: Vec<&Vec<usize>> = Vec::new();
        for window in query_bytes.windows(3) {
            let tri = [window[0], window[1], window[2]];
            match self.trigrams.get(&tri) {
                Some(indices) => candidate_sets.push(indices),
                None => return Vec::new(), // A trigram not found means no matches
            }
        }

        if candidate_sets.is_empty() {
            return Vec::new();
        }

        // Intersect all candidate sets
        let mut candidates: HashSet<usize> = candidate_sets[0].iter().copied().collect();
        for set in &candidate_sets[1..] {
            let other: HashSet<usize> = set.iter().copied().collect();
            candidates = candidates.intersection(&other).copied().collect();
        }

        // Verify actual substring match (trigrams can give false positives)
        candidates.iter()
            .filter_map(|&i| {
                let (path, content) = &self.files[i];
                if content.to_lowercase().contains(&query_lower) {
                    Some((path.clone(), content.clone()))
                } else {
                    None
                }
            })
            .take(limit)
            .collect()
    }

    fn update_file(&mut self, path: &Path, new_content: &str) {
        // Remove old entry if exists
        if let Some(idx) = self.files.iter().position(|(p, _)| p == path) {
            self.files[idx].1 = new_content.to_string();
            // Rebuild trigrams for this file (simple approach: full rebuild)
            // For a production system we'd do incremental updates
            *self = Self::build(&self.files);
        } else {
            // New file — add and rebuild
            self.files.push((path.to_path_buf(), new_content.to_string()));
            *self = Self::build(&self.files);
        }
    }
}

// ── Tool parameter types ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
struct SearchParams {
    /// Text query to search for in vault files
    query: String,
    /// Maximum number of results (default 20)
    limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
struct ReadParams {
    /// Relative path to file within the vault
    path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
struct WriteParams {
    /// Relative path to file within the vault
    path: String,
    /// File content (markdown)
    content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
struct ListParams {
    /// Subdirectory to list (omit for vault root)
    directory: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
struct LinksParams {
    /// Relative path to file to find links for
    path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
struct TagsParams {
    /// Optional tag prefix filter
    filter: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
struct MetadataParams {
    /// Relative path to file
    path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
struct DailyParams {
    /// Date in YYYY-MM-DD format (defaults to today)
    date: Option<String>,
    /// Text to append to the daily note
    append: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
struct ImportParams {
    /// Path to the source file to convert (PDF, DOCX, XLSX, image, audio, etc.)
    source_path: String,
    /// Relative path in the library to save the converted markdown (e.g., "Research/Deep Dives/imported-doc.md")
    library_path: String,
    /// Optional title for the frontmatter
    title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
struct TraverseParams {
    /// Note title (file stem) to start traversal from
    start: String,
    /// Maximum number of hops (default 2)
    depth: Option<usize>,
    /// Optional tag filter — only include notes with this tag
    tag_filter: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
struct ShortestPathParams {
    /// Starting note title (file stem)
    from: String,
    /// Target note title (file stem)
    to: String,
}

// ── Helpers ───────────────────────────────────────────────────────────

impl LibraryServer {
    /// Resolve a relative path against vault roots. Returns the first match,
    /// or falls back to the first vault root for new files.
    fn resolve_path(&self, rel: &str) -> PathBuf {
        for root in &self.library_paths {
            let candidate = root.join(rel);
            if candidate.exists() {
                return candidate;
            }
        }
        // Default to first vault for new files
        self.library_paths[0].join(rel)
    }

    /// Collect all markdown files across all vault roots, respecting .librarianignore.
    fn all_md_files(&self) -> Vec<PathBuf> {
        let mut files = Vec::new();
        for root in &self.library_paths {
            let mut builder = ignore::WalkBuilder::new(root);
            builder.hidden(true); // skip dotfiles by default

            // Check for .librarianignore at vault root
            let ignore_file = root.join(".librarianignore");
            if ignore_file.exists() {
                builder.add_ignore(ignore_file);
            } else {
                // Apply default exclusion patterns via a temporary override
                builder.filter_entry(move |entry| {
                    let path_str = entry.path().to_string_lossy();
                    !path_str.contains(".obsidian")
                        && !path_str.contains(".trash")
                        && !path_str.contains("node_modules")
                });
            }

            for entry in builder.build().flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |ext| ext == "md") && path.is_file() {
                    files.push(path.to_path_buf());
                }
            }
        }
        files
    }

    /// Get the relative path of an absolute path, trying each vault root.
    fn relative_path(&self, abs: &Path) -> String {
        for root in &self.library_paths {
            if let Ok(rel) = abs.strip_prefix(root) {
                return rel.to_string_lossy().to_string();
            }
        }
        abs.to_string_lossy().to_string()
    }

    fn extract_frontmatter(content: &str) -> Option<String> {
        if content.starts_with("---\n") {
            if let Some(end) = content[4..].find("\n---") {
                return Some(content[4..4 + end].to_string());
            }
        }
        None
    }

    /// Extract aliases from YAML frontmatter (supports both list and comma-separated formats).
    fn extract_aliases(content: &str) -> Vec<String> {
        let fm = match Self::extract_frontmatter(content) {
            Some(fm) => fm,
            None => return Vec::new(),
        };
        for line in fm.lines() {
            let trimmed = line.trim();
            // Handle "aliases: [a, b, c]" format
            if let Some(rest) = trimmed.strip_prefix("aliases:") {
                let rest = rest.trim();
                if rest.starts_with('[') {
                    return rest.trim_start_matches('[').trim_end_matches(']')
                        .split(',')
                        .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                }
                // Single value on same line
                if !rest.is_empty() {
                    return vec![rest.trim_matches('"').trim_matches('\'').to_string()];
                }
                // YAML list format on subsequent lines handled below
            }
            // Handle "- alias" items under aliases:
            if trimmed.starts_with("- ") {
                // This is a simplification — we'd need state to know we're under aliases:
                // For now, the [a, b, c] and single-line formats cover most Obsidian usage
            }
        }
        Vec::new()
    }

    /// Get all note titles and aliases for auto-linking.
    /// Returns (match_term, canonical_stem, relative_path) triples.
    fn all_note_titles(&self) -> Vec<(String, String, String)> {
        let mut titles = Vec::new();
        for p in self.all_md_files() {
            let stem = match p.file_stem() {
                Some(s) => s.to_string_lossy().to_string(),
                None => continue,
            };
            let rel = self.relative_path(&p);

            // Add the file stem itself as a matchable title
            titles.push((stem.clone(), stem.clone(), rel.clone()));

            // Add aliases from frontmatter
            if let Ok(content) = std::fs::read_to_string(&p) {
                for alias in Self::extract_aliases(&content) {
                    titles.push((alias, stem.clone(), rel.clone()));
                }
            }
        }
        titles
    }

    /// Auto-link: scan content for mentions of existing note titles and wrap them in [[wikilinks]].
    /// Uses canonical file stems for Obsidian graph-view compatibility.
    fn auto_link_content(&self, content: &str, exclude_path: &str) -> (String, Vec<String>) {
        let titles = self.all_note_titles();
        let existing_links = Self::extract_wikilinks(content);
        let existing_set: HashSet<&str> = existing_links.iter().map(|s| s.as_str()).collect();

        let mut result = content.to_string();
        let mut links_added = Vec::new();

        // Sort by match_term length descending so longer matches are found first
        let mut candidates: Vec<_> = titles.iter()
            .filter(|(match_term, canonical, rel)| {
                match_term.len() >= 3
                    && rel != exclude_path
                    && !existing_set.contains(canonical.as_str())
                    && !existing_set.contains(match_term.as_str())
            })
            .collect();
        candidates.sort_by(|a, b| b.0.len().cmp(&a.0.len()));

        // Track which canonical stems we've already linked to avoid duplicates
        let mut linked_stems: HashSet<String> = HashSet::new();

        for (match_term, canonical, _rel) in &candidates {
            if linked_stems.contains(canonical.as_str()) {
                continue;
            }

            let pattern = format!(r"(?i)\b{}\b", regex::escape(match_term));
            if let Ok(re) = regex::Regex::new(&pattern) {
                // Get body text (skip frontmatter)
                let fm_end = if result.starts_with("---\n") {
                    result[4..].find("\n---\n").map(|i| 4 + i + 5).unwrap_or(0)
                } else {
                    0
                };

                let body_part = &result[fm_end..];

                // Check not already linked
                if body_part.contains(&format!("[[{}]]", canonical))
                    || body_part.contains(&format!("[[{}|", canonical))
                {
                    continue;
                }

                if let Some(m) = re.find(body_part) {
                    // Use canonical file stem for the wikilink (R11: graph-view compatible)
                    let replacement = if match_term.to_lowercase() == canonical.to_lowercase() {
                        format!("[[{}]]", canonical)
                    } else {
                        // Alias match: use [[canonical|matched text]] format
                        let matched_text = &body_part[m.start()..m.end()];
                        format!("[[{}|{}]]", canonical, matched_text)
                    };
                    let new_body = format!(
                        "{}{}{}",
                        &body_part[..m.start()],
                        replacement,
                        &body_part[m.end()..]
                    );
                    result = format!("{}{}", &result[..fm_end], new_body);
                    links_added.push(canonical.to_string());
                    linked_stems.insert(canonical.to_string());
                }
            }
        }

        (result, links_added)
    }

    fn extract_wikilinks(content: &str) -> Vec<String> {
        let re = regex::Regex::new(r"\[\[([^\]|]+)(?:\|[^\]]+)?\]\]").unwrap();
        re.captures_iter(content)
            .map(|c| c[1].to_string())
            .collect()
    }

    fn extract_tags(content: &str) -> Vec<String> {
        let re = regex::Regex::new(r"(?:^|\s)#([\w/-]+)").unwrap();
        re.captures_iter(content)
            .map(|c| c[1].to_string())
            .collect()
    }

    /// Build a bidirectional adjacency list from all wikilinks in the vault.
    /// Keys and values are file stems (note titles). Returns (outgoing, incoming) maps.
    fn build_graph(&self) -> (HashMap<String, Vec<String>>, HashMap<String, Vec<String>>) {
        let mut outgoing: HashMap<String, Vec<String>> = HashMap::new();
        let mut incoming: HashMap<String, Vec<String>> = HashMap::new();

        for path in self.all_md_files() {
            let stem = match path.file_stem() {
                Some(s) => s.to_string_lossy().to_string(),
                None => continue,
            };
            outgoing.entry(stem.clone()).or_default();

            if let Ok(content) = std::fs::read_to_string(&path) {
                for link in Self::extract_wikilinks(&content) {
                    outgoing.entry(stem.clone()).or_default().push(link.clone());
                    incoming.entry(link).or_default().push(stem.clone());
                }
            }
        }

        (outgoing, incoming)
    }

    /// Build the initial search index from all vault files.
    fn build_search_index(&self) -> SearchIndex {
        let files: Vec<(PathBuf, String)> = self.all_md_files()
            .into_iter()
            .filter_map(|p| {
                let content = std::fs::read_to_string(&p).ok()?;
                Some((p, content))
            })
            .collect();
        eprintln!("Librarian: indexed {} files for search", files.len());
        SearchIndex::build(&files)
    }
}

// ── Tool implementations ──────────────────────────────────────────────

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

        let index = self.search_index.lock().unwrap();
        let matches = index.search(query, limit);
        drop(index);

        let results: Vec<_> = matches.iter()
            .map(|(path, content)| {
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
        let (linked_content, links_added) = self.auto_link_content(&params.0.content, &params.0.path);
        match std::fs::write(&full, &linked_content) {
            Ok(_) => {
                // Update search index
                if let Ok(mut index) = self.search_index.lock() {
                    index.update_file(&full, &linked_content);
                }
                let link_msg = if links_added.is_empty() {
                    String::new()
                } else {
                    format!(", auto-linked: {}", links_added.join(", "))
                };
                Ok(CallToolResult::success(vec![Content::text(
                    format!("Written: {} ({} bytes{})", params.0.path, linked_content.len(), link_msg),
                )]))
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
            // Multi-vault: show vault roots at top level
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
            let bases: Vec<PathBuf> = match &params.0.directory {
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

        let full = self.resolve_path(&params.0.path);
        let outgoing = if let Ok(content) = std::fs::read_to_string(&full) {
            Self::extract_wikilinks(&content)
        } else {
            vec![]
        };

        let mut backlinks = Vec::new();
        for file_path in self.all_md_files() {
            if file_path == full { continue; }
            if let Ok(content) = std::fs::read_to_string(&file_path) {
                let links = Self::extract_wikilinks(&content);
                if links.iter().any(|l| l == &target_name || l == &params.0.path) {
                    backlinks.push(self.relative_path(&file_path));
                }
            }
        }

        let result = serde_json::json!({
            "file": params.0.path,
            "backlinks": backlinks,
            "outgoing": outgoing,
            "backlink_count": backlinks.len(),
            "outgoing_count": outgoing.len(),
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
            if let Ok(mut index) = self.search_index.lock() {
                index.update_file(&full, &new_content);
            }
            Ok(CallToolResult::success(vec![Content::text(format!("Appended to {}", rel_path))]))
        } else if full.exists() {
            let content = std::fs::read_to_string(&full).unwrap_or_default();
            Ok(CallToolResult::success(vec![Content::text(content)]))
        } else {
            let content = format!("# {}\n\n", date_str);
            std::fs::write(&full, &content)
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;
            if let Ok(mut index) = self.search_index.lock() {
                index.update_file(&full, &content);
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

        let (_, suggestions) = self.auto_link_content(&content, &params.0.path);

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
        let (outgoing, incoming) = self.build_graph();

        // BFS
        let mut visited: HashMap<String, usize> = HashMap::new(); // node -> depth
        let mut queue: std::collections::VecDeque<(String, usize)> = std::collections::VecDeque::new();
        let mut edges: Vec<(String, String)> = Vec::new();

        visited.insert(start.clone(), 0);
        queue.push_back((start.clone(), 0));

        while let Some((node, depth)) = queue.pop_front() {
            if depth >= max_depth { continue; }

            // Collect neighbors (both outgoing links and backlinks)
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

        // Apply tag filter if specified
        let nodes: Vec<_> = if let Some(ref tag_filter) = params.0.tag_filter {
            visited.iter()
                .filter(|(node, _)| {
                    // Find the file and check its tags
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

        // Deduplicate edges
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
        let (outgoing, incoming) = self.build_graph();

        // BFS for shortest path
        let mut visited: HashMap<String, String> = HashMap::new(); // node -> predecessor
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

        // Reconstruct path
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
        let (outgoing, incoming) = self.build_graph();

        // Collect all known nodes
        let mut all_nodes: HashSet<String> = HashSet::new();
        for (k, vs) in &outgoing {
            all_nodes.insert(k.clone());
            for v in vs { all_nodes.insert(v.clone()); }
        }
        for (k, vs) in &incoming {
            all_nodes.insert(k.clone());
            for v in vs { all_nodes.insert(v.clone()); }
        }

        // Find connected components via BFS
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

        // Hub notes (most connections)
        let mut connection_counts: Vec<(String, usize)> = all_nodes.iter()
            .map(|node| {
                let out_count = outgoing.get(node).map_or(0, |v| v.len());
                let in_count = incoming.get(node).map_or(0, |v| v.len());
                (node.clone(), out_count + in_count)
            })
            .collect();
        connection_counts.sort_by(|a, b| b.1.cmp(&a.1));

        // Orphan notes (no links in or out)
        let orphans: Vec<_> = all_nodes.iter()
            .filter(|node| {
                let out = outgoing.get(*node).map_or(0, |v| v.len());
                let inc = incoming.get(*node).map_or(0, |v| v.len());
                out == 0 && inc == 0
            })
            .cloned()
            .collect();

        // Bridge notes: nodes whose removal would increase component count
        // Approximation: nodes that connect to multiple components' members
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

        let (linked_content, links_added) = self.auto_link_content(&content, lib_path);
        let full_path = self.resolve_path(lib_path);
        if let Some(parent) = full_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        match std::fs::write(&full_path, &linked_content) {
            Ok(_) => {
                if let Ok(mut index) = self.search_index.lock() {
                    index.update_file(&full_path, &linked_content);
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
}

// ── Server handler ────────────────────────────────────────────────────

#[tool_handler]
impl ServerHandler for LibraryServer {}

// ── Setup command ─────────────────────────────────────────────────────

fn find_binary_path() -> String {
    std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "librarian-mcp".to_string())
}

fn claude_desktop_config_path() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        dirs::home_dir().map(|h| h.join("Library/Application Support/Claude/claude_desktop_config.json"))
    }
    #[cfg(target_os = "linux")]
    {
        dirs::home_dir().map(|h| h.join(".config/Claude/claude_desktop_config.json"))
    }
    #[cfg(target_os = "windows")]
    {
        std::env::var("APPDATA").ok().map(|a| PathBuf::from(a).join("Claude/claude_desktop_config.json"))
    }
}

fn claude_code_config_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude/settings.json"))
}

fn run_setup(vault_paths: &[PathBuf]) -> Result<(), Box<dyn std::error::Error>> {
    let binary = find_binary_path();
    let vault_args: Vec<String> = vault_paths.iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    let mut configured = Vec::new();

    // Claude Desktop
    if let Some(config_path) = claude_desktop_config_path() {
        if let Some(parent) = config_path.parent() {
            if parent.exists() {
                let mut config: serde_json::Value = if config_path.exists() {
                    let content = std::fs::read_to_string(&config_path)?;
                    serde_json::from_str(&content).unwrap_or(serde_json::json!({}))
                } else {
                    serde_json::json!({})
                };

                // Backup existing config
                if config_path.exists() {
                    let backup = config_path.with_extension("json.bak");
                    std::fs::copy(&config_path, &backup)?;
                }

                let mcp_servers = config
                    .as_object_mut().unwrap()
                    .entry("mcpServers")
                    .or_insert(serde_json::json!({}));

                mcp_servers.as_object_mut().unwrap().insert(
                    "librarian".to_string(),
                    serde_json::json!({
                        "command": binary,
                        "args": vault_args,
                    }),
                );

                std::fs::write(&config_path, serde_json::to_string_pretty(&config)?)?;
                configured.push(format!("Claude Desktop ({})", config_path.display()));
            }
        }
    }

    // Claude Code
    if let Some(config_path) = claude_code_config_path() {
        if let Some(parent) = config_path.parent() {
            if parent.exists() {
                let mut config: serde_json::Value = if config_path.exists() {
                    let content = std::fs::read_to_string(&config_path)?;
                    serde_json::from_str(&content).unwrap_or(serde_json::json!({}))
                } else {
                    serde_json::json!({})
                };

                // Backup existing config
                if config_path.exists() {
                    let backup = config_path.with_extension("json.bak");
                    std::fs::copy(&config_path, &backup)?;
                }

                let mcp_servers = config
                    .as_object_mut().unwrap()
                    .entry("mcpServers")
                    .or_insert(serde_json::json!({}));

                mcp_servers.as_object_mut().unwrap().insert(
                    "librarian".to_string(),
                    serde_json::json!({
                        "command": binary,
                        "args": vault_args,
                    }),
                );

                std::fs::write(&config_path, serde_json::to_string_pretty(&config)?)?;
                configured.push(format!("Claude Code ({})", config_path.display()));
            }
        }
    }

    if configured.is_empty() {
        eprintln!("No Claude installations found. Install Claude Desktop or Claude Code first.");
        eprintln!("You can manually add this to your MCP config:");
        eprintln!();
        eprintln!("  \"librarian\": {{");
        eprintln!("    \"command\": \"{}\",", binary);
        eprintln!("    \"args\": {:?}", vault_args);
        eprintln!("  }}");
    } else {
        println!("Librarian configured for:");
        for target in &configured {
            println!("  ✓ {}", target);
        }
        println!();
        println!("Vault{}: {}", if vault_args.len() > 1 { "s" } else { "" }, vault_args.join(", "));
        println!();
        println!("Restart Claude to connect your vault.");
    }

    Ok(())
}

// ── CLI ───────────────────────────────────────────────────────────────

use clap::Parser;

/// Give Claude a librarian for your markdown vault.
///
/// Librarian is an MCP server that connects Claude to your Obsidian vault
/// or any folder of markdown files. It provides search, auto-linking,
/// backlinks, tags, and more.
#[derive(Parser)]
#[command(name = "librarian-mcp", version, about)]
struct Cli {
    /// Vault paths to serve (can specify multiple)
    #[arg(value_name = "VAULT_PATH")]
    vaults: Vec<PathBuf>,

    /// Auto-configure Claude Desktop and Claude Code to use Librarian
    #[arg(long)]
    setup: bool,
}

// ── Main ──────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let default_ignores = vec![
        ".obsidian/".to_string(),
        ".trash/".to_string(),
        ".git/".to_string(),
        "node_modules/".to_string(),
    ];

    // Resolve vault paths: CLI args > env vars > default
    let library_paths: Vec<PathBuf> = if !cli.vaults.is_empty() {
        cli.vaults.clone()
    } else if let Ok(vaults) = std::env::var("LIBRARIAN_VAULTS") {
        vaults.split(':').map(PathBuf::from).collect()
    } else if let Ok(vault) = std::env::var("LIBRARIAN_VAULT") {
        vec![PathBuf::from(vault)]
    } else if let Ok(vault) = std::env::var("VEROWRITE_VAULT") {
        vec![PathBuf::from(vault)]
    } else {
        let default = dirs::home_dir()
            .map(|h| h.join("vaults/The Labyrinth"))
            .unwrap_or_else(|| PathBuf::from("."));
        vec![default]
    };

    // Handle --setup
    if cli.setup {
        return run_setup(&library_paths).map_err(|e| e.into());
    }

    // Validate paths
    for path in &library_paths {
        if !path.exists() {
            eprintln!("Warning: vault path does not exist: {}", path.display());
        }
    }

    let vault_display: Vec<_> = library_paths.iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    let server = LibraryServer {
        library_paths,
        default_ignores,
        search_index: std::sync::Arc::new(Mutex::new(SearchIndex::default())),
        tool_router: LibraryServer::tool_router(),
    };

    // Build search index
    let index = server.build_search_index();
    *server.search_index.lock().unwrap() = index;

    if vault_display.len() == 1 {
        eprintln!("Librarian MCP starting — vault: {}", vault_display[0]);
    } else {
        eprintln!("Librarian MCP starting — {} vaults: {}", vault_display.len(), vault_display.join(", "));
    }

    let transport = rmcp::transport::stdio();
    let service = server.serve(transport).await?;
    service.waiting().await?;

    Ok(())
}
