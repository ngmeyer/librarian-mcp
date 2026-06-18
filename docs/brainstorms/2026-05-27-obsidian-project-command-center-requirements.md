---
date: 2026-05-27
topic: obsidian-project-command-center
---

# Obsidian Project Command Center

## Summary

A native-Obsidian project command center, powered by librarian-mcp as a data layer. librarian maintains structured per-project **status notes** (written at session-close) and per-project **news notes** (fetched from RSS feeds on a schedule); a **Dataview dashboard note** renders both into status tiles and per-project news inside Obsidian — so opening one note replaces asking "what's next."

---

## Problem Frame

Neal runs a portfolio of active projects (trading/Voltron/QF2, signupspark, ourgospelstudy, localcred, librarian-mcp, the Threshold book, etc.). Today, re-establishing "where is everything and what should I do next" is an **active pull**: he runs `/portfolio-resume` or `/project-status`, or simply asks the agent "what's next." Each answer is generated on demand and evaporates — there's no persistent surface that's already current when he opens his vault in the morning. External signal (releases, news, competitor moves) for each project is even more scattered: it lives in feeds and sites he'd have to check project by project.

The cost is recurring friction at the start of every work session and staleness between sessions. The vault is already the daily home (Obsidian, the Karpathy "LLM wiki" pattern librarian leans into), but it has no living front page. The pain is sharpest first thing in the day and whenever Neal context-switches between projects.

---

## Actors

- A1. Neal (operator): opens the dashboard daily, consumes status + news, configures which RSS feeds each project watches.
- A2. session-close agent: at the end of a project work session, writes/updates that project's status note in the vault.
- A3. scheduled news agent: on a cadence, fetches each project's RSS feeds and writes news notes into the vault.
- A4. librarian-mcp (data layer): provides tools to write status/news notes and to scaffold/refresh the Dataview dashboard note. Does **not** render the dashboard.
- A5. Obsidian + Dataview (render layer): renders the dashboard note from the structured status/news notes.

---

## Key Flows

- F1. Daily glance
  - **Trigger:** Neal opens the dashboard note in Obsidian.
  - **Actors:** A1, A5
  - **Steps:** Dataview queries the status notes → renders per-project tiles (health, next action, last-updated age) → renders recent news per project below/beside each.
  - **Outcome:** Neal sees the whole portfolio's state and fresh signal at a glance, without running a command or prompting.
  - **Covered by:** R7, R8, R9

- F2. Status update at session-close
  - **Trigger:** A project work session ends and `session-close` runs.
  - **Actors:** A2, A4
  - **Steps:** session-close synthesizes the project's current status (health, next action, blockers) → writes/updates that project's status note via librarian.
  - **Outcome:** That project's tile reflects reality as of the just-finished session.
  - **Covered by:** R3, R4

- F3. Scheduled news refresh
  - **Trigger:** Scheduled run (e.g., daily).
  - **Actors:** A3, A4
  - **Steps:** For each project with configured feeds, fetch RSS/Atom entries → dedupe against existing news notes → write new entries as news notes tagged to the project.
  - **Outcome:** Each project's news is current; no duplicates accumulate.
  - **Covered by:** R5, R6, R11

- F4. Configure a project's feeds
  - **Trigger:** Neal adds a project or wants to watch new sources.
  - **Actors:** A1, A4
  - **Steps:** Neal records the project's RSS feed URLs in the project's status note (or registry) → next scheduled run picks them up.
  - **Outcome:** The project participates in the news pipeline.
  - **Covered by:** R2, R5

---

## Requirements

**Data model**
- R1. Each project has a single status note carrying at least: project name, health/status, next action, blockers, last-updated timestamp, link to the repo, and the project's RSS feed list.
- R2. The set of projects the dashboard covers is exactly the set of status notes present in the vault — no separate registry note. An `active` frontmatter flag hides a project without deleting its note; `group` and `order` fields control sectioning and sort.
- R11. Each news item is a note carrying at least: owning project, title, source, publish date, and URL — enough for Dataview to group, sort, and link it.

**Status pipeline (session-driven)**
- R3. `session-close` writes/updates the active project's status note as part of its existing reconcile step.
- R4. A status update overwrites the prior status for that project (latest-wins) and refreshes the last-updated timestamp.

**News pipeline (scheduled RSS)**
- R5. A scheduled agent fetches each project's configured RSS/Atom feeds and writes new entries as news notes.
- R6. News ingestion is idempotent: an entry already present (by URL) is not duplicated on re-runs.
- R12. A project with no configured feeds is handled gracefully — it simply shows no news, never an error.

**Dashboard rendering (Dataview, in Obsidian)**
- R7. A dashboard note renders one tile/row per project showing health/status, next action, and last-updated age, using Dataview/dataviewjs + CSS.
- R8. Each project surfaces its most recent N news items inline, linked to source.
- R9. The dashboard visibly flags stale status — when a project's last-updated age exceeds the staleness threshold (default 30 days), the tile signals it (e.g., a "stale" marker), so old status is never read as current. The threshold is one vault-wide setting, not per-project.
- R10. The dashboard requires no manual rebuild to reflect new status/news notes — opening/refreshing the note re-runs the queries.

**librarian-mcp tooling**
- R13. librarian provides a tool to scaffold the dashboard note and the status/news note structure for a vault (so the feature can be stood up and re-applied generically on any vault).
- R14. librarian's status/news note writes go through its normal write path (auto-link, cache update) and respect `.librarianisolate` / `.librarianignore`.
- R15. Tunable settings (staleness threshold default 30 days, recent-news count N, news cadence) live in one wide vault-level librarian config, consistent with the existing `.librarian*` config pattern — not scattered per-project.

---

## Acceptance Examples

- AE1. **Covers R9.** Given a project whose status note was last updated 40 days ago and the default 30-day staleness threshold, when Neal opens the dashboard, that project's tile shows a stale marker alongside its (old) status.
- AE2. **Covers R6.** Given a news note already exists for a feed entry URL, when the scheduled agent re-runs, no second note is created for that URL.
- AE3. **Covers R12.** Given a project with zero configured feeds, when the dashboard renders, that project shows its status tile with an empty (not errored) news section.
- AE4. **Covers R3, R4.** Given Neal finishes a session on `librarian-mcp`, when `session-close` runs, the librarian-mcp status note reflects the new next-action and a fresh timestamp, replacing the prior one.

---

## Success Criteria

- Neal stops asking "what's next" / running `/portfolio-resume` to start the day — he opens the dashboard instead.
- On open, the dashboard reflects reality: recently-worked projects show current status; untouched projects show clearly-flagged stale status; news is as fresh as the last scheduled run.
- A downstream planner (`ce-plan`) can implement without inventing the data shapes, the pipeline split, or the staleness behavior — they're specified here.
- The scaffold tool stands the dashboard up on a fresh vault without manual note authoring.

---

## Scope Boundaries

- **Browser / iframe HTML dashboard** — the rich standalone `.html` (à la `GRAPH_VIZ.html`) and embedding it in Obsidian were considered and rejected in favor of native Dataview. Kept as a documented escape hatch (see Key Decisions), not built.
- **Non-RSS news sources** — agent-reach social/web queries and reusing `/research-intake` output were considered and set aside; news is RSS/Atom only for v1.
- **Scheduled status scraping** — status is session-driven only. No agent sweeps repos on a schedule to refresh status; untouched projects rely on the staleness flag instead.
- **Multi-user / sharing** — single operator (Neal), single vault. No collaboration, permissions, or hosting.
- **librarian rendering the dashboard** — librarian maintains data; Dataview renders. librarian does not generate dashboard HTML for this feature.

---

## Key Decisions

- Native Obsidian (Dataview) over browser HTML: keeps the dashboard inside the daily tool with zero context switch; accepts a visual ceiling (badges/tables/callout-cards, not arbitrary tiles) as the cost. The HTML path remains the escape hatch if that ceiling later disappoints.
- librarian as data layer, Dataview as render layer: librarian writes structured notes; Obsidian draws them. Plays to librarian's strengths and sidesteps Obsidian's HTML/JS sanitization.
- Hybrid freshness: status from `session-close` (accurate — the agent was just in the repo), news from a scheduled job. Splits the freshness problem by data type.
- RSS/Atom as the news source: stable, low-noise, matches the "feed" mental model; per-project feed URLs are operator-configured.
- Staleness is surfaced, not hidden: because status is session-driven, the dashboard must make age visible so stale state is never mistaken for current.
- Staleness threshold = 30 days, global, set in one wide vault-level config (not per-project): a single knob to tune, consistent with the `.librarian*` config pattern, avoiding per-project config sprawl.
- Project registry = presence of a status note (no separate roster): self-maintaining; `group`/`order`/`active` frontmatter provides grouping, sort, and hide — ~95% of a dedicated roster's control with none of the two-place sync cost.

---

## Dependencies / Assumptions

- Obsidian **Dataview** plugin is installed and enabled in the vault (hard dependency for rendering).
- An RSS fetch capability is available to the scheduled agent (e.g., the agent-reach RSS channel) — *assumed available, verify during planning.*
- A scheduling mechanism exists to run the news agent on a cadence (e.g., the `/schedule` skill).
- Single vault, single operator (Neal); project status is meaningful to maintain in the vault.
- The richness the operator wants is achievable within Dataview/dataviewjs + CSS — *assumption flagged in Call outs; if false, revisit the HTML escape hatch.*

---

## Outstanding Questions

### Resolve Before Planning

- None — the two open product decisions (staleness threshold; registry mechanism) were resolved during the brainstorm and recorded in Key Decisions (staleness = 30 days global via wide config; registry = status-note presence).

### Deferred to Planning

- [Affects R15][Technical] Exact wide-config format and whether it consolidates the existing `.librarian*` dotfiles or adds a new one.
- [Affects R1, R11][Technical] Exact frontmatter schema and vault folder layout for status and news notes.
- [Affects R7, R8][Technical] Dataview vs dataviewjs for the tiles, and the CSS-snippet approach for status badges/cards.
- [Affects R6][Technical] Dedup mechanism and where the "seen URLs" state lives.
- [Affects R5][Needs research] Whether the agent-reach RSS channel covers the needed feed formats, or a dedicated fetch step is required.

---

## Adjacent feature candidates (broader "what else?" ask — not specced here)

A menu surfaced by the original prompt, for separate brainstorms/prioritization. Each leans into the Karpathy LLM-wiki positioning:

- **Vault-as-RAG context injection** — on `library_read`, auto-append the most relevant related notes (using the now-strong search + traversal) so any note read pulls its neighborhood. Extends the retrieval work just shipped.
- **Scheduled vault maintenance** — run `library_optimize` (dry-run) / `library_connect` / `library_eval` on a cadence and write a health note; keeps the graph optimized without manual runs.
- **"Ask the vault" Q&A tool** — retrieve + synthesize an answer with note citations (the read side of brain-RAG), surfacing the vault's knowledge conversationally.
- **Tag taxonomy enforcement** — the self-learn gap report flagged ~0% tag usage; a tool to propose/apply a small controlled tag vocabulary would unlock cross-folder slicing (and better Dataview queries for the dashboard).
- **Daily/weekly digest note** — auto-generate a rollup of what changed across the vault (new notes, status changes, news) — a temporal companion to the spatial dashboard.
- **Proactive link suggestions** — surface high-confidence unlinked-mention candidates as a review queue, instead of only on demand via `connect`.
