---
date: 2026-05-30
topic: judgment-layer-project-decision-state-pages
---

# Judgment Layer — Project Decision & State Pages

## Summary

A generic, open-source feature: one **decision & state page per project** in the vault, structured as an **outcome-anchored experiment log**. Each project declares one or two anchor metrics (Sharpe, CTR, conversion, retention, etc.); the page records what changed, the observed metric delta, ratified decisions, and a ranked list of **agent-proposed candidate next moves** drawn from the vault's accumulated research. Runs on-demand via a `/librarian` skill subcommand; additive-only; never mutates the operator's prose. Voltron is the pilot validation; every aspect of the design is general-purpose so the feature works for any operator on any vault.

---

## Problem Frame

The vault holds the research, but the steering decisions for each project live elsewhere — in plans, ADRs, session memory, and the operator's head. So each new session re-derives "what did we decide, what's working, what should we try next" instead of compounding judgment from the accumulated record. The pain is sharpest in projects where the operator has limited domain experience: Voltron has 30+ trading research notes (Markov, RBI, Kelly sizing, gap-and-go, multi-timeframe, etc.), but with no falsifiable record of *what was tried, what moved Sharpe, and what's most likely to move it next*, the operator is steering by guess.

The strategy reset (`STRATEGY.md`) names this as the brain's reason to exist — decision reduction. But "reduction" alone (retention) doesn't address operators with thin domain experience. **An outcome-anchored experiment log + agent-proposed next moves does both**: it stops re-derivation *and* compensates for missing instinct by grounding proposals in the accumulated research and the project's own outcome history.

---

## Actors

- A1. **Operator** (generic; Neal is the pilot) — declares each project's anchor outcome(s); logs experiment outcomes (the metric delta); ratifies decisions; accepts, rejects, or edits agent-proposed next moves.
- A2. **Agent skill** (`/librarian` subcommand) — reads the project's decision & state page plus the scoped research corpus; proposes ranked next moves with cited evidence; writes proposals as fenced managed blocks; never mutates operator prose.
- A3. **librarian-mcp** — provides structure and primitives (read/write notes with auto-link + isolation, scope a project's research via folder/community/tag, list recent changes). No LLM; no synthesis itself.
- A4. **External source-of-truth for outcomes** — wherever the metric actually lives (backtest, A/B platform, analytics, spreadsheet). **Not integrated in this MVP** — the operator brings the delta to the page.

---

## Key Flows

- F1. Initialize a project's decision & state page
  - **Trigger:** Operator starts treating a project as governed by this layer.
  - **Actors:** A1, A3
  - **Steps:** Operator creates the page (or runs a `/librarian` init command), declares anchor outcome(s) in frontmatter, declares the research scope (folder / community / tag), and optionally seeds initial experiment-log entries from existing plans/PRs.
  - **Outcome:** The project has a canonical page librarian and the agent can read and write to.
  - **Covered by:** R1, R2, R3

- F2. Log an experiment outcome
  - **Trigger:** Operator finishes (or revisits) an experiment / shipped change.
  - **Actors:** A1
  - **Steps:** Operator appends a log entry — what changed, expected effect, observed delta on the anchor metric, link to the plan/PR/research that motivated it. Optionally marks an existing decision as ratified/killed by this outcome.
  - **Outcome:** The falsifiable record grows; the page reflects what actually moved the metric.
  - **Covered by:** R4, R5, R12

- F3. Get agent-proposed candidate next moves
  - **Trigger:** Operator at a steering moment — "what should I try next?"
  - **Actors:** A1, A2, A3
  - **Steps:** Operator runs the `/librarian` propose subcommand on the project. Agent reads the page (anchor, log, ratified decisions, prior proposals), pulls scoped research via librarian primitives, and writes a ranked list of candidate next moves into a fenced managed block on the page — each candidate carrying the move, expected impact on the anchor metric, and citations to research notes that motivated it.
  - **Outcome:** A current, evidence-cited slate of next moves sits on the page for the operator to weigh.
  - **Covered by:** R6, R7, R8, R10, R13

- F4. Accept / reject / edit an agent proposal
  - **Trigger:** Operator reviews the proposed-next-moves block.
  - **Actors:** A1
  - **Steps:** Operator picks one (or none); marks the choice on the page; when the resulting experiment runs and an outcome is logged (F2), the loop closes.
  - **Outcome:** A proposal moves from candidate → ratified decision → experiment outcome → record.
  - **Covered by:** R5, R9, R12

---

## Requirements

**Page shape (generic, open-source)**
- R1. Each project gets one decision & state page identified by a `type: decision-state` frontmatter marker and a project name; the file lives in a per-vault convention (e.g., a `Projects/` folder) that the feature documents but does not enforce beyond presence of the marker.
- R2. The page's frontmatter declares one or more **anchor outcome metrics** (operator-chosen string + unit, e.g., `anchor_outcome: Sharpe` or `anchor_outcome: ["CTR", "conversion"]`), and a **research scope** (folder paths, communities, tags, or wikilinks — the operator declares what counts as the project's corpus).
- R3. The page has named, fenced sections for: **anchor outcomes** (current value, target), **experiment & outcome log** (append-only entries), **ratified decisions** (may fold into log entries — see Outstanding Questions), and **agent-proposed candidate next moves** (managed by the agent in a fenced block).

**Operator authoring**
- R4. The operator can append a new experiment-log entry (what changed, expected effect, observed delta on the anchor metric, link to source plan/PR/research). Entries are append-only; existing entries are not silently rewritten.
- R5. The operator can mark an experiment outcome as **ratifying** or **killing** a decision; ratified decisions are surfaced as a derived view (or, if `Ratified decisions` is kept as a standalone section, written there additively).
- R12. The operator brings the metric delta from the external source-of-truth (backtest, A/B platform, etc.); the feature never assumes it can read the metric directly.

**Agent steering (`/librarian` subcommand)**
- R6. A `/librarian` skill subcommand (working name: `propose`) reads the project's decision & state page plus the declared research scope and writes a ranked list of candidate next moves into a fenced managed block on the page.
- R7. Each candidate carries: the proposed move (action), expected effect on the anchor metric (qualitative or quantitative), and **citations** to specific research notes that motivated it.
- R8. Candidates are ranked by expected anchor-metric impact, drawing on prior experiment outcomes (what's worked) and accumulated research (what's been argued).
- R10. The agent re-running `propose` overwrites only the agent-managed block on the page; the experiment log, ratified decisions, anchor outcomes, and any operator-authored prose are untouched.
- R13. The agent never cites research notes from a folder listed in `.librarianisolate` — isolation is honored end-to-end.

**Project research scoping (librarian primitives)**
- R9. The agent uses librarian primitives to gather the project's research corpus from the scope declared in frontmatter (folder, community, tag, or explicit wikilinks). The exact primitive composition is a planning concern; the brainstorm decision is that scope is **operator-declared** in the page, not auto-inferred.

**Generic / open-source contract**
- R11. The feature works on any vault: no Neal-specific paths, project names, metric names, or folder conventions are baked in. All vault-specific values come from the page's frontmatter or `.librarian*` config.
- R14. The feature degrades gracefully: a project page with no research scope still produces a (degraded) experiment log and accepts manual decisions; a project page with no logged outcomes still produces (lower-confidence) proposals from research alone.

---

## Acceptance Examples

- AE1. **Covers R6, R7, R8.** Given the Voltron page declares `anchor_outcome: Sharpe` and research scope `community: QuantFlow`, when the operator runs the propose subcommand, the page receives a fenced block listing 3–5 ranked candidate moves, each citing 1+ research notes from the QuantFlow community and naming an expected effect on Sharpe.
- AE2. **Covers R4.** Given the operator appends a new experiment-log entry recording "Sharpe 1.24 → 1.41 after adding regime-slope gate, link to plan", when the page is reopened, the prior entries are unchanged and the new entry is the most recent.
- AE3. **Covers R10.** Given the propose block exists from a prior run, when propose runs again, only the propose block is overwritten — the experiment log, anchor outcomes, ratified decisions, and any operator prose remain byte-identical.
- AE4. **Covers R13.** Given `.librarianisolate` lists `Threshold`, when propose runs on a project whose research scope includes the wider vault, no candidate cites any note inside `Threshold/`.
- AE5. **Covers R12, R14.** Given a project page declares an anchor outcome but has no logged experiment outcomes yet, when propose runs, the agent produces candidates citing research only (no outcome history), and the page notes the lower confidence.
- AE6. **Covers R5.** Given an experiment outcome that confirms a hypothesis, when the operator marks the corresponding decision as ratified, that decision is visible as ratified on the next page read without rewriting prior log entries.

---

## Success Criteria

- The **Voltron pilot** is meaningful: opening Voltron's decision & state page before a steering session shows the project's anchor (Sharpe), the experiments tried with their deltas, the ratified decisions to date, and a ranked, cited candidate-next-moves block — and the operator can act from the page without re-reading the broader trading research corpus.
- **Decision-reduction** is observable: in a follow-up session, the operator does not re-derive past steering decisions for Voltron — they read them off the page.
- **Decision-quality** is observable: the operator can articulate *why* a candidate was chosen by pointing to research citations, not domain instinct they don't have.
- The feature is **generic**: a second pilot on a different project type (SignUpSpark CTR, or another) works with no code change — only different frontmatter (anchor + scope).
- A downstream planner can implement without inventing the page shape, the propose contract, the additive/managed-block boundary, or the operator-declared-research-scope mechanism.

---

## Scope Boundaries

- **Entity pages** (people, recurring concepts) — same Track 1 family but not in this MVP; a future widening once project pages are working.
- **Automated ingestion from repos** (plans, ADRs, brainstorm outcomes, `PROJECT_STATUS.md`) — Track 2 (Corpus acquisition). MVP is operator-seeded: the operator pastes/links existing plan content if useful, but no automated repo→vault sync.
- **Awareness surfaces** — daily/weekly briefings, the spatial dashboard, on-demand "ask the vault" — different track (Awareness); not built here.
- **Reading external metric sources directly** — backtests, A/B platforms, analytics. The operator brings the delta; metric integrations are out of scope for v1.
- **Auto-scheduled propose runs** — the propose subcommand is on-demand only. No daemon, no cron. Avoids the maintenance-debt failure mode.
- **Cross-project synthesis** — the agent considers one project's scope at a time; portfolio-level synthesis is a future feature.
- **Mutation of operator prose anywhere on the page** — the agent only writes inside its fenced managed block.

---

## Key Decisions

- **Outcome-anchored, not decision-anchored:** every log entry is evaluated against a declared anchor metric (Sharpe, CTR, conversion, retention…). Makes the record falsifiable and lets accumulated outcomes compound judgment — directly addresses the "thin domain experience" pain that retention alone doesn't.
- **Retention + agent-proposed steering** is the MVP scope, not retention-only. This widens Track 1 beyond `STRATEGY.md`'s current wording (which still says retention) — strategy needs a small update on its next pass.
- **On-demand `/librarian` subcommand, operator-driven** — agent does not auto-maintain the page; the operator runs propose at a steering moment. Avoids maintenance debt; respects the additive contract.
- **Additive + fenced managed blocks** (the `## Related (auto)` pattern, generalized) — agent only writes within its block; operator prose is never touched. This is the durability story: stale agent content is always re-derivable, never blocking.
- **Operator-declared research scope** in frontmatter (folder/community/tag/wikilinks), not auto-inferred — keeps the feature generic and predictable; the agent never has to guess what "Voltron research" means.
- **Operator brings the metric delta** — the feature is generic precisely because it doesn't integrate with any specific metric source. The trade-off is operator discipline: if deltas aren't logged, the log decays back into narrative.
- **Voltron is the pilot, not the design target** — every requirement is generic; Voltron just stress-tests them.

---

## Dependencies / Assumptions

- The 0.1.2 librarian-mcp substrate (graph, search, write with auto-link + isolation) is the foundation; assumed live.
- The agent (Claude in a `/librarian` skill subcommand) handles the LLM work — proposing, citing, ranking. The Rust MCP server holds no LLM and does not perform synthesis itself.
- A scoping primitive exists or can be cheaply composed to gather a project's research corpus from frontmatter declaration (folder path, community, tag, or explicit wikilinks) — verify against `library_cluster` / `library_search` / `library_traverse` in planning.
- Anchor metric source-of-truth (backtest, analytics, etc.) is operator-managed; no external integrations in this MVP.
- Single operator per vault (matches `STRATEGY.md`'s Who section); multi-operator concurrency on the same page is not a current concern.

---

## Outstanding Questions

### Resolve Before Planning

- None — the load-bearing product decisions (scope = retention + steering, outcome-anchored shape, on-demand subcommand, additive + managed blocks, operator-declared research scope, operator-brings-deltas, generic across vaults) were resolved during the brainstorm and recorded in Key Decisions.

### Deferred to Planning

- [Affects R3, R5][Soft scope] Whether **Ratified decisions** is a standalone section on the page or a derived view of the experiment log (each log entry that ratifies/kills a decision). Brainstorm preference: include as standalone in MVP, demote to derived after the pilot if it proves redundant.
- [Affects R9][Soft scope] Whether **Research index** is a hand-curated section on the page or auto-derived from the declared research scope. Brainstorm preference: derive from the scope (no manual section) unless the pilot shows the operator needs to override.
- [Affects R1, R3][Technical] Exact vault-folder convention for project pages (e.g., `Projects/<name>.md`) and the frontmatter schema (field names, types).
- [Affects R9][Technical] How the agent composes "the project's research corpus" from the declared scope using existing librarian primitives (`library_cluster`, `library_search`, `library_traverse`, the pending `library_changes`).
- [Affects R6, R10][Technical] Fenced-block protocol for the agent-proposed section: marker syntax, idempotency on re-runs, preserving prior proposals as history vs overwriting.
- [Affects R8][Needs research] How the agent ranks candidates by expected anchor-metric impact when prior outcome history is sparse — agent-only judgment with cited research vs explicit confidence scoring.
- [Affects all] `STRATEGY.md` update on next pass: Track 1 widens from "retention" to "retention + agent-proposed steering" to honor the MVP scope chosen here.

---

## The operator contract (worth naming explicitly)

The feature's whole loop depends on one operator habit: **logging the outcome delta when an experiment completes**. If deltas aren't logged, the page decays into a narrative log and the falsifiability advantage evaporates. The MVP spec should:
- Surface this in the propose subcommand's output ("last delta logged: 14 days ago" as a freshness flag),
- Optionally remind on session-close when an experiment shipped without an outcome entry,
- Treat "delta-logging hygiene" as the load-bearing metric the pilot watches.

This is the equivalent of ZEUS's "capture is the single most important piece of infrastructure" rule, applied to this layer.
