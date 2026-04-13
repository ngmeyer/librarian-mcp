---
date: 2026-04-11
topic: open-ideation
focus: open-ended
---

# Ideation: Librarian-MCP Improvements

## Codebase Context

- Rust MCP server, ~1500 lines across 8 modules (main, server, tools, graph, search, viz, report, setup)
- 17 MCP tools: search, CRUD, graph analysis (traverse, shortest path, cluster, god nodes, report, visualize), import, daily notes, stats, suggest links
- Skill file bundles `/librarian` with 12 commands: ingest, import, from (gmail/web/twitter/calendar/claude), search, connect, daily, graph, analyze, status
- In-memory trigram search index + bidirectional wikilink graph, both rebuilt frequently
- Just added: community detection (Louvain), betweenness centrality, PageRank, HTML viz, report generation
- No tests, no CI/CD, no persistent cache
- Distributed via Homebrew, single developer
- Vault tested: 195 files / 450K words

## Ranked Ideas

### 1. Unified Vault Cache (Graph + Titles + Incremental Search Index)
**Description:** Build graph, title list, and search index once at startup in a single `VaultCache` struct behind `Arc<Mutex>`. On `library_write`/`library_daily`, update only the changed file's entries. Every tool reads from cache instead of re-scanning the vault.
**Rationale:** Every graph tool calls `build_graph()` reading ALL files from disk. Every write calls `all_note_titles()` reading ALL files. `update_file()` rebuilds the entire trigram index. A single `/librarian analyze` reads the vault 3+ times. One struct, one invalidation path, every tool gets faster.
**Downsides:** Must handle edge cases (file deletes, renames, external edits). Adds ~150 lines of cache management.
**Confidence:** 90%
**Complexity:** Medium
**Status:** Explored

### 2. Auto-Link Code Block Safety
**Description:** Make `auto_link_content()` skip fenced code blocks, inline code, URLs, and heading text when inserting `[[wikilinks]]`. Currently the regex replaces blindly across all body text.
**Rationale:** Real bug. A note titled "Error" or "Test" aggressively links inside code examples. A 3-letter title inside a URL becomes a broken wikilink. Silent data corruption in the most-used tool path.
**Downsides:** Regex-based markdown parsing is imperfect. ~30 lines of exclusion zone detection.
**Confidence:** 95%
**Complexity:** Low
**Status:** Explored

### 3. BM25 Search Result Ranking
**Description:** Replace arbitrary-order search results with BM25 scoring (TF * IDF, normalized by doc length). Return results sorted by relevance.
**Rationale:** Search returns results in HashSet iteration order. A 50-word mention ranks the same as a 2,000-word deep dive. Bad ranking means Claude reads irrelevant files first, wasting context window. ~50 lines on top of existing trigram candidate set.
**Downsides:** BM25 is a heuristic — doesn't understand semantics. But massive improvement over random order.
**Confidence:** 85%
**Complexity:** Low
**Status:** Unexplored

### 4. Write-Amplification Backlink Suggestions
**Description:** After every `library_write`, scan existing notes for unlinked mentions of the just-written note's title. Return backlink suggestions in the write response (not auto-applied).
**Rationale:** Auto-linking only works forward (new note links to existing notes). Never creates inbound links. Graph grows asymmetrically. Write-amplification means every note strengthens the neighborhood. Infrastructure exists — `auto_link_content` run in reverse. ~20 lines.
**Downsides:** Adds latency to writes (must scan titles against all content). Mitigated by title cache from idea #1.
**Confidence:** 80%
**Complexity:** Low
**Status:** Unexplored

### 5. Vault-as-Context-Window (`library_context`)
**Description:** New tool `library_context(topic)` assembles a curated briefing: search + read top 3-5 results + traverse 1 hop, returns a single pre-formatted context block. One tool call instead of 4-5.
**Rationale:** Highest-leverage compounding feature. If retrieval is one call, Claude can check the vault at conversation start. Currently requires user to remember `/librarian search` and skill to orchestrate 5 sequential calls.
**Downsides:** Risk of returning too much or too little. Hard to tune "curated" algorithmically.
**Confidence:** 70%
**Complexity:** Medium
**Status:** Unexplored

## Rejection Summary

| # | Idea | Reason Rejected |
|---|------|-----------------|
| 1 | Lazy startup | Non-problem — startup is once, fast for typical vaults |
| 2 | Batch file scans | Subsumed by unified cache (idea #1) |
| 3 | Write-ahead backup | Vaults are typically git-tracked or Obsidian-synced already |
| 4 | Health check | Gold-plating for a single-user CLI tool |
| 5 | Self-healing setup | Absolute paths are correct; Homebrew handles PATH |
| 6 | Collapse commands | LLM doesn't benefit from fewer tools; ambiguity hurts routing |
| 7 | Fuzzy search | Trigrams already provide tolerance; ranking is the real fix |
| 8 | Semantic search (embeddings) | Embedding model + vector store = second project |
| 9 | Concept expansion via graph | Speculative; do BM25 first, evaluate later |
| 10 | Typed, weighted edges | Link context parsing is ambiguous and fragile |
| 11 | Semantic link inference | Same dependency burden as embeddings |
| 12 | Hierarchical communities | Nobody asked; flat clusters work |
| 13 | Temporal analysis | Requires historical data that doesn't exist |
| 14 | Velocity dashboard | Needs persistence layer from scratch |
| 15 | Predictive linking | Research problem, not engineering |
| 16 | Session pipeline automation | Workflow concern for skill/hook layer, not MCP server |
| 17 | File watching | Adds dependency + thread for niche problem |
| 18 | Decision log framing | Template/convention, not a tool — belongs in skill |
| 19 | Proactive vault push | Challenges fundamental MCP interaction model |
| 20 | Multi-tenant vaults | Requires shared storage layer — different product |
| 21 | Self-orchestrating tools | Controversial autonomy; better handled by skill layer |
| 22 | Living reports | Needs incremental graph + file watching — premature |
| 23 | Federated cross-vault graph | Node ID collision handling is complex; premature |

## Session Log
- 2026-04-11: Initial ideation — 40 raw ideas from 5 frames, 25 after dedupe, 5 survivors. Top 4 selected for brainstorm.
- 2026-04-11: Brainstormed ideas 1-4 as cohesive feature set → docs/brainstorms/2026-04-11-vault-cache-and-quality-requirements.md
