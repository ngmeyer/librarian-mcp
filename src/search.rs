use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Default)]
pub struct SearchIndex {
    /// All indexed files: (absolute path, content)
    pub files: Vec<(PathBuf, String)>,
    /// Trigram -> indices into `files`
    trigrams: HashMap<[u8; 3], Vec<usize>>,
    /// BM25: word count per file
    pub doc_lengths: Vec<usize>,
    /// BM25: average document length across vault
    pub avg_doc_length: f64,
    /// BM25: word -> number of docs containing it
    pub doc_freq: HashMap<String, usize>,
    /// BM25: total number of documents
    pub total_docs: usize,
}

impl SearchIndex {
    pub fn build(paths: &[(PathBuf, String)]) -> Self {
        let mut idx = SearchIndex {
            files: paths.to_vec(),
            trigrams: HashMap::new(),
            doc_lengths: Vec::with_capacity(paths.len()),
            avg_doc_length: 0.0,
            doc_freq: HashMap::new(),
            total_docs: paths.len(),
        };

        // Build trigrams and BM25 stats in one pass
        for (i, (_path, content)) in paths.iter().enumerate() {
            let lower = content.to_lowercase();

            // Trigrams
            let bytes = lower.as_bytes();
            for window in bytes.windows(3) {
                let tri = [window[0], window[1], window[2]];
                idx.trigrams.entry(tri).or_default().push(i);
            }

            // BM25: doc length
            let word_count = content.split_whitespace().count();
            idx.doc_lengths.push(word_count);

            // BM25: doc_freq (unique lowercased words per doc)
            let unique_words: HashSet<&str> = lower.split_whitespace().collect();
            for word in unique_words {
                *idx.doc_freq.entry(word.to_string()).or_insert(0) += 1;
            }
        }

        for indices in idx.trigrams.values_mut() {
            indices.sort_unstable();
            indices.dedup();
        }

        // BM25: average doc length
        if !idx.doc_lengths.is_empty() {
            let total: usize = idx.doc_lengths.iter().sum();
            idx.avg_doc_length = total as f64 / idx.doc_lengths.len() as f64;
        }

        idx
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<(PathBuf, String, f64)> {
        let query_lower = query.to_lowercase();

        // Candidate gather: term-based OR retrieval. A document is a candidate
        // if it contains ANY query term (not the whole phrase verbatim), so
        // conceptual multi-word queries retrieve instead of returning nothing.
        // BM25 below ranks candidates by how many/how important the terms are.
        let terms: Vec<String> = query_lower
            .split_whitespace()
            .map(|t| t.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
            .filter(|t| !t.is_empty())
            .collect();

        // Per-term candidate docs: trigram-prefilter (terms >= 3 chars) then
        // verify the term is actually a substring; linear scan for short terms.
        let term_candidates = |term: &str| -> HashSet<usize> {
            if term.len() < 3 {
                return self
                    .files
                    .iter()
                    .enumerate()
                    .filter(|(_, (_, c))| c.to_lowercase().contains(term))
                    .map(|(i, _)| i)
                    .collect();
            }
            let tb = term.as_bytes();
            let mut acc: Option<HashSet<usize>> = None;
            for w in tb.windows(3) {
                let tri = [w[0], w[1], w[2]];
                match self.trigrams.get(&tri) {
                    Some(idx) => {
                        let s: HashSet<usize> = idx.iter().copied().collect();
                        acc = Some(match acc {
                            Some(p) => p.intersection(&s).copied().collect(),
                            None => s,
                        });
                    }
                    None => return HashSet::new(),
                }
            }
            acc.unwrap_or_default()
                .into_iter()
                .filter(|&i| self.files[i].1.to_lowercase().contains(term))
                .collect()
        };

        let mut candidate_set: HashSet<usize> = HashSet::new();
        if terms.is_empty() {
            // Fall back to whole-string substring for punctuation-only queries.
            for (i, (_, content)) in self.files.iter().enumerate() {
                if content.to_lowercase().contains(&query_lower) {
                    candidate_set.insert(i);
                }
            }
        } else {
            for term in &terms {
                candidate_set.extend(term_candidates(term));
            }
        }
        let matched_indices: Vec<usize> = candidate_set.into_iter().collect();

        // BM25 scoring
        let k1: f64 = 1.2;
        let b: f64 = 0.75;
        let n = self.total_docs as f64;
        let avgdl = self.avg_doc_length;
        let query_terms: Vec<&str> = query_lower.split_whitespace().collect();

        let mut scored: Vec<(PathBuf, String, f64)> = matched_indices.into_iter()
            .map(|i| {
                let (path, content) = &self.files[i];
                let doc_len = self.doc_lengths.get(i).copied().unwrap_or(0) as f64;
                let content_lower = content.to_lowercase();
                let doc_words: Vec<&str> = content_lower.split_whitespace().collect();

                let mut score = 0.0f64;
                for term in &query_terms {
                    let tf = doc_words.iter().filter(|w| *w == term).count() as f64;
                    let df = self.doc_freq.get(*term).copied().unwrap_or(0) as f64;
                    let idf = ((n - df + 0.5) / (df + 0.5) + 1.0).ln();
                    score += idf * (tf * (k1 + 1.0)) / (tf + k1 * (1.0 - b + b * doc_len / avgdl));
                }

                (path.clone(), content.clone(), score)
            })
            .collect();

        scored.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(limit);
        scored
    }

    pub fn update_file(&mut self, path: &Path, new_content: &str) {
        if let Some(idx) = self.files.iter().position(|(p, _)| p == path) {
            self.remove_trigrams_for(idx);
            self.remove_bm25_for(idx);
            self.files[idx].1 = new_content.to_string();
            self.add_trigrams_for(idx);
            self.add_bm25_for(idx);
        } else {
            self.files.push((path.to_path_buf(), new_content.to_string()));
            let idx = self.files.len() - 1;
            self.add_trigrams_for(idx);
            self.doc_lengths.push(0);
            self.total_docs += 1;
            self.add_bm25_for(idx);
        }
    }

    /// Remove a file from the index entirely.
    pub fn remove_file(&mut self, path: &Path) {
        if let Some(idx) = self.files.iter().position(|(p, _)| p == path) {
            // Remove trigrams for this index
            self.remove_trigrams_for(idx);
            // Remove BM25 stats for this file
            self.remove_bm25_for(idx);
            // Remove the doc_lengths entry
            self.doc_lengths.remove(idx);
            self.total_docs = self.total_docs.saturating_sub(1);
            // Remove the file entry
            self.files.remove(idx);
            // Shift all trigram indices > idx down by 1
            for indices in self.trigrams.values_mut() {
                indices.retain(|&i| i != idx);
                for i in indices.iter_mut() {
                    if *i > idx {
                        *i -= 1;
                    }
                }
            }
            // Clean up empty trigram entries
            self.trigrams.retain(|_, v| !v.is_empty());
            // Recompute avg_doc_length
            self.recompute_avg_doc_length();
        }
    }

    /// Add a new file to the index.
    pub fn add_file(&mut self, path: &Path, content: &str) {
        self.files.push((path.to_path_buf(), content.to_string()));
        let idx = self.files.len() - 1;
        self.doc_lengths.push(0);
        self.total_docs += 1;
        self.add_trigrams_for(idx);
        self.add_bm25_for(idx);
    }

    /// Remove all trigram entries for a given file index.
    fn remove_trigrams_for(&mut self, idx: usize) {
        for indices in self.trigrams.values_mut() {
            indices.retain(|&i| i != idx);
        }
        self.trigrams.retain(|_, v| !v.is_empty());
    }

    /// Add trigram entries for a given file index.
    fn add_trigrams_for(&mut self, idx: usize) {
        let lower = self.files[idx].1.to_lowercase();
        let bytes = lower.as_bytes();
        let mut seen = std::collections::HashSet::new();
        for window in bytes.windows(3) {
            let tri = [window[0], window[1], window[2]];
            if seen.insert(tri) {
                self.trigrams.entry(tri).or_default().push(idx);
            }
        }
    }

    /// Remove BM25 doc_freq contributions for a file (before content change or removal).
    fn remove_bm25_for(&mut self, idx: usize) {
        let lower = self.files[idx].1.to_lowercase();
        let unique_words: HashSet<&str> = lower.split_whitespace().collect();
        for word in unique_words {
            if let Some(count) = self.doc_freq.get_mut(word) {
                *count = count.saturating_sub(1);
                if *count == 0 {
                    self.doc_freq.remove(word);
                }
            }
        }
    }

    /// Add BM25 doc_freq contributions and update doc_length for a file (after content change or addition).
    fn add_bm25_for(&mut self, idx: usize) {
        let content = &self.files[idx].1;
        let word_count = content.split_whitespace().count();
        self.doc_lengths[idx] = word_count;

        let lower = content.to_lowercase();
        let unique_words: HashSet<&str> = lower.split_whitespace().collect();
        for word in unique_words {
            *self.doc_freq.entry(word.to_string()).or_insert(0) += 1;
        }

        self.recompute_avg_doc_length();
    }

    /// Recompute avg_doc_length from current doc_lengths.
    fn recompute_avg_doc_length(&mut self) {
        if self.doc_lengths.is_empty() {
            self.avg_doc_length = 0.0;
        } else {
            let total: usize = self.doc_lengths.iter().sum();
            self.avg_doc_length = total as f64 / self.doc_lengths.len() as f64;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn idx() -> SearchIndex {
        SearchIndex::build(&[
            (PathBuf::from("a.md"), "VWAP mean reversion strategy for crypto".into()),
            (PathBuf::from("b.md"), "regime gate using a Markov transition matrix".into()),
            (PathBuf::from("c.md"), "gospel study notes on covenant and mercy".into()),
        ])
    }

    // Term-based OR retrieval: a multi-word query whose words are scattered
    // across docs (and never appear as one contiguous phrase) must still match.
    #[test]
    fn search_is_term_based_not_substring() {
        let results = idx().search("VWAP regime reversion", 10);
        let paths: Vec<_> = results.iter().map(|(p, _, _)| p.to_string_lossy().to_string()).collect();
        assert!(paths.contains(&"a.md".to_string()), "doc with VWAP/reversion must match");
        assert!(paths.contains(&"b.md".to_string()), "doc with regime must match (OR semantics)");
        assert!(!paths.contains(&"c.md".to_string()), "unrelated doc must not match");
    }
}
