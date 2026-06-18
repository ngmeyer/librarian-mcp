---
name: librarian-mcp
last_updated: 2026-05-30
---

# librarian-mcp Strategy

## Target problem

Every time Neal (or an agent acting for him) starts work, he re-establishes what was already decided, where each project stands, and what's true for him personally — because that judgment lives scattered across repo docs, email, texts, and ephemeral Claude memory, never retained in one place. Decisions get re-derived, sometimes silently re-litigated, and the vault that should hold them is instead a pile of scraped research.

## Our approach

Maintain a judgment layer in the brain (decisions, rationale, entity pages, project state) as synthesis-as-docs that act as canonical commitments; recompute the awareness layer (news, what-changed, daily flux) on demand. Maintain what reduces decisions; recompute what's just news.

## Who it's for

**Primary:** Neal — single operator running a portfolio of projects (trading/Voltron/QF2, signupspark, ourgospelstudy, localcred, librarian-mcp, the Threshold book, …). He's hiring the brain to retain his accumulated judgment so he stops re-deriving past decisions and project state at the start of every session.

**Secondary:** Agents acting on Neal's behalf — Claude sessions across the portfolio. They're hiring the brain as authoritative context so they don't silently re-decide things or drift off Neal's voice.

## Key metrics

- **Decision/state freshness coverage** — fraction of active projects with a current (<14-day) decision & state page in the vault. Measurable via the vault + `library_changes`; can regress (staleness, gaps).
- **Re-derivation rate** — fraction of session-start "what did we decide / where are we / what's true for Neal" questions answered from the brain vs re-derived. Qualitative/sampled; the most direct read on decision-reduction.
- **Retrieval quality** — `library_eval`'s recall@2hops and ranked precision@10. Already measured deterministically (v0.1.2); regresses if structural rot returns.
- **Personal-corpus presence** — share of "true to Neal" entity pages (relationships, preferences, commitments) with non-empty, current content. Captures the personal-corpus gap.

## Tracks

### Judgment layer (maintained synthesis-as-docs + agent-proposed steering)

The output side: per-project decision & state pages, per-entity pages (people, projects, recurring concepts), kept current as canonical commitments. Two halves: **retention** (decisions, rationale, ratified outcomes — never re-derived) and **steering** (an on-demand `/librarian propose` reads the page's accumulated outcome history plus its declared research scope and drafts ranked, cited candidate next moves into a fenced managed block). Replaces "ephemeral synthesis" with retained judgment that also compensates for thin domain experience when the operator next picks up the project.

_Why it serves the approach:_ this IS the maintained judgment layer — without it the brain has nothing to retain (decisions stay re-derivable) and nothing to steer with (the operator works from instinct rather than evidence).

### Corpus acquisition (get the right inputs in)

The input side: pipe decision-bearing content from project repos (plans, ADRs, brainstorm outcomes, `PROJECT_STATUS`) and the personal corpus (email, texts, commitments) into the vault. Privacy-local for personal sources; never committed/published.

_Why it serves the approach:_ the biggest current gap. The brain cannot reduce decisions it doesn't contain — most of them live outside the vault today.

### Awareness layer (recompute on demand)

The ephemeral side: on-demand briefings, "what changed" recency queries (`library_changes`), the future spatial dashboard. Cheap to regenerate, no commitment value, additive output only.

_Why it serves the approach:_ keeps the maintained layer focused on decisions; everything that's just current-awareness goes here so it never accretes maintenance debt.

## Not working on

- A local mirror of the internet / a RAG search index over arbitrary scraped content — the "bad collection" failure mode.
- Maintained synthesis of ephemeral awareness (daily news pages that need reconciliation) — rejected on durability grounds; the awareness layer is recompute-only.
- librarian-mcp generating synthesis itself — the Rust server holds no LLM; synthesis is the agent's job. The server provides structure + primitives only.
