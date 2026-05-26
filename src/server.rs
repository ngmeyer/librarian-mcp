use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rmcp::handler::server::router::tool::ToolRouter;

use crate::cache::VaultCache;

/// Librarian MCP Server — give Claude a librarian for your markdown vault
#[derive(Clone)]
pub struct LibraryServer {
    /// One or more vault roots
    pub library_paths: Vec<PathBuf>,
    /// Default exclusion patterns when no .librarianignore exists
    pub default_ignores: Vec<String>,
    /// Lowercased note stems/aliases the auto-linker must never link
    /// (generic structural filenames like INDEX/README/SKILL that match
    /// common words and pollute the graph with cross-domain false edges).
    pub link_stoplist: Vec<String>,
    /// Unified vault cache (search index, graph, titles)
    pub cache: std::sync::Arc<Mutex<VaultCache>>,
    pub tool_router: ToolRouter<Self>,
}

/// Generic stems excluded from auto-linking by default. These are structural
/// or template filenames whose stems collide with everyday prose words, so
/// matching them creates noise rather than meaningful links. Extend per-vault
/// with a `.librarianstoplist` file (one term per line) in the vault root.
pub const DEFAULT_LINK_STOPLIST: &[&str] = &[
    "claude", "skill", "index", "readme", "memory", "language",
    "changelog", "filename", "critic-prompt", "scoring-rubric",
];

impl LibraryServer {
    /// Resolve a relative path against vault roots. Returns the first match,
    /// or falls back to the first vault root for new files.
    /// Build the auto-link stoplist: the hardcoded defaults plus any terms
    /// listed in a `.librarianstoplist` file at any vault root. All lowercased.
    pub fn build_link_stoplist(library_paths: &[PathBuf]) -> Vec<String> {
        let mut stop: HashSet<String> =
            DEFAULT_LINK_STOPLIST.iter().map(|s| s.to_string()).collect();
        for root in library_paths {
            if let Ok(contents) = std::fs::read_to_string(root.join(".librarianstoplist")) {
                for line in contents.lines() {
                    let term = line.trim();
                    if !term.is_empty() && !term.starts_with('#') {
                        stop.insert(term.to_lowercase());
                    }
                }
            }
        }
        stop.into_iter().collect()
    }

    pub fn resolve_path(&self, rel: &str) -> PathBuf {
        for root in &self.library_paths {
            let candidate = root.join(rel);
            if candidate.exists() {
                return candidate;
            }
        }
        self.library_paths[0].join(rel)
    }

    /// Collect all markdown files across all vault roots, respecting .librarianignore.
    pub fn all_md_files(&self) -> Vec<PathBuf> {
        let mut files = Vec::new();
        for root in &self.library_paths {
            let mut builder = ignore::WalkBuilder::new(root);
            builder.hidden(true);

            let ignore_file = root.join(".librarianignore");
            if ignore_file.exists() {
                builder.add_ignore(ignore_file);
            } else {
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
    pub fn relative_path(&self, abs: &Path) -> String {
        for root in &self.library_paths {
            if let Ok(rel) = abs.strip_prefix(root) {
                return rel.to_string_lossy().to_string();
            }
        }
        abs.to_string_lossy().to_string()
    }

    pub fn extract_frontmatter(content: &str) -> Option<String> {
        if content.starts_with("---\n") {
            if let Some(end) = content[4..].find("\n---") {
                return Some(content[4..4 + end].to_string());
            }
        }
        None
    }

    /// Extract aliases from YAML frontmatter.
    pub fn extract_aliases(content: &str) -> Vec<String> {
        let fm = match Self::extract_frontmatter(content) {
            Some(fm) => fm,
            None => return Vec::new(),
        };
        for line in fm.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("aliases:") {
                let rest = rest.trim();
                if rest.starts_with('[') {
                    return rest.trim_start_matches('[').trim_end_matches(']')
                        .split(',')
                        .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                }
                if !rest.is_empty() {
                    return vec![rest.trim_matches('"').trim_matches('\'').to_string()];
                }
            }
        }
        Vec::new()
    }

    /// Return byte ranges within `text` that should be excluded from auto-linking.
    /// Covers fenced code blocks, inline code, URLs, and existing wikilinks.
    fn find_exclusion_zones(text: &str) -> Vec<(usize, usize)> {
        let mut zones = Vec::new();

        // Fenced code blocks: ```...```
        let fenced = regex::Regex::new(r"(?ms)^```[^\n]*\n.*?^```").unwrap();
        for m in fenced.find_iter(text) {
            zones.push((m.start(), m.end()));
        }

        // Inline code: `...`
        let inline = regex::Regex::new(r"`[^`]+`").unwrap();
        for m in inline.find_iter(text) {
            zones.push((m.start(), m.end()));
        }

        // URLs: http:// or https:// until whitespace
        let urls = regex::Regex::new(r"https?://\S+").unwrap();
        for m in urls.find_iter(text) {
            zones.push((m.start(), m.end()));
        }

        // Existing wikilinks: [[...]]
        let wikilinks = regex::Regex::new(r"\[\[[^\]]+\]\]").unwrap();
        for m in wikilinks.find_iter(text) {
            zones.push((m.start(), m.end()));
        }

        zones
    }

    /// Auto-link: scan content for mentions of existing note titles and wrap them in [[wikilinks]].
    pub fn auto_link_content(&self, content: &str, exclude_path: &str, titles: &[(String, String, String)]) -> (String, Vec<String>) {
        let existing_links = Self::extract_wikilinks(content);
        let existing_set: HashSet<&str> = existing_links.iter().map(|s| s.as_str()).collect();

        let mut result = content.to_string();
        let mut links_added = Vec::new();

        let mut candidates: Vec<_> = titles.iter()
            .filter(|(match_term, canonical, rel)| {
                match_term.len() >= 3
                    && rel != exclude_path
                    && !self.link_stoplist.contains(&match_term.to_lowercase())
                    && !existing_set.contains(canonical.as_str())
                    && !existing_set.contains(match_term.as_str())
            })
            .collect();
        candidates.sort_by(|a, b| b.0.len().cmp(&a.0.len()));

        let mut linked_stems: HashSet<String> = HashSet::new();

        for (match_term, canonical, _rel) in &candidates {
            if linked_stems.contains(canonical.as_str()) {
                continue;
            }

            let pattern = format!(r"(?i)\b{}\b", regex::escape(match_term));
            if let Ok(re) = regex::Regex::new(&pattern) {
                let fm_end = if result.starts_with("---\n") {
                    result[4..].find("\n---\n").map(|i| 4 + i + 5).unwrap_or(0)
                } else {
                    0
                };

                let body_part = &result[fm_end..];

                if body_part.contains(&format!("[[{}]]", canonical))
                    || body_part.contains(&format!("[[{}|", canonical))
                {
                    continue;
                }

                let exclusion_zones = Self::find_exclusion_zones(body_part);

                // Find the first match that doesn't overlap an exclusion zone
                let mut search_start = 0;
                let found = loop {
                    if search_start >= body_part.len() {
                        break None;
                    }
                    match re.find(&body_part[search_start..]) {
                        Some(m) => {
                            let abs_start = search_start + m.start();
                            let abs_end = search_start + m.end();
                            let overlaps = exclusion_zones.iter().any(|(zs, ze)| {
                                abs_start < *ze && abs_end > *zs
                            });
                            if overlaps {
                                // Advance past this match and keep searching
                                search_start = abs_end;
                            } else {
                                break Some((abs_start, abs_end));
                            }
                        }
                        None => break None,
                    }
                };

                if let Some((m_start, m_end)) = found {
                    let replacement = if match_term.to_lowercase() == canonical.to_lowercase() {
                        format!("[[{}]]", canonical)
                    } else {
                        let matched_text = &body_part[m_start..m_end];
                        format!("[[{}|{}]]", canonical, matched_text)
                    };
                    let new_body = format!(
                        "{}{}{}",
                        &body_part[..m_start],
                        replacement,
                        &body_part[m_end..]
                    );
                    result = format!("{}{}", &result[..fm_end], new_body);
                    links_added.push(canonical.to_string());
                    linked_stems.insert(canonical.to_string());
                }
            }
        }

        (result, links_added)
    }

    pub fn extract_wikilinks(content: &str) -> Vec<String> {
        let re = regex::Regex::new(r"\[\[([^\]|]+)(?:\|[^\]]+)?\]\]").unwrap();
        re.captures_iter(content)
            .map(|c| c[1].to_string())
            .collect()
    }

    pub fn extract_tags(content: &str) -> Vec<String> {
        let re = regex::Regex::new(r"(?:^|\s)#([\w/-]+)").unwrap();
        re.captures_iter(content)
            .map(|c| c[1].to_string())
            .collect()
    }

}
