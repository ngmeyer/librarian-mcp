---
date: 2026-05-28
topic: on-demand-synthesis-pipeline
---

# On-Demand Synthesis Pipeline

## Summary

An on-demand synthesis pipeline for the vault. librarian-mcp keeps the *structure* excellent (the 0.1.2 graph/search/maintenance work) and adds cheap re-synthesis **primitives**; an agent-orchestrated skill re-synthesizes on a schedule (and on demand), writing **daily and weekly briefings** as additive notes — surfacing patterns, contradictions, open questions, and the single most-surprising connection. The latest synthesis is cached into a fenced block/note purely so visual surfaces render without re-running an LLM. The MVP is the briefing loop; the spatial dashboard and an "ask the vault" Q&A surface are later surfaces of the same pipeline.

---

## Problem Frame

The vault is the daily home (the Karpathy "LLM Wiki" / "Claude + Obsidian" pattern), but it behaves like a filing cabinet, not a thinking system: you can retrieve what you saved, but opening it doesn't hand you back a synthesis, a contradiction, or a connection you wouldn't have found yourself. Re-establishing "what's going on across everything" is an active pull (`/portfolio-resume`, "what's next") and external/internal signal stays scattered.

The tempting fix — *maintain* an evolving synthesis layer that integrates every new note — was considered and rejected: it accrues staleness and maintenance debt, mutates content, and fights the direction models are moving (longer context, native memory, cheaper inference). The durable shape is the opposite: keep the **memory** (structure) excellent and cheap to maintain, and **re-synthesize on demand** — letting synthesis quality ride model improvements instead of decaying. The pain is sharpest first thing each day and whenever signal needs connecting across projects.

---

## Actors

- A1. Neal (operator): reads the daily/weekly briefings; occasionally triggers an on-demand synthesis; keeps capturing notes (upstream).
- A2. Synthesis skill (agent, scheduled + on-demand): reads a recent window of the vault via librarian primitives, synthesizes, writes the briefing note, refreshes the cache.
- A3. librarian-mcp (structure + primitives): maintains the graph/search/links/MOCs and exposes cheap re-synthesis primitives (recent-change, cluster export, contradiction candidates). Holds no LLM.
- A4. Obsidian + Dataview (render layer): renders cached synthesis / briefing notes cheaply, no LLM call on open.

---

## Key Flows

- F1. Daily briefing
  - **Trigger:** Scheduled run (e.g., morning).
  - **Actors:** A2, A3
  - **Steps:** Skill asks librarian "what changed in the last ~7 days" + pulls relevant clusters → re-synthesizes → writes an additive daily briefing note (patterns, contradictions, open questions, the one most-surprising connection) → refreshes the render cache.
  - **Outcome:** A current briefing exists without Neal asking; the cache is fresh for any surface.
  - **Covered by:** R3, R4, R5, R7

- F2. Weekly briefing
  - **Trigger:** Scheduled weekly run.
  - **Actors:** A2, A3
  - **Steps:** Deeper sweep over ~30 days; surfaces only connections Neal would *not* have found by deliberate search; writes an additive weekly briefing note.
  - **Outcome:** A periodic deeper synthesis that prioritizes non-obvious cross-topic links.
  - **Covered by:** R3, R6, R7

- F3. On-demand re-synthesis
  - **Trigger:** Neal asks for a synthesis now (a topic, a question, "what connects X and Y").
  - **Actors:** A1, A2, A3
  - **Steps:** Skill re-synthesizes live from the current structural vault → returns/writes the result.
  - **Outcome:** Fresh synthesis on request, no reliance on a maintained layer.
  - **Covered by:** R3, R8

- F4. Surface render from cache
  - **Trigger:** Neal opens a visual surface (briefing note, later the dashboard).
  - **Actors:** A1, A4
  - **Steps:** The surface renders the latest cached synthesis / briefing notes via Dataview — no LLM call.
  - **Outcome:** Instant render; the cache reflects the last scheduled/on-demand run.
  - **Covered by:** R9, R10

---

## Requirements

**Re-synthesis primitives (librarian-mcp)**
- R1. librarian exposes a "what changed since <date/window>" primitive (new/modified notes in a window) so the skill can scope a briefing without re-reading the whole vault.
- R2. librarian exposes a cluster/neighbourhood export primitive (a community's or a topic's notes) so the skill can synthesize over a coherent slice cheaply. (Extends existing graph/cluster capability.)
- R3. Re-synthesis primitives respect `.librarianisolate` / `.librarianignore` (isolated folders never bleed into a cross-topic synthesis).
- R11. librarian exposes a "candidate contradictions" primitive — pairs/sets of notes that plausibly conflict — as input the skill can vet (it surfaces candidates; the agent judges).

**Briefing loop (agent skill)**
- R4. A scheduled skill produces a **daily** briefing over a recent window (default ~7 days): patterns, contradictions, open questions, and the single most-surprising connection.
- R6. A scheduled skill produces a **weekly** briefing over a deeper window (default ~30 days) that prioritizes connections not findable by deliberate search.
- R8. The same synthesis is invokable **on demand** for an ad-hoc topic/question, not only on schedule.
- R5. Synthesis is always **re-computed** from the current structural vault on each run — there is no persisted synthesis layer that is incrementally maintained.

**Output contract**
- R7. Briefings are written as **additive** notes; the pipeline never edits the prose of existing notes.
- R12. Briefing notes are written through librarian's normal path (auto-link, cache update) so they join the graph and become corpus for future synthesis — this is what "updates the knowledge system" means here.
- R9. The latest synthesis is written into a **fenced cache** (a managed block / cache note) solely so visual surfaces can render without an LLM call; the cache is overwritten each run and is never treated as authoritative truth.

**Surfaces & config**
- R10. Visual surfaces (the briefing note today; the dashboard later) render from cached synthesis + briefing notes via Dataview, with no LLM call on open.
- R13. Cadence and window sizes (daily window, weekly window, schedule times) are set in the wide vault-level librarian config (consistent with the `.librarian*` pattern), not hardcoded.
- R14. The spatial dashboard (see `docs/brainstorms/2026-05-27-obsidian-project-command-center-requirements.md`) is a later *surface* of this pipeline, reading the same structural vault + cache — not a separate system.

---

## Acceptance Examples

- AE1. **Covers R4, R7.** Given three new notes captured in the last 7 days, when the daily run fires, a new dated briefing note is created (existing notes untouched) summarizing patterns and naming one surprising connection among them.
- AE2. **Covers R5, R9.** Given the daily run ran this morning, when it runs again tomorrow, the briefing is re-synthesized from scratch over the new window and the cache is overwritten — no merge with yesterday's synthesis.
- AE3. **Covers R10.** Given Neal opens the briefing surface between scheduled runs, it renders the last cached synthesis instantly with no LLM call (and is therefore as fresh as the last run, not live).
- AE4. **Covers R3, R6.** Given the vault has an isolated folder (e.g. the Threshold book), when the weekly briefing runs, no connection crosses that folder's boundary.
- AE5. **Covers R11.** Given two notes that assert opposing claims on the same topic, when synthesis runs, the contradiction is surfaced in the briefing's contradictions section (the primitive flags the candidate; the agent confirms it).

---

## Success Criteria

- Neal reads a briefing instead of running `/portfolio-resume` or asking "what's next"; the daily/weekly briefings tell him something he didn't already know often enough to be worth opening.
- The "filing cabinet vs thinking system" test passes: asking the vault (via the briefing or on-demand) returns a synthesis/connection, not a folder of notes.
- No maintenance debt accrues: there is no synthesis layer to go stale, and a missed run costs nothing but a stale cache until the next run.
- A downstream planner can build without inventing the pipeline shape, the on-demand-vs-maintained stance, the output contract, or where synthesis vs structure lives.

---

## Scope Boundaries

### Deferred for later

- The spatial **project dashboard** — becomes a later surface of this pipeline (its requirements are captured separately in the 2026-05-27 command-center doc).
- An **on-demand "ask the vault" Q&A** surface (cited answers to arbitrary questions) — the read side beyond scheduled briefings.
- News/RSS ingestion as a synthesis input — folded in once the briefing loop exists (it lives in the dashboard doc today).

### Outside this product's identity

- **Maintained / in-place evolving synthesis** (the LLM-Wiki "integrate every note into persistent summary pages" model) — explicitly rejected on durability grounds; the pipeline re-synthesizes instead.
- The **capture layer** (Telegram bot, ingestion, zero-friction inflow) — upstream of librarian; handled by research-intake / agent-reach, not this pipeline.
- librarian-mcp **performing synthesis itself** — the Rust server holds no LLM; synthesis is the agent skill's job. librarian provides structure + primitives only.

---

## Key Decisions

- Re-synthesis on demand over a maintained synthesis layer: avoids staleness and maintenance debt, mutates nothing, and rides model improvements (longer context, native memory, cheaper inference) instead of fighting them. This is the durability stance — settled after explicitly weighing the maintained-synthesis alternative.
- Memory vs intelligence kept separate: librarian = structural memory + cheap re-synthesis primitives; the agent skill = intelligence. The MCP server cannot and should not call an LLM.
- Additive output + render cache: briefings are new notes (never edit prose); the only persisted synthesis is a disposable cache so Dataview surfaces render without an LLM. "Updates the knowledge system" = briefings join the corpus and graph, not a maintained layer.
- Lean on the graph, not folder reorg: cross-topic connection quality comes from librarian's graph (which already bridges topic-based folders), so there is no need to re-file the vault into ZEUS-style type-based folders. The 0.1.2 graph-quality work is the enabler.
- MVP = the daily/weekly briefing loop; dashboard and ask-the-vault are later surfaces of the same engine.

---

## Dependencies / Assumptions

- A scheduling mechanism runs the skill daily/weekly (e.g. the `/schedule` skill) — *assumed available.*
- Obsidian **Dataview** renders the cache/briefings cheaply (shared with the dashboard track).
- The 0.1.2 structural graph is good enough that on-demand synthesis surfaces genuinely useful connections — *the eval metrics (recall, ranked precision) are the proxy; revisit if briefings feel generic.*
- Per-run LLM cost (daily + weekly re-synthesis) is acceptable at single-operator scale — *a recurring compute cost, not free, but small.*
- Single vault, single operator (Neal).

---

## Outstanding Questions

### Resolve Before Planning

- None — the load-bearing product decisions (on-demand vs maintained, output contract, MVP slice, structure-vs-synthesis split) were resolved during the brainstorm and recorded in Key Decisions.

### Deferred to Planning

- [Affects R1, R2, R11][Technical] Which re-synthesis primitives are worth building as Rust tools vs. composed by the skill from existing tools (search/traverse/cluster/stats).
- [Affects R11][Needs research] How to generate good *candidate* contradictions cheaply (e.g., opposing-claim heuristics over same-cluster notes) without an LLM in the server.
- [Affects R9][Technical] Cache representation — a fenced block in a dashboard note vs a dedicated cache note — and how Dataview reads it.
- [Affects R4, R6][Technical] Briefing note layout/location and how the "most-surprising connection" is selected and de-duplicated across runs.
- [Affects R13][Technical] Exact wide-config keys for cadence/windows (shares the config mechanism deferred in the dashboard doc).
