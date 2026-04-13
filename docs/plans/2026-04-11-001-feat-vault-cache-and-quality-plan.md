---
title: "feat: Vault Cache, Search Quality, Write Safety, and Daydream"
type: feat
status: completed
date: 2026-04-11
origin: docs/brainstorms/2026-04-11-vault-cache-and-quality-requirements.md
---

# feat: Vault Cache, Search Quality, Write Safety, and Daydream

## Overview

Performance and correctness upgrade for librarian-mcp. Introduces a unified in-memory cache (graph + titles + search index) with mtime-based invalidation, fixes auto-link corruption in code blocks, adds BM25 search ranking, returns backlink suggestions on every write, and adds a `/librarian daydream` skill command for serendipitous vault discovery.

## Problem Statement / Motivation

Every graph tool call re-reads the entire vault from disk. Every write rescans all files for title matching. The search index rebuilds entirely on single-file updates. Auto-linking corrupts code blocks. Search results are unranked. These compound into a tool that feels slow and untrustworthy as vaults grow beyond ~200 files. (see origin: `docs/brainstorms/2026-04-11-vault-cache-and-quality-requirements.md`)

## Proposed Solution

### Phase 1: VaultCache struct (R1, R2, R3)

Create `src/cache.rs` with a `VaultCache` struct that holds:

```
pub struct VaultCache {
    // Search
    pub search_index: SearchIndex,

    // Graph (bidirectional adjacency)
    pub outgoing: HashMap<String, Vec<String>>,
    pub incoming: HashMap<String, Vec<String>>,

    // Titles (for auto-linking)
    pub titles: Vec<(String, String, String)>,  // (match_term, canonical, rel_path)

    // Invalidation
    file_mtimes: HashMap<PathBuf, SystemTime>,
    last_full_build: SystemTime,
}
```

**Integration points:**
- `src/server.rs`: Replace `search_index: Arc<Mutex<SearchIndex>>` with `cache: Arc<Mutex<VaultCache>>` on `LibraryServer`
- `src/server.rs:build_graph()`: Remove. Graph now lives in cache.
- `src/server.rs:build_search_index()`: Replace with `VaultCache::build_full()`
- `src/server.rs:all_note_titles()`: Remove. Titles now live in cache.
- `src/main.rs:77-86`: Build `VaultCache` at startup instead of `SearchIndex`

**Cache methods:**
- `build_full(md_files) -> VaultCache` — one-time startup build
- `check_and_refresh(&mut self, md_files)` — compare mtimes, incrementally update changed files
- `update_file(&mut self, path, content)` — called by `library_write` / `library_daily`

**Mtime checking (R2):**
- Store `HashMap<PathBuf, SystemTime>` mapping each file to its last-known mtime
- On `check_and_refresh()`: scan `all_md_files()`, compare each file's mtime to stored value
- Only re-read and re-index files whose mtime changed
- For each changed file: update search index entry, update graph edges, update title entry

**Incremental search index (R3):**
- Add `file_trigrams: HashMap<usize, HashSet<[u8; 3]>>` to SearchIndex
- On file update: remove old trigrams for that file index, compute new trigrams, insert
- Remove the `*self = Self::build(&self.files)` line in `update_file()`

**Tool changes:**
- All graph tools (`library_traverse`, `library_shortest_path`, `library_graph_analysis`, `library_cluster`, `library_visualize`, `library_report`): read graph from `self.cache.lock().unwrap()` instead of calling `build_graph()`
- Before graph reads, call `cache.check_and_refresh(self.all_md_files())` to catch external edits
- `library_search`: read search_index from cache
- `library_write` / `library_daily` / `library_import`: call `cache.update_file()` after write

**Files modified:**
- `src/cache.rs` — NEW (~150 lines)
- `src/server.rs` — remove `build_graph()`, `build_search_index()`, `all_note_titles()`, change struct field
- `src/search.rs` — add `file_trigrams` tracking, fix `update_file()` to be incremental
- `src/tools.rs` — all tools read from cache, graph tools skip `build_graph()`
- `src/main.rs` — build VaultCache at startup

### Phase 2: Auto-link safety (R4)

Modify `auto_link_content()` in `src/server.rs:127-189` to skip exclusion zones.

**Approach:** Pre-pass to identify byte ranges that must not be modified.

```rust
fn find_exclusion_zones(body: &str) -> Vec<(usize, usize)> {
    let mut zones = Vec::new();
    // Fenced code blocks: ```...```
    // Inline code: `...`
    // URLs: http://... or https://... until whitespace
    // Existing wikilinks: [[...]]
    zones
}
```

**Integration:** In `auto_link_content()`, after computing `fm_end` and `body_part`:
1. Compute exclusion zones on `body_part`
2. When `re.find(body_part)` finds a match, check if the match range overlaps any exclusion zone
3. If overlap, skip this match and continue searching

**Files modified:**
- `src/server.rs` — add `find_exclusion_zones()`, modify `auto_link_content()` (~30 lines net)

### Phase 3: BM25 search ranking (R5)

Add BM25 scoring to `SearchIndex` in `src/search.rs`.

**BM25 formula:**
```
score(q, d) = Σ IDF(qi) * (tf(qi, d) * (k1 + 1)) / (tf(qi, d) + k1 * (1 - b + b * |d|/avgdl))
```
Where: k1=1.2, b=0.75, avgdl=average document length in words

**Implementation:**
- Add `doc_lengths: Vec<usize>` (word count per file) and `avg_doc_length: f64` to SearchIndex
- Add `doc_freq: HashMap<String, usize>` — number of documents containing each lowercased word (computed at build time)
- In `search()`: after trigram candidate filtering, compute BM25 score for each candidate, sort descending
- Return type unchanged but results now ordered by relevance

**IDF computation:** From the full vault (all indexed files). This is computed once at build time and stored.

**Tool change:**
- `library_search` in tools.rs: add `"score": score` to each result JSON object

**Files modified:**
- `src/search.rs` — add BM25 fields to SearchIndex, scoring in `search()` (~50 lines)
- `src/tools.rs` `library_search` — add score to output JSON (~3 lines)

### Phase 4: Write-amplification backlink suggestions (R6)

After `library_write` writes and updates the cache, scan for reverse mentions.

**Logic:** In `library_write` after the file is written:
1. Extract the written file's stem (note title)
2. From the cached search index, find files whose content contains the title as a word boundary match
3. Exclude files that already have a `[[title]]` wikilink
4. Return up to 5 suggestions as `backlink_suggestions` in the response

**Files modified:**
- `src/tools.rs` `library_write` — add backlink suggestion scan after write (~25 lines)
- Depends on title being in cache (Phase 1)

### Phase 5: Daydream skill command (R7)

Add `/librarian daydream` to `skill/SKILL.md`. This is skill-layer only — no Rust changes.

**Command section to add:**

```
/librarian daydream [focus]    Discover non-obvious connections across vault notes
```

**Behavior:**
1. Call `library_list` to get all note paths
2. Call `library_read` on a random sample (50 notes, weighted toward recent by filename/date frontmatter)
3. Generate 50 random pairs from the sample
4. Check `Daydreams/history.json` (via `library_read`) for already-processed pairs, skip them
5. Launch parallel sub-agents (Sonnet model) — each receives 5 note pairs and synthesizes connections:
   - Abstract analogies
   - Similar problems in different domains
   - Potential hybrid ideas
   - Revealing contradictions
6. Launch parallel critic sub-agents (Haiku model) — score each connection on novelty (1-10), coherence (1-10), usefulness (1-10). Accept if average >= 7.0.
7. For each accepted insight: call `library_write` to save to `Daydreams/<YYYYMMDD>-<slug>.md` with frontmatter (title, source_notes, scores, date, tags: [daydream])
8. Update `Daydreams/history.json` with processed pairs via `library_write`
9. Report: N pairs processed, M insights accepted, file paths

**If `[focus]` provided:** Weight note sampling toward notes matching the focus term (use `library_search` to find focus-relevant notes, then pair them with random notes from outside that cluster).

**Files modified:**
- `skill/SKILL.md` — add `daydream` command section (~80 lines)

## Technical Considerations

- **Thread safety:** VaultCache behind `Arc<Mutex>` (same pattern as current SearchIndex). Consider `RwLock` if read contention becomes an issue — but Mutex is simpler and MCP calls are sequential.
- **No new Cargo dependencies.** All algorithms (BM25, mtime, exclusion zones) are standard library operations.
- **Backward compatibility:** Search results gain a `score` field. Write results gain a `backlink_suggestions` field. Both are additive — no existing fields removed.
- **Daydream cost:** ~$0.40-0.50 per run using Sonnet for synthesis and Haiku for critique. Documented in skill.

## Acceptance Criteria

- [ ] **R1**: `VaultCache` struct holds graph, titles, and search index. All tools read from cache.
- [ ] **R2**: External edits (e.g., Obsidian) detected on next tool call via mtime comparison.
- [ ] **R3**: `update_file()` in SearchIndex no longer calls `Self::build()`. Single-file update is O(file_size).
- [ ] **R4**: `auto_link_content()` does not modify text inside fenced code blocks, inline code, or URLs.
- [ ] **R5**: `library_search` returns results sorted by BM25 score. Score included in JSON.
- [ ] **R6**: `library_write` response includes `backlink_suggestions` array with paths and snippets.
- [ ] **R7**: `/librarian daydream` produces insight files in `Daydreams/` with auto-wikilinks.
- [ ] `cargo build` passes with no new warnings.
- [ ] Existing 17 MCP tools continue to work (no regressions).

## Implementation Phases

### Phase 1: VaultCache (R1 + R2 + R3)
**Files:** `src/cache.rs` (new), `src/search.rs`, `src/server.rs`, `src/tools.rs`, `src/main.rs`
**Estimated effort:** ~150 new lines, ~80 lines modified
**Success:** Graph tools don't re-read disk on unchanged vaults. `cargo build` passes.

### Phase 2: Auto-link safety (R4)
**Files:** `src/server.rs`
**Estimated effort:** ~30 new lines
**Success:** Code blocks and URLs survive auto-linking unmodified.

### Phase 3: BM25 ranking (R5)
**Files:** `src/search.rs`, `src/tools.rs`
**Estimated effort:** ~50 new lines
**Success:** Search results sorted by relevance. Score in JSON output.

### Phase 4: Write-amplification (R6)
**Files:** `src/tools.rs`
**Estimated effort:** ~25 new lines
**Success:** Write response includes backlink suggestions.

### Phase 5: Daydream skill (R7)
**Files:** `skill/SKILL.md`
**Estimated effort:** ~80 new lines (skill definition only)
**Success:** `/librarian daydream` orchestrates insight discovery.

## Dependencies & Risks

- **Phase 1 is the foundation** — Phases 3-4 depend on cache for performance. Phase 2 is independent.
- **Risk: mtime resolution.** Some filesystems have 1-second mtime granularity. Two edits within 1 second could be missed. Acceptable for the MCP request-response model.
- **Risk: VaultCache Mutex contention.** Sequential MCP calls mean this is unlikely. If it becomes an issue, upgrade to `RwLock`.

## Sources & References

### Origin

- **Origin document:** [docs/brainstorms/2026-04-11-vault-cache-and-quality-requirements.md](docs/brainstorms/2026-04-11-vault-cache-and-quality-requirements.md) — Key decisions: mtime-based invalidation, BM25 over TF-IDF, backlink suggestions in write response, daydream as skill-only feature.

### Internal References

- Ideation: `docs/ideation/2026-04-11-open-ideation.md`
- VaultCache pattern: `src/search.rs:4-10` (existing SearchIndex)
- Auto-link implementation: `src/server.rs:127-189`
- Graph build: `src/server.rs:206-226`
- Search flow: `src/tools.rs:125-157`
- Write flow: `src/tools.rs:174-201`
- Daydream inspiration: glebis/claude-skills daydream skill, Gwern's LLM Daydreaming essay
