---
title: "feat: library_changes recency primitive"
type: feat
status: active
date: 2026-05-28
origin: docs/brainstorms/2026-05-28-on-demand-synthesis-pipeline-requirements.md
---

# feat: library_changes recency primitive

## Summary

Add one new MCP tool, `library_changes`, that returns the vault notes modified within the last N days, most-recent first. It reads the modification times librarian already tracks (`VaultCache.file_mtimes`) — no new indexing. This is the first structural primitive of the on-demand synthesis pipeline (origin: `docs/brainstorms/2026-05-28-on-demand-synthesis-pipeline-requirements.md`, requirement R1), built now because it is independently useful (recency view, "what's new since I was last here," session-resume) ahead of the deferred briefing loop that will also consume it.

---

## Problem Frame

The vault has no way to answer "what changed recently?" — every tool is point-in-time (search, read, graph) or whole-vault (stats, report). The future briefing loop needs a cheap "changed in the last N days" query to scope a synthesis without re-reading the whole vault, and the operator wants the same answer directly today. The data already exists in the cache (per-file mtime); nothing surfaces it.

---

## Requirements

- R1. Expose a primitive that lists notes changed within a recent window, scoped by N days, suitable for scoping a later synthesis pass and for direct recency queries. (origin R1)

**Origin actors:** A2 (synthesis skill — future consumer), A3 (librarian-mcp — provider of this primitive)
**Origin flows:** F1 (daily briefing) and F2 (weekly briefing) will consume this primitive when built; not implemented here.
**Origin acceptance examples:** none of the origin AEs target this primitive directly (they target the briefing loop); this plan adds its own scenarios.

---

## Scope Boundaries

- The briefing loop / `/librarian briefing` subcommand — deferred (origin "Deferred for later"; the user chose to build the primitive ahead of its consumer).
- The spatial dashboard surface and on-demand "ask the vault" — deferred (origin).
- Cluster-content export (origin R2) — not built; `library_cluster` + `library_read` already compose it and there is no consumer yet.
- Contradiction-candidate primitive (origin R11) — not built; genuinely novel/hard, deferred until the briefing needs it.
- No "created vs modified" distinction — the cache tracks filesystem mtime only; this primitive reports *modified* within the window (see Key Technical Decisions).

### Deferred to Follow-Up Work

- Version bump + release (e.g., 0.1.3) and Homebrew tap update once this lands: separate release step, following the established cargo-dist + tap flow.

---

## Context & Research

### Relevant Code and Patterns

- `src/cache.rs` — `VaultCache.file_mtimes: HashMap<PathBuf, SystemTime>` already holds per-file modification times, populated in `build_full` and kept current by `check_and_refresh`. The field is currently private to the cache; this plan adds a query method rather than exposing the field.
- `src/tools.rs` — tool registration pattern: a param struct deriving `Serialize/Deserialize/JsonSchema`, plus an `async fn` decorated with `#[tool(description = …)]` inside the `#[tool_router] impl LibraryServer` block. `library_daily` (writes `Journal/YYYY/…`) and `library_stats` are the closest existing handlers to mirror for "read cache → format JSON result."
- `src/tools.rs` — every tool handler calls `cache.check_and_refresh(self)` first so the result reflects current disk state; `self.relative_path(&abs)` converts absolute paths to vault-relative for output.
- `all_md_files()` / the cache already exclude `.librarianignore`d files, so a changes list inherits that exclusion for free.
- `src/tools.rs` tests module — existing unit tests (`advertises_tools_capability`, `auto_link_respects_stoplist`) construct a `LibraryServer` with empty vaults + `VaultCache::default()`; mirror this for the new test.

### Institutional Learnings

- This session's 0.1.x work established that new tools must be advertised (the `get_info` capability fix) — adding a tool needs no capability change (the router advertises it), but the regression test `advertises_tools_capability` should still pass.
- `.librarianisolate` governs *link-crossing*, not *visibility* — a recency listing intentionally does not apply isolation (it lists changed files regardless of folder; it creates no links).

### External References

- None — fully covered by local patterns.

---

## Key Technical Decisions

- Add a query method on `VaultCache` (e.g., `changed_within(cutoff: SystemTime) -> Vec<(PathBuf, SystemTime)>`) rather than making `file_mtimes` public: keeps the cache's internal representation encapsulated and gives the tool a testable, pure surface.
- Window expressed in **days** (`days`, default 7): matches the brainstorm's daily(~7)/weekly(~30) windows and is the simplest operator-facing knob. An absolute-date variant is unnecessary for the current consumers.
- Report **modified** within the window, not "created": the cache only has filesystem mtime. Document this in the tool description so callers don't assume creation semantics.
- Result shape: vault-relative `path` + a recency indicator (age in days or ISO mtime), sorted most-recent-first, optionally capped by `limit`. Exact field names are an implementation detail (see Deferred to Implementation).
- Listing ignores `.librarianisolate` (visibility ≠ link-crossing) but honors `.librarianignore` (inherited from the cache/file walk).

---

## Open Questions

### Resolved During Planning

- Subcommand vs tool: this is a librarian-mcp **tool** (Rust), not a skill verb — it's a structural primitive with no LLM involvement.
- Build ahead of consumer? Yes — `library_changes` is independently useful, so it is not speculative despite the briefing being deferred.

### Deferred to Implementation

- Exact output field names and recency representation (age-in-days vs ISO timestamp vs both).
- Whether to add an optional `directory` filter and/or `limit` param now or leave the result uncapped — decide when wiring the handler; default to a `limit` with a sensible cap if result sizes prove large.

---

## Implementation Units

### U1. `library_changes` tool + cache query method

**Goal:** Add a `changed_within` query method to `VaultCache` and a `library_changes` MCP tool that returns notes modified within the last N days, most-recent-first.

**Requirements:** R1

**Dependencies:** None

**Files:**
- Modify: `src/cache.rs` (add `changed_within` method over `file_mtimes`)
- Modify: `src/tools.rs` (add `ChangesParams` struct + `library_changes` handler in the `#[tool_router]` impl)
- Test: `src/cache.rs` (unit test for `changed_within`) and/or `src/tools.rs` tests module

**Approach:**
- `changed_within(cutoff)` filters `file_mtimes` to entries with `mtime >= cutoff`, returns them sorted by mtime descending.
- The tool: `check_and_refresh` → compute `cutoff = now - days` → call `changed_within` → map each absolute path to `self.relative_path(...)` → serialize JSON list. Default `days = 7` when omitted.
- Keep the tool description explicit that it reports *modified* (mtime) within the window and is bounded by `.librarianignore`.

**Patterns to follow:**
- Param struct + `#[tool(description=…)]` handler shape from existing tools in `src/tools.rs` (`library_stats`, `library_daily`).
- `check_and_refresh` + `relative_path` usage from any existing read tool.

**Test scenarios:**
- Happy path: given files with synthetic mtimes 2 and 10 days old and `days=7`, `changed_within` returns only the 2-day file. Covers R1.
- Happy path: multiple in-window files are returned sorted most-recent-first.
- Edge case: `days` omitted defaults to 7.
- Edge case: nothing modified within the window → empty list, not an error.
- Edge case: an `.librarianignore`d file is absent from results (inherited from the cache/file set).
- Regression: `advertises_tools_capability` still passes (the new tool is advertised by the router).

**Verification:**
- `cargo test` passes including the new `changed_within` test and the existing capability/stoplist/isolation tests.
- A manual `tools/list` handshake shows `library_changes` present; a `tools/call` returns recently-modified notes for the live vault, newest first.

---

### U2. Documentation: tool reference + counts

**Goal:** Reflect the new tool in user-facing docs so counts and references stay accurate.

**Requirements:** R1 (supporting)

**Dependencies:** U1

**Files:**
- Modify: `README.md` (tool list / count)
- Modify: `markdown/CLAUDE.md` workspace doc tool list if it enumerates tools (currently says "17 tools" — already stale; update to current count)
- Modify: the `/librarian` skill `SKILL.md` Tool Reference table (add `library_changes`) — note this lives in the user's skills dir, not this repo; flag for the operator rather than editing repo-external files silently.

**Approach:**
- Update enumerated tool counts/tables to include `library_changes` with a one-line description.

**Test scenarios:**
- Test expectation: none — documentation only, no behavioral change.

**Verification:**
- Tool counts in README/workspace doc match the actual advertised tool count.

---

## System-Wide Impact

- **API surface parity:** Adds one tool to the advertised MCP surface; no change to existing tools. Distributed via the Homebrew tap, so it reaches users only after a release (deferred follow-up).
- **Error propagation:** Read-only tool; on a missing/unreadable mtime, skip that file rather than failing the call.
- **State lifecycle risks:** None — read-only over existing cache state; `check_and_refresh` already handles staleness.
- **Unchanged invariants:** No change to auto-linking, isolation, write paths, or any existing tool's output. `.librarianignore` exclusion preserved; `.librarianisolate` intentionally not applied to a visibility listing.

---

## Risks & Dependencies

| Risk | Mitigation |
|------|------------|
| Building ahead of the (deferred) briefing consumer | Tool is independently useful (recency/session-resume); scope is one small read-only tool, low carrying cost |
| `file_mtimes` reflects librarian's own writes (auto-link rewrites bump mtime) | Acceptable — "changed" includes librarian-touched notes; document the mtime semantics |
| Large result sets on busy vaults | Optional `limit` param (deferred-to-implementation decision) |

---

## Sources & References

- **Origin document:** docs/brainstorms/2026-05-28-on-demand-synthesis-pipeline-requirements.md (R1; primitives section)
- Related code: `src/cache.rs` (`file_mtimes`, `check_and_refresh`), `src/tools.rs` (`library_stats`, `library_daily`, tests module)
- Companion brainstorm: docs/brainstorms/2026-05-27-obsidian-project-command-center-requirements.md (deferred dashboard surface)
