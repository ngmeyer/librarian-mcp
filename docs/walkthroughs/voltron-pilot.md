# Pilot walkthrough — Voltron decision & state page

A step-by-step on-ramp for the Judgment layer (Track 1 of `STRATEGY.md`), using Voltron — a trading project — as the worked example. **The walkthrough is generic.** Voltron is the example; substitute any project name, any anchor metric, and any research scope and the same steps apply.

Plan reference: `docs/plans/2026-05-30-001-feat-judgment-layer-mvp-plan.md` · Origin: `docs/brainstorms/2026-05-30-judgment-layer-project-decision-state-pages-requirements.md`

---

## What you need before starting

- librarian-mcp v0.1.2+ installed and configured against your vault.
- The `/librarian` skill installed (run `librarian-mcp --setup <vault-path>` to (re)install).
- A project where you've made decisions you'd like to retain and have research in the vault you'd like the agent to draw from.
- One anchor metric you genuinely measure. Be honest — if you don't measure it now, pick something you can.

For the example: Voltron's anchor is **Sharpe**; the trading research lives in the vault under the `QuantFlow` community (auto-detected via `library_cluster`).

---

## Step 1 — Initialize the page

```
/librarian project init Voltron
```

The skill prompts for:

| Field | What to enter | Example for Voltron | Substitute for your project |
|---|---|---|---|
| Anchor outcome(s) | The metric you optimize | `Sharpe` | `CTR` for an email campaign; `conversion` for a signup flow; `retention` for an app; whatever you measure |
| Research scope | At least one of folders / communities / tags / wikilinks | `communities: ["QuantFlow"]` | `tags: ["signupspark", "gtm"]` for SignUpSpark; `folders: ["Research/PithyByte"]` for the writing project |
| Repo (optional) | Path or URL | `/Users/nealme/Projects/voltron` | The repo where the actual change-work happens |

Result: `Projects/Voltron.md` exists with frontmatter, empty Anchor outcomes / Experiment log / Ratified decisions sections, no auto block (yet).

**Substitution test:** running `/librarian project init SignUpSpark` with anchor `CTR` and scope `tags: ["signupspark"]` produces the same shape with no skill code change. That's the generic contract holding.

---

## Step 2 — Fill in the anchor's current value and target

Open `Projects/Voltron.md` in your editor and complete the `## Anchor outcomes` section by hand:

```markdown
## Anchor outcomes
- **Sharpe:** current 1.24 (last measured 2026-05-15) · target ≥ 1.50
```

This is operator-owned territory; no command writes it for you. Keep it short — one line per anchor.

---

## Step 3 — Backfill 2–3 prior experiments

Run `/librarian project log Voltron` once for each meaningful prior experiment you remember. The skill prompts for the entry fields; you bring the deltas from your source-of-truth (backtest, A/B platform, analytics).

Example backfill entries for Voltron:

```
$ /librarian project log Voltron

  What changed: Added regime-slope gate (slope ≥ 0.003)
  Expected: reduce chop-day false entries
  Observed delta: Sharpe 1.18 → 1.24 over 30 trading days
  Source: [[Voltron VWAP mean reversion achieves Sharpe 1.24 on 59-day backtest using 5K fixed bets and 3pct stops]]
  Ratifies a decision? Yes — "Keep regime-slope gate"

$ /librarian project log Voltron

  What changed: Tightened stop from 3% → 2.5%
  Expected: reduce per-loss magnitude
  Observed delta: Sharpe 1.24 → 1.21; reverted 2026-05-05
  Source: backtest in repo
  Ratifies a decision? Yes — kills "Tighten stop to 2.5%" (we keep 3%)
```

After this step the page has a falsifiable record. Two experiments isn't enough for high-confidence ranking on its own — but combined with the research corpus in Step 4, the agent can already produce useful candidates.

**Substitute equivalent:** for SignUpSpark this might be `Switched subject line from A→B; CTR 12.3% → 14.1%` with source `[[Email A/B test results 2026-05]]`.

---

## Step 4 — Run propose

```
/librarian propose Voltron
```

The agent will:
1. Read the page (anchor: Sharpe; scope: community QuantFlow).
2. Compose the research corpus by pulling QuantFlow-community notes (capped at ~80 if larger; you'll see a note in the output if so).
3. Apply the isolation filter — if `.librarianisolate` lists any folder, citations from that folder are dropped.
4. Read the prior 2 experiment outcomes (regime-slope gate, stop-tightening kill).
5. Rank 5 candidate next moves with citations, confidence levels, and expected effects on Sharpe.
6. Write the result into a new `## Candidate next moves (auto)` block at the end of the page.

Open the page and read the block. A useful first pilot output looks roughly like:

```markdown
## Candidate next moves (auto)

_Generated 2026-05-30 by `/librarian propose`. Corpus: 28 notes scoped from community QuantFlow. Re-running this command overwrites only this section._

### 1. Add a Markov persistence gate at τ=0.85 over a 60-bar window · confidence: medium

**Expected:** Adds an independent regime signal on top of the slope gate; should reduce false entries on transition days without sacrificing the trend captures the slope gate already wins.

**Why:** The Markov approach is a strictly more informative regime gate than the scalar slope you already use; the supporting source claims real PnL on prediction markets with τ=0.87, but lower-confidence on equities. Your prior log shows the slope gate moved Sharpe +0.06, so an additional independent gate is the strongest next swing.

**Citations:** [[Markov transition matrix with 0.87 diagonal persistence threshold produced $1.3M on Polymarket across 3 bots in 30 days (0xRicker)]], [[Combining 50 weak signals at IC 0.05 beats one strong signal at IC 0.10 using the Fundamental Law IR equals IC times root N (RohOnChain)]]

### 2. ...
```

This is the moment of truth for the pilot. Two questions to answer honestly:

- **Does any candidate name a research note you would not have surfaced unaided?** If yes, the agent is compensating for thin domain experience — the bet is paying off. If no, either the scope is too narrow or the research corpus doesn't contain anything you didn't already have in your head.
- **Is the ranking defensible?** Read the rationale. If the top candidate is supported by citations you'd consider weak, that's a prompt-tuning signal, not a feature failure. Note it; the propose subcommand's prompt iterates over the next few pilot runs.

---

## Step 5 — Close the loop

Pick the candidate you find most compelling (or pick none — the page has already done its job by retaining decisions and surfacing what's been tried).

If you act on a candidate:
1. Do the work in the project repo.
2. When the outcome is measurable, run `/librarian project log Voltron` to record the delta.
3. Re-run `/librarian propose Voltron`. The new log entry now informs the ranking — the loop is closed.

The candidate-next-moves block is overwritten on the second propose run; everything else on the page is byte-identical. If it isn't, that's a regression — file an issue against U4's acceptance examples (AE3 / R10).

---

## The operator-contract check (end of week one)

The Judgment layer's whole value rests on **logging the outcome delta when an experiment completes**. If you don't, the page decays into a narrative log and the falsifiability advantage evaporates.

After one week of pilot use, ask yourself:

| Question | Healthy answer | Unhealthy answer |
|---|---|---|
| How many experiments have I logged this week? | At least one if anything shipped; zero if nothing shipped (acceptable) | Zero despite shipped work — the habit hasn't taken hold |
| When I opened the project page mid-week, did I read it? | Yes, and I changed a decision based on it | No, I asked "what's next" elsewhere |
| Did `propose` surface anything I wouldn't have surfaced unaided? | Yes, at least once | No — the agent is restating what I already know |

If any answer is unhealthy, that's a signal to revisit the operator habit (delta-logging) or the propose prompt — not to abandon the page. Carry forward to the next pilot iteration.

---

## Substituting a non-trading project

The Voltron run above is the example. Here is how each step translates:

| Step | Voltron (trading) | SignUpSpark (GTM) | Any project |
|---|---|---|---|
| 1 — Init anchor | `Sharpe` | `CTR` or `conversion` | Whatever you measure |
| 1 — Init scope | `communities: ["QuantFlow"]` | `tags: ["gtm", "signupspark"]` | Pick the scope that names YOUR research |
| 3 — Backfill | Backtest results | Past A/B test results | Whatever moved (or didn't move) the metric |
| 4 — Propose | Ranked Markov / Kelly / regime variants | Ranked subject lines / send-time / segment variants | The agent draws from whatever research the scope captures |
| 5 — Loop | Ship → backtest → log → re-propose | Ship → measure CTR → log → re-propose | Same shape |

The shape is invariant. Substitution is by frontmatter only, not code.

---

## When to graduate beyond the MVP

This pilot validates the MVP scope (one project, one operator, manual init + log + propose). Real signs to consider the next track:

- Several projects are running on this layer and the operator wants a portfolio view → the **Awareness track** (dashboard, briefings).
- Operator wants to scope research by something the four scope keys (folders / communities / tags / wikilinks) can't express cleanly → consider a Rust `library_corpus` convenience primitive (Track-1.1 follow-up noted in the plan).
- Plans, ADRs, and `PROJECT_STATUS.md` content the operator wants flowing into the page from repos automatically → the **Corpus acquisition track** (Track 2).

Until one of those signals fires, the MVP is enough. Keep logging deltas.
