use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::search::SearchIndex;
use crate::server::LibraryServer;

impl Default for VaultCache {
    fn default() -> Self {
        VaultCache {
            search_index: SearchIndex::default(),
            outgoing: HashMap::new(),
            incoming: HashMap::new(),
            titles: Vec::new(),
            file_mtimes: HashMap::new(),
        }
    }
}

pub struct VaultCache {
    pub search_index: SearchIndex,
    pub outgoing: HashMap<String, Vec<String>>,
    pub incoming: HashMap<String, Vec<String>>,
    pub titles: Vec<(String, String, String)>, // (match_term, canonical, rel_path)
    file_mtimes: HashMap<PathBuf, SystemTime>,
}

impl VaultCache {
    /// Build the full cache from scratch by reading all md files once.
    pub fn build_full(server: &LibraryServer) -> VaultCache {
        let mut search_files: Vec<(PathBuf, String)> = Vec::new();
        let mut outgoing: HashMap<String, Vec<String>> = HashMap::new();
        let mut incoming: HashMap<String, Vec<String>> = HashMap::new();
        let mut titles: Vec<(String, String, String)> = Vec::new();
        let mut file_mtimes: HashMap<PathBuf, SystemTime> = HashMap::new();

        for path in server.all_md_files() {
            let stem = match path.file_stem() {
                Some(s) => s.to_string_lossy().to_string(),
                None => continue,
            };
            let rel = server.relative_path(&path);

            let content = match std::fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            // Store mtime
            if let Ok(meta) = std::fs::metadata(&path) {
                if let Ok(mtime) = meta.modified() {
                    file_mtimes.insert(path.clone(), mtime);
                }
            }

            // Search index data
            search_files.push((path.clone(), content.clone()));

            // Graph: outgoing/incoming
            outgoing.entry(stem.clone()).or_default();
            for link in LibraryServer::extract_wikilinks(&content) {
                outgoing.entry(stem.clone()).or_default().push(link.clone());
                incoming.entry(link).or_default().push(stem.clone());
            }

            // Titles: stem + aliases
            titles.push((stem.clone(), stem.clone(), rel.clone()));
            for alias in LibraryServer::extract_aliases(&content) {
                titles.push((alias, stem.clone(), rel.clone()));
            }
        }

        let search_index = SearchIndex::build(&search_files);
        eprintln!("Librarian: indexed {} files for search", search_files.len());

        VaultCache {
            search_index,
            outgoing,
            incoming,
            titles,
            file_mtimes,
        }
    }

    /// Check file mtimes and refresh changed/deleted/new files.
    pub fn check_and_refresh(&mut self, server: &LibraryServer) {
        let current_files = server.all_md_files();
        let current_set: std::collections::HashSet<PathBuf> =
            current_files.iter().cloned().collect();

        // Find deleted files (in cache but not on disk)
        let cached_paths: Vec<PathBuf> = self.file_mtimes.keys().cloned().collect();
        for path in cached_paths {
            if !current_set.contains(&path) {
                self.remove_file_entries(&path, server);
                self.file_mtimes.remove(&path);
            }
        }

        // Find new or changed files
        for path in &current_files {
            let current_mtime = std::fs::metadata(path)
                .ok()
                .and_then(|m| m.modified().ok());

            let needs_update = match (self.file_mtimes.get(path), current_mtime) {
                (Some(cached), Some(current)) => *cached != current,
                (None, _) => true, // new file
                _ => false,
            };

            if needs_update {
                if let Ok(content) = std::fs::read_to_string(path) {
                    self.remove_file_entries(path, server);
                    self.add_file_entries(path, &content, server);
                    if let Some(mtime) = current_mtime {
                        self.file_mtimes.insert(path.clone(), mtime);
                    }
                }
            }
        }
    }

    /// Update a single file's cache entries (called after library_write).
    pub fn update_single_file(&mut self, path: &Path, content: &str, server: &LibraryServer) {
        self.remove_file_entries(path, server);
        self.add_file_entries(path, content, server);

        // Update mtime
        if let Ok(meta) = std::fs::metadata(path) {
            if let Ok(mtime) = meta.modified() {
                self.file_mtimes.insert(path.to_path_buf(), mtime);
            }
        }
    }

    /// Remove all cache entries associated with a file path.
    fn remove_file_entries(&mut self, path: &Path, server: &LibraryServer) {
        let stem = match path.file_stem() {
            Some(s) => s.to_string_lossy().to_string(),
            None => return,
        };
        let rel = server.relative_path(path);

        // Remove from graph
        if let Some(targets) = self.outgoing.remove(&stem) {
            for target in &targets {
                if let Some(sources) = self.incoming.get_mut(target) {
                    sources.retain(|s| s != &stem);
                    if sources.is_empty() {
                        self.incoming.remove(target);
                    }
                }
            }
        }
        // Also remove any incoming edges pointing to this stem from other nodes
        // (these are in outgoing of other nodes, so we just clean the incoming entry)
        self.incoming.remove(&stem);

        // Remove from titles
        self.titles.retain(|(_, _, r)| r != &rel);

        // Remove from search index
        self.search_index.remove_file(path);
    }

    /// Add cache entries for a file from its content.
    fn add_file_entries(&mut self, path: &Path, content: &str, server: &LibraryServer) {
        let stem = match path.file_stem() {
            Some(s) => s.to_string_lossy().to_string(),
            None => return,
        };
        let rel = server.relative_path(path);

        // Add to graph
        outgoing_entry_add(&mut self.outgoing, &mut self.incoming, &stem, content);

        // Add to titles
        self.titles.push((stem.clone(), stem.clone(), rel.clone()));
        for alias in LibraryServer::extract_aliases(content) {
            self.titles.push((alias, stem.clone(), rel.clone()));
        }

        // Add to search index
        self.search_index.add_file(path, content);
    }
}

/// Helper: add outgoing/incoming edges for a file stem from content.
fn outgoing_entry_add(
    outgoing: &mut HashMap<String, Vec<String>>,
    incoming: &mut HashMap<String, Vec<String>>,
    stem: &str,
    content: &str,
) {
    outgoing.entry(stem.to_string()).or_default();
    for link in LibraryServer::extract_wikilinks(content) {
        outgoing.entry(stem.to_string()).or_default().push(link.clone());
        incoming.entry(link).or_default().push(stem.to_string());
    }
}
