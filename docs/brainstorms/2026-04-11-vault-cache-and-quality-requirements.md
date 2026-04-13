---
date: 2026-04-11
topic: vault-cache-and-quality
---

# Vault Cache, Search Quality, and Write Safety

## Problem Frame

As vaults grow beyond ~200 files, librarian-mcp's performance degrades because every graph tool rebuilds the entire wikilink graph from disk, every write rescans all files for auto-linking titles, and the search index rebuilds entirely on each file update. Additionally, the auto-linker silently corrupts content by inserting wikilinks inside code blocks and URLs, and search returns results in arbitrary order with no relevance ranking. These four problems are interconnected — a unified cache makes the performance fixes viable, which in turn makes new features (backlink suggestions, ranked search) fast enough to run on every operation.

## Requirements

- R1. **Unified vault cache.** Build the wikilink graph, note title/alias list, and search index once at startup. Store in a shared `VaultCache` struct behind `Arc<Mutex>`. All tools read from cache instead of rescanning the vault.
- R2. **Mtime-based cache invalidation.** Before read operations that depend on the cache, check whether any vault file has a modification time newer than the last cache build timestamp. If so, rebuild only the changed files' entries (incremental update for search index, graph adjacency, and title list). Writes through `library_write` and `library_daily` update the cache immediately.
- R3. **Incremental search index updates.** When a single file changes, remove its old trigrams and insert new ones instead of rebuilding the entire index. The full rebuild only happens at startup.
- R4. **Auto-link safety zones.** `auto_link_content()` must skip fenced code blocks (``` ... ```), inline code (`` ` ... ` ``), URLs (http/https), and existing wikilinks when scanning for title matches. Content inside these zones must never be modified.
- R5. **BM25 search result ranking.** `library_search` must return results sorted by BM25 relevance score (term frequency normalized by document length, weighted by inverse document frequency across the vault). The score must be included in the result JSON.
- R6. **Write-amplification backlink suggestions.** After `library_write` writes a file, scan the cached title list to find existing notes that mention the just-written note's title but don't link to it. Return these as a `backlink_suggestions` array in the write response (note path + context snippet). Do not auto-apply — suggestions only.

- R7. **Daydream integration via `/librarian daydream`.** Add a skill command that orchestrates vault discovery inspired by Gwern's daydreaming loop and glebis's Daydream skill. The command: (a) samples 50 random note pairs from the vault using `library_search` and `library_list`, weighted toward recent notes, (b) launches parallel sub-agents to synthesize non-obvious connections between each pair, (c) filters results through a critic pass (accept if novelty + coherence + usefulness avg >= 7/10), (d) writes accepted insights to `Daydreams/<YYYYMMDD>-<slug>.md` via `library_write` (gaining auto-wikilinks), (e) tracks processed pairs in `Daydreams/history.json` to avoid duplicates across runs. This is a skill-layer feature — no new Rust MCP tools needed. Librarian's existing tools provide the vault access; the skill orchestrates the multi-agent synthesis.

## Success Criteria

- Graph tool calls (`library_traverse`, `library_cluster`, `library_report`, etc.) do not re-read all files from disk when the vault hasn't changed since the last call
- A `library_write` to a 1,000-file vault completes in under 500ms (currently O(N) full-vault scans)
- Auto-linking never inserts wikilinks inside code blocks, inline code, or URLs
- `library_search` results for a multi-word query return the most topically relevant note first, not an arbitrary match
- `library_write` response includes backlink suggestions when other notes mention the written note's title
- `/librarian daydream` produces 5-15 accepted insight notes per run, auto-wikilinked into the knowledge graph
- Daydream insights appear in graph analysis (god nodes, communities, surprising connections) on subsequent runs

## Scope Boundaries

- No file-watching daemon — mtime checks on tool invocation are sufficient
- No semantic/embedding search — BM25 over trigram candidates only
- No auto-application of backlink suggestions — the response suggests, the LLM or user decides
- No persistent on-disk cache — cache lives in memory, rebuilt at startup
- No changes to the MCP tool API surface beyond adding `score` to search results and `backlink_suggestions` to write results
- Daydream is a skill-layer feature only — no new Rust MCP tools, no embedding models, no external APIs beyond Claude's own sub-agents

## Key Decisions

- **Mtime-based invalidation over file watching:** Simpler, no new dependencies, correct for the MCP request-response model. External edits (Obsidian) are detected on next tool call.
- **Incremental index update over full rebuild:** `update_file()` currently calls `Self::build(&self.files)`. The fix is surgical — remove old trigrams, insert new ones.
- **Backlink suggestions in write response, not a separate tool:** Reduces tool calls and makes the compounding effect automatic — every write returns graph-strengthening suggestions.
- **BM25 over simpler TF-IDF:** BM25's document-length normalization prevents long notes from dominating results. Standard algorithm, well-understood parameters (k1=1.2, b=0.75).

## Outstanding Questions

### Deferred to Planning
- [Affects R2][Technical] What granularity for mtime checking — per-file or vault-level directory mtime?
- [Affects R3][Technical] Should incremental trigram update track per-file trigram sets, or diff old vs new content?
- [Affects R4][Technical] Best approach for code block detection — pre-pass to identify exclusion byte ranges, or regex negative lookahead?
- [Affects R5][Needs research] Should BM25 IDF be computed from the full vault or only the trigram candidate set?

## Next Steps

→ `/ce:plan` for structured implementation planning
