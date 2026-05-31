---
title: "feat: Judgment layer MVP — project decision & state pages + propose subcommand"
type: feat
status: active
date: 2026-05-30
origin: docs/brainstorms/2026-05-30-judgment-layer-project-decision-state-pages-requirements.md
---

# feat: Judgment layer MVP — project decision & state pages + propose subcommand

## Summary

Ship the Judgment-layer MVP from the origin brainstorm as **skill-content + page conventions in the librarian-mcp repo**, deployed via the existing `--setup` flow. No new Rust primitives. Three new `/librarian` subcommands — `project init`, `project log`, and `propose` — let any operator declare a per-project decision & state page (with an anchor outcome and a research scope), log experiment outcomes, and request ranked, cited candidate next moves drafted into a fenced managed block on the page. Voltron is the pilot.

---

## Problem Frame

Per origin: the operator (and the agents acting for them) re-derives decisions and project state at the start of every session, and lacks decision-quality help where domain experience is thin. The Judgment layer turns the vault into a falsifiable, outcome-anchored substrate. This plan executes the *how*: a small skill surface over existing librarian primitives, with a deployable artifact in the open-source repo.

---

## Requirements

(Origin R-IDs in `docs/brainstorms/2026-05-30-judgment-layer-project-decision-state-pages-requirements.md`. The plan covers R1–R14 except where the brainstorm itself deferred work to Track 2 / later surfaces.)

- R1. Per-project decision & state page identified by `type: decision-state` frontmatter; lives at a default repo convention (`Projects/<project>.md`) but works wherever the marker is present.
- R2. Frontmatter declares one or more anchor outcomes and an explicit research scope (folders / communities / tags / wikilinks). No inference.
- R3. Named sections: anchor outcomes, experiment & outcome log, ratified decisions, agent-managed candidate-next-moves block.
- R4. Operator can append experiment-log entries; existing entries are append-only and never silently rewritten.
- R5. Operator can mark an outcome as ratifying / killing a decision.
- R6. A `/librarian` skill subcommand reads the page + scoped research and writes ranked candidate next moves into a fenced managed block on the page.
- R7. Each candidate carries: proposed move, expected effect on anchor, and citations to specific research notes.
- R8. Candidates ranked by expected anchor-metric impact, drawing on prior outcome history + accumulated research.
- R9. Research-corpus composition uses **existing** librarian primitives (`library_search`, `library_traverse`, `library_cluster`, `library_tags`, `library_list`, `library_read`, `library_metadata`) — no new Rust primitive in this plan (see Key Technical Decisions).
- R10. Re-running `propose` overwrites only the managed block; everything else byte-stable.
- R11. Feature is generic: no repo-, project-, metric-, or folder-specific values are baked in. All vault-specific values come from page frontmatter or `.librarian*` config.
- R12. Operator brings the metric delta — no metric-source integrations.
- R13. `.librarianisolate` honored end-to-end: agent never cites notes from isolated folders.
- R14. Degrades gracefully: page with no scope still accepts manual logs; page with no outcome history still produces (lower-confidence) proposals from research alone.

**Origin actors:** A1 (Operator), A2 (Agent skill — `/librarian` subcommand), A3 (librarian-mcp — structure + primitives), A4 (External metric source-of-truth — not integrated).

**Origin flows:** F1 (Initialize page), F2 (Log experiment outcome), F3 (Get proposed candidate next moves), F4 (Accept/reject/edit a proposal).

**Origin acceptance examples:** AE1–AE6 (covered in unit test scenarios below).

---

## Scope Boundaries

- **Entity pages** (people, recurring concepts) — Track 1 extension, not in MVP. (Origin: Deferred for later.)
- **Automated repo→vault ingestion** (plans, ADRs, `PROJECT_STATUS.md`, brainstorm outcomes) — Track 2 (Corpus acquisition); MVP is operator-seeded. (Origin: Deferred for later.)
- **Awareness surfaces** — briefings, dashboard, ask-the-vault — different track. (Origin: Deferred for later.)
- **Direct metric-source integrations** (backtests, A/B platforms, analytics) — operator brings the delta. (Origin: Outside this product's identity for v1.)
- **Cross-project synthesis** — agent considers one project's scope at a time.
- **Mutation of operator prose** anywhere on the page — agent only writes inside the fenced managed block.

### Deferred to Follow-Up Work

- New Rust primitive for "scoped research corpus" — composes existing primitives in v1; promoted to Rust if pilot reveals performance/UX issues.
- `STRATEGY.md` update widening Track 1 wording from "retention" to "retention + steering" — documentation-only follow-up, can land alongside or after this plan.
- The `docs/brainstorms/2026-05-28-on-demand-synthesis-pipeline-requirements.md` doc still carries its old on-demand-only stance; either annotate as superseded by STRATEGY or update it. Not blocking this plan.

---

## Context & Research

### Relevant Code and Patterns

- **Fenced managed-block upsert pattern.** `src/tools.rs::upsert_related_block` (added in v0.1.2 for `library_optimize`) is the canonical pattern: find an H2 marker (`## Related (auto)`), replace the section until the next `\n## `, idempotent on re-run. The propose subcommand mirrors this pattern but with a different section name. The skill performs the upsert from the agent side (read → string-edit → write) — no new Rust helper required.
- **Frontmatter parsing.** `src/server.rs::extract_frontmatter` returns the YAML block between the leading `---` markers. The skill reads page frontmatter via `library_read` then parses; alternatively `library_metadata` returns parsed frontmatter directly.
- **Isolation enforcement.** `src/server.rs::crosses_isolation` and `top_folder` are honored automatically when the skill uses `library_write` (isolated-folder candidates are filtered from auto-link). For the `propose` *citations*, the skill explicitly filters out any candidate research note whose path starts with an isolated folder by checking `.librarianisolate` (loaded into `LibraryServer::isolated_folders` at start; alternatively the skill reads `.librarianisolate` directly).
- **Research-scope primitives** (all shipped in v0.1.2):
  - `library_search` (term-based BM25) — for tag-like or keyword scoping.
  - `library_cluster` — returns community-of-stem; for `community: <label>` scoping.
  - `library_traverse` — neighborhood from a wikilink; for explicit-anchor scoping.
  - `library_tags` — list notes by tag.
  - `library_list` + `library_read` — for folder scoping.
  - `library_metadata` — frontmatter accessor.
- **`/librarian` skill subcommand structure.** The skill currently exposes 14 subcommands (`ingest`, `import`, `from <source>`, `search`, `connect`, `daily`, `graph`, `analyze`, `daydream`, `status`, etc.). New subcommands slot in following the same SKILL.md section pattern.
- **Skill deployment.** `src/setup.rs` (invoked via `librarian-mcp --setup`) installs the skill into the operator's `~/.claude/skills/librarian/` directory. The skill template lives in the repo and is updated by re-running `--setup`. **U1 verifies the exact in-repo template path before subsequent units modify it.**

### Institutional Learnings

- From v0.1.2 work: managed-block writes must be byte-stable outside the fence (no whitespace drift). The `upsert_related_block` pattern is the proven shape.
- `.librarianisolate` semantics: it governs *link-crossing*, not *visibility*. A research listing can include isolated notes; **citations from a non-isolated page into an isolated folder must be filtered out by the skill.**
- Frontmatter writes via `library_write` round-trip cleanly; no need for a dedicated frontmatter-edit tool.

### External References

- None — fully covered by local patterns.

---

## Key Technical Decisions

- **No new Rust primitive in this MVP.** The brainstorm preferred skill-composition; the v0.1.2 primitives cover all four scope kinds (folder, community, tag, explicit wikilinks). If pilot UX or performance disappoints, a `library_corpus` convenience primitive becomes the next step — recorded under Deferred to Follow-Up Work, not this plan.
- **Three new subcommands** (`project init`, `project log`, `propose`) rather than just `propose`. `init` and `log` are small but materially support the **delta-logging operator habit** the brainstorm named as load-bearing. Each is a thin layer over `library_write` and reads frontmatter from existing tools.
- **Default page location: `Projects/<project>.md`**, generic, declared in the skill's reference file. Operators may relocate; the `type: decision-state` frontmatter marker is the canonical signal — location is convention, not constraint.
- **Fenced managed block uses an H2 marker**, consistent with the `## Related (auto)` pattern from v0.1.2. Marker: `## Candidate next moves (auto)`. Upsert is skill-side (read → replace section → write), no new Rust helper.
- **Section layout on the page** (in order): frontmatter, `# <Project>` H1, *Anchor outcomes* (operator-authored), *Experiment & outcome log* (operator-authored, append-only), *Ratified decisions* (operator-authored, standalone — brainstorm preference), `## Candidate next moves (auto)` (agent-managed fenced block). Research index does **not** get a standalone section — citations are inline in the proposed candidates (brainstorm preference).
- **Skill content is shipped from the repo** via `librarian-mcp --setup`. The new subcommand definitions live in the in-repo skill template; operators get them by re-running setup. This is the open-source distribution path.
- **Ranking is agent-side**, no Rust ranking primitive. The skill instructs the agent to rank candidates by (a) prior outcome history if present, (b) research-evidence strength (sources, recency, methodological rigor), (c) explicit confidence (`high|medium|low`) written inline in each candidate.
- **Isolation filter applied at citation time.** The `propose` subcommand reads `.librarianisolate` (or the cache equivalent) and excludes any candidate research note in an isolated top-level folder before writing the managed block. This honors R13 end-to-end.

---

## Open Questions

### Resolved During Planning

- "Just `propose` or also `project init` / `log`?" → All three; init and log support the delta-logging habit.
- "Standalone Ratified decisions section vs derived view?" → Standalone in MVP (brainstorm preference; demote post-pilot if redundant).
- "Standalone Research index section vs derived inline?" → Derived inline in propose candidates.
- "New Rust primitive for scope composition?" → No; skill composes from existing primitives (deferred Rust convenience to v2).
- "Fenced-block marker format?" → H2 section header (`## Candidate next moves (auto)`), mirroring the `## Related (auto)` pattern already shipped.

### Deferred to Implementation

- Exact in-repo path for the skill template (verified in U1; install path is `~/.claude/skills/librarian/SKILL.md`, deployed by `src/setup.rs`).
- Exact frontmatter field types and YAML shape for `research_scope` (object with `folders`, `communities`, `tags`, `wikilinks` arrays; finalize in U1).
- Whether `library_metadata` or a manual frontmatter parse in the skill is more robust for reading the page's anchor/scope on every `propose` invocation.
- Exact wording of the agent prompt inside the `propose` subcommand — needs iteration against the Voltron pilot output before declaring stable.

---

## Implementation Units

### U1. Project-page convention + frontmatter schema in the repo

**Goal:** Establish the generic project-page convention as a reference file in the repo's skill template, including frontmatter schema and the section layout. Verify the in-repo skill-template path so subsequent units can edit it precisely.

**Requirements:** R1, R2, R3, R11

**Dependencies:** None

**Files:**
- Verify: `src/setup.rs` (determines where the skill template lives in the repo and where it deploys).
- Create or modify: the repo's skill template directory — add a new reference file documenting the project-page convention (e.g., `<skill-template-dir>/references/project-pages.md`). Exact path resolved during this unit.
- Modify: the repo's `/librarian` skill template SKILL.md — add a top-of-file pointer to the new reference file.

**Approach:**
- Read `src/setup.rs` to confirm where the skill template is sourced from and where it deploys.
- Author a reference file documenting: (a) page location convention (`Projects/<project>.md` default; operators may relocate), (b) frontmatter schema — `type: decision-state`, `project`, `anchor_outcome` (string or array), `research_scope` (object with `folders`/`communities`/`tags`/`wikilinks` arrays), optional `repo`, `status`, (c) section layout in order: H1 title, *Anchor outcomes*, *Experiment & outcome log*, *Ratified decisions*, `## Candidate next moves (auto)`.
- Add an example page near the top of the reference file (Voltron-shaped: Sharpe anchor, QuantFlow community scope, two backfilled log entries) — generic enough that operators see the shape, not Neal-specific paths.

**Patterns to follow:**
- The existing skill template's reference files (e.g., the `daydream` section already references a similar pattern).
- `extract_frontmatter` in `src/server.rs` defines the YAML-parsing surface — keep the schema reachable by that parser.

**Test scenarios:**
- Test expectation: none — documentation only. Manual verification: re-running `librarian-mcp --setup` from the repo deploys the new reference file to `~/.claude/skills/librarian/references/project-pages.md` and the SKILL.md pointer is reachable.

**Verification:**
- The reference file exists in the repo's skill-template tree.
- After `librarian-mcp --setup`, the new reference file is installed to the operator's skills directory.
- The example page in the reference parses cleanly via `library_metadata` (frontmatter is valid YAML; required fields present).

---

### U2. `/librarian project init` subcommand

**Goal:** Add the subcommand that initializes a new project decision & state page — operator declares anchor outcome and research scope, page is written via `library_write` with the canonical section skeleton.

**Requirements:** R1, R2, R3, R11, F1

**Dependencies:** U1

**Files:**
- Modify: the repo's `/librarian` skill template SKILL.md — new `## project init` section under the Commands listing and a detailed subcommand block.

**Approach:**
- Define the subcommand surface: `/librarian project init <project-name>`.
- Interactive flow in the skill prompt: ask operator for anchor outcome (string or list), ask for research scope (folders / communities / tags / wikilinks — at least one), optionally ask for repo pointer.
- Write the new page via `library_write` to `Projects/<project>.md` (operators can override location via path arg; skill documents both forms).
- Skill block enforces: don't overwrite an existing page — if the path exists, report and exit.
- The initial page contents: frontmatter block + H1 + empty *Anchor outcomes* (operator fills current/target) + empty *Experiment & outcome log* + empty *Ratified decisions*. The `## Candidate next moves (auto)` fenced section is NOT pre-created — `propose` writes it on first run.

**Patterns to follow:**
- `/librarian daily` and `/librarian from <source>` subcommand blocks in the existing SKILL.md — same prompt-structure shape.
- `library_write` is the write path (auto-links, respects isolation, refreshes cache).

**Test scenarios:**
- **Covers F1.** Happy path: given a vault with no `Projects/Voltron.md`, when operator runs `/librarian project init Voltron` and answers prompts with anchor `Sharpe` and scope `community: QuantFlow`, the page is created at `Projects/Voltron.md` with the declared frontmatter and the canonical section skeleton.
- Edge case: given an existing `Projects/Voltron.md`, when the operator re-runs init, the subcommand reports the conflict and does not overwrite.
- Edge case: when no research scope is provided, the subcommand prompts again rather than writing an empty scope (R14 says degrade gracefully but init's job is to capture the operator's intent — empty scope is a slip).
- Edge case: paths and project names with spaces/punctuation are handled (operator-provided strings sanitized for filename safety).

**Verification:**
- Running the subcommand produces a page that round-trips through `library_metadata` (frontmatter parses cleanly).
- The page passes the `## Candidate next moves (auto)` block precondition for U4 (no pre-existing fenced block on first init).

---

### U3. `/librarian project log` subcommand

**Goal:** Add the subcommand that appends a single experiment-log entry to a project's *Experiment & outcome log* section — operator-driven, append-only, idempotent on the rest of the page.

**Requirements:** R4, R5, R12, F2

**Dependencies:** U1

**Files:**
- Modify: the repo's `/librarian` skill template SKILL.md — new `## project log` section.

**Approach:**
- Subcommand: `/librarian project log <project>`.
- Interactive: prompt operator for entry fields — what changed, expected effect, observed metric delta (operator brings this), link to source plan/PR/research, optional decision-ratification marker (ratifies/kills which decision).
- Compose a single dated log entry (markdown; suggested shape: bold timestamp + bullet sub-items).
- Read the project page via `library_read`, locate the *Experiment & outcome log* section header, append the entry to the END of that section (just before the next H2), write back via `library_write`.
- If the page lacks the section, surface a clear error directing the operator to `project init` (R14: degrade gracefully but don't silently invent structure).
- If the entry ratifies/kills a decision, additionally append the matching entry to *Ratified decisions* (or surface that the section needs to exist — same handling).

**Patterns to follow:**
- `upsert_related_block` for the section-find logic (find H2 marker; identify next H2 as the section's end); but unlike that pattern, **append** within the section rather than replace.
- `library_write` ensures auto-link + cache refresh on save.

**Test scenarios:**
- **Covers F2 / AE2.** Happy path: given a project page with one prior log entry, when the operator appends a new entry recording `Sharpe 1.24 → 1.41 after adding regime-slope gate`, the page now has both entries — prior entry is byte-identical, new entry is the most recent.
- Edge case: page exists but lacks an *Experiment & outcome log* section — subcommand surfaces a clear error pointing at `project init` (R14 graceful degradation: don't silently invent the section).
- Edge case: operator marks the entry as ratifying a decision; the corresponding entry is also appended to *Ratified decisions*.
- Edge case: a candidate-next-moves managed block exists on the page — its content is byte-identical after log.

**Verification:**
- A `library_read` of the page after log shows the appended entry; a diff against the prior version shows changes ONLY inside the log section (and optionally inside *Ratified decisions*).
- The agent-managed block, if present, is untouched.

---

### U4. `/librarian propose` subcommand

**Goal:** The load-bearing subcommand. Reads the project page, scopes the research corpus from frontmatter, generates a ranked list of cited candidate next moves, writes them into the agent-managed fenced block on the page — overwriting only that block.

**Requirements:** R6, R7, R8, R9, R10, R13, R14, F3, F4

**Dependencies:** U1

**Files:**
- Modify: the repo's `/librarian` skill template SKILL.md — new `## propose` section.

**Approach:**
- Subcommand: `/librarian propose <project>` (optionally `--count N` for number of candidates, default 5).
- Read the page via `library_read`; parse frontmatter to extract `anchor_outcome` and `research_scope`.
- Compose the research corpus from declared scope (skill composes from existing primitives):
  - For each `folder` in scope: `library_list` then `library_read` per `.md`.
  - For each `community` label: `library_cluster`, filter members to that community, `library_read` each.
  - For each `tag`: `library_tags` then `library_read` per match.
  - For each explicit `wikilink`: `library_traverse` depth 1, `library_read` neighbors.
  - Union, dedupe by stem.
- **Apply isolation filter:** drop any corpus note whose path's top-level folder appears in `.librarianisolate` (skill reads the file or uses cached server state).
- Read prior outcome history from the page's *Experiment & outcome log* (informs ranking).
- Agent generates 3–5 candidate next moves (default 5; configurable). Each candidate carries: the proposed move (action), expected effect on the anchor (qualitative or quantitative), explicit confidence (`high|medium|low`), and one or more citations to specific corpus notes by their relative path or wikilink.
- Rank candidates by expected anchor-metric impact, drawing on outcome history (what's moved the metric) + research evidence strength + confidence.
- Construct the managed block content: `## Candidate next moves (auto)` H2, generation timestamp, then numbered candidates.
- Upsert the block into the page (find existing `## Candidate next moves (auto)` and replace through next H2; if absent, append after the last operator section). Write via `library_write`.
- Output a short summary to the operator: count of candidates, top candidate's expected effect, count of citations.

**Patterns to follow:**
- `upsert_related_block` in `src/tools.rs` is the upsert pattern (find marker → replace until next `\n## ` → write).
- The optimizer's hub-generation in `src/optimize.rs` shows how to compose corpus from communities, group by directory, and write a fenced section — mirror its citation style.

**Test scenarios:**
- **Covers F3 / AE1.** Happy path: given the Voltron page declares anchor `Sharpe` and scope `community: QuantFlow`, when the operator runs propose, the page receives a fenced block with 3–5 ranked candidates, each citing one or more notes from the QuantFlow community and naming an expected effect on Sharpe.
- **Covers AE3 / R10.** Idempotency: given the propose block exists from a prior run, when propose runs again, the experiment log, anchor outcomes, ratified decisions, and any operator prose are byte-identical; only the candidate-next-moves block changes.
- **Covers AE4 / R13.** Isolation: given `.librarianisolate` lists `Threshold`, when propose runs on a project whose scope union includes a folder Threshold lives next to, no candidate cites any note inside `Threshold/`.
- **Covers AE5 / R14.** Sparse-history: given a project page declares anchor + scope but has no logged experiment outcomes yet, when propose runs, candidates are produced from research evidence alone and confidence is marked `medium` or `low` (not `high`).
- **Covers F4.** Loop closure: given the operator picks a candidate, makes the change, and runs `project log` recording an outcome, then re-runs propose, the ranking incorporates the new outcome (this is integration-class — verifies the candidate→log→re-propose loop).
- Edge case: page is missing required frontmatter (`anchor_outcome` or `research_scope`) — subcommand surfaces a clear error directing the operator to `project init`.
- Edge case: research_scope yields zero corpus notes — propose returns a clear "scope is empty" message and does not write an empty managed block.

**Verification:**
- `library_metadata` confirms the page's frontmatter remains valid after propose.
- A diff of the page across two propose runs shows changes confined to the managed block (header through end-of-block).
- Each candidate in the output has at least one citation that resolves to a real note in the vault (no fabricated wikilinks).

---

### U5. Voltron pilot walkthrough

**Goal:** Document the concrete Voltron pilot — anchor declaration, scope choice, backfilled experiments, first propose run, observed output — both as validation of the MVP and as the on-ramp example any operator can adapt.

**Requirements:** Success Criteria — the Voltron pilot is the validation; this unit makes it reproducible. R11 (generic): the walkthrough doubles as the "for any operator" template.

**Dependencies:** U2, U3, U4 (the subcommands exist before the walkthrough can run end-to-end).

**Files:**
- Create: `docs/walkthroughs/voltron-pilot.md` (or sibling location under `docs/`).

**Approach:**
- Step-by-step: (1) init Voltron page with anchor `Sharpe` and scope `community: QuantFlow`; (2) backfill 2–3 experiment-log entries from existing Voltron history (operator-recorded deltas); (3) run propose; (4) review the candidate-next-moves block — does any candidate name a research note Neal could not have surfaced unaided?
- Document the **operator contract test:** at end of week one, has the operator logged at least one new outcome? If yes, the load-bearing habit is holding; if no, the MVP needs UX nudges (recorded as Track-1.1 follow-up).
- Frame the walkthrough as generic — Voltron is the example, but every step works for SignUpSpark/CTR or any other project on substitution.

**Patterns to follow:**
- The pre-existing `librarian` skill's per-subcommand examples are the precedent for generic-with-example framing.

**Test scenarios:**
- Test expectation: none — documentation only. Manual verification: a fresh operator following the walkthrough end-to-end produces a working Voltron page and a propose output with at least three cited candidates.

**Verification:**
- The walkthrough is followed once end-to-end and produces a Voltron page meeting AE1, AE3, AE4 manually.
- The walkthrough's "any operator" framing holds — substitute a different project name + anchor + scope and the steps still apply.

---

## System-Wide Impact

- **Interaction graph:** New skill subcommands are read-most-of-the-time, write-only-through-`library_write`. The cache refreshes via `update_single_file` on every `library_write`, which is already the established path.
- **Error propagation:** Operator-facing errors (missing frontmatter, missing section, scope empty, page conflict on init) surface to the operator with a clear next-action; no silent invention of structure.
- **State lifecycle:** Project pages are operator-owned; only the managed block is agent-owned and overwritten on re-run. No partial-write risk because `library_write` is atomic per-file.
- **API surface parity:** No change to existing librarian-mcp Rust tools or their contracts. The new surface is at the skill level only.
- **Integration coverage:** The candidate→log→re-propose loop crosses three subcommands — covered in U4's integration test scenario.
- **Unchanged invariants:** The 0.1.2 graph, search, auto-link, isolation, and stoplist behaviors are unchanged. `library_write` continues to apply auto-link and respect `.librarianisolate` and `.librarianstoplist`.

---

## Risks & Dependencies

| Risk | Mitigation |
|------|------------|
| Agent-side ranking quality is hard to validate (no Rust ranking primitive) | Voltron pilot is the first quality gate; iterate prompt wording in U4 before declaring stable; record bad/good rankings as the prompt-tuning corpus |
| Operator skips delta-logging (the load-bearing habit) | The walkthrough names this explicitly; consider a freshness hint in propose output (e.g., "last delta logged 14d ago") as a Track-1.1 follow-up if the pilot shows decay |
| Research-corpus composition is too slow when scope is large | Skill defaults: cap per-scope-kind reads, use search/tag prefilters before full reads; if pilot UX disappoints, promote to a Rust `library_corpus` primitive (deferred follow-up) |
| Fenced-block upsert subtly breaks operator prose if section markers collide | Use the rare, unambiguous marker `## Candidate next moves (auto)` matching the existing `## Related (auto)` convention; U4 tests confirm byte-stability outside the fence |
| Skill template path in the repo is non-obvious | U1's first task is to verify it via `src/setup.rs` before subsequent units depend on its location |

---

## Documentation / Operational Notes

- Operators receive the new subcommands by re-running `librarian-mcp --setup`. The setup flow is idempotent.
- `STRATEGY.md` widens Track 1 wording from "retention" to "retention + steering" on its next pass (Deferred to Follow-Up Work).
- `docs/brainstorms/2026-05-28-on-demand-synthesis-pipeline-requirements.md` should be annotated or revised: it carries an on-demand-only stance that STRATEGY now contradicts. Not blocking this plan.

---

## Sources & References

- **Origin document:** docs/brainstorms/2026-05-30-judgment-layer-project-decision-state-pages-requirements.md
- **Strategy anchor:** STRATEGY.md (repo root) — Track 1 (Judgment layer).
- **Companion brainstorms (referenced, not in scope):**
  - docs/brainstorms/2026-05-27-obsidian-project-command-center-requirements.md (dashboard — Awareness track)
  - docs/brainstorms/2026-05-28-on-demand-synthesis-pipeline-requirements.md (pipeline — needs reconciliation with STRATEGY)
- **Related plan:** docs/plans/2026-05-28-001-feat-library-changes-primitive-plan.md (Awareness primitive, lower priority)
- **Code patterns:** `src/tools.rs::upsert_related_block`, `src/optimize.rs` (hub composition), `src/server.rs::extract_frontmatter`/`crosses_isolation`, `src/setup.rs` (skill deployment).
