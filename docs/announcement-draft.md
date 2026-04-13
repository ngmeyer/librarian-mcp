---
title: "I built a vault that teaches itself"
date: 2026-04-12
type: announcement
target: PithyByte article + Twitter thread
---

# I built a vault that teaches itself

Last week I ran a command on my 207-file Obsidian vault. It audited the knowledge graph, found 14 gaps in my research, then went to the internet, evaluated sources, and filed 8 new notes — cited, structured, and wikilinked into the graph. 

No manual searching. No copy-pasting. No tab chaos.

The vault got smarter while I made coffee.

---

## The problem with "second brains"

Most Obsidian vaults are graveyards. You save notes with good intentions, then never connect them. The graph view looks impressive but the knowledge doesn't compound.

I had 207 files across agent architecture, trading strategies, GTM playbooks, and skills research. Some were excellent. Most were disconnected islands. The vault *stored* knowledge but it didn't *grow*.

## What I built

Three pieces that work together:

### 1. Librarian MCP — the engine

An MCP server that gives Claude direct access to your vault. 17 tools: search (trigram-indexed), auto-wikilink on write, backlinks, tags, graph traversal, community detection, and interactive visualization.

```bash
brew install ngmeyer/tap/librarian-mcp
librarian-mcp --setup ~/my-vault
```

One command. Claude can now read, write, search, and analyze your vault.

### 2. Research Skill Graph — the methodology

A 16-file interconnected vault that forces every research topic through 6 analytical lenses:

- **Technical** — what do the numbers say?
- **Economic** — follow the money
- **Historical** — what patterns repeat?
- **Geopolitical** — zoom to the global chessboard
- **Contrarian** — what if the consensus is wrong?
- **First Principles** — rebuild from fundamentals

Each lens produces findings that often contradict the others. The tension between lenses is where the real insight lives.

When I ran "why are prediction market edges compressing?" through this system, the technical lens said "arbitrage spreads dropped from 5% to sub-1%." The contrarian lens said "the favourite-longshot bias has persisted for 80 years and won't be arbitraged away because it's rooted in human psychology." Neither is wrong. The truth lives in the tension.

### 3. Self-Learn — the autopilot

A skill that audits the vault for 6 types of knowledge gaps:

1. **Orphan clusters** — notes disconnected from the graph
2. **Dead-end topics** — surface coverage, no depth
3. **Missing foundations** — concepts referenced but never defined
4. **Stale knowledge** — outdated information in fast-moving fields
5. **Missing perspectives** — topics covered from one angle only
6. **Disconnected bridges** — clusters that should be linked but aren't

For each gap, it formulates search queries, evaluates sources with a 5-tier trust system, synthesizes across multiple sources, and writes the findings back to the vault — auto-wikilinked into the graph.

## What actually happened

I pointed self-learn at my vault. It found:

- **14 knowledge gaps** ranked by centrality, actionability, and freshness
- My 22-note agent architecture cluster had **zero contrarian coverage** — every note was enthusiastic about agents, none asked when they fail
- My 14-note trading cluster had **zero synthesis** — individual notes collecting dust with no integrated view
- **9 broken wikilinks** pointing to files that no longer existed

Then it went to work. Two digest notes came back that changed how I think about these topics:

**Trading digest:** "Risk infrastructure is stronger than alpha generation, and no strategy has out-of-sample validation." The vault had 14 strategy notes but nobody had asked: does any of this actually work on fresh data? (Answer: we don't know.)

**Agent architecture digest:** "Multi-agent systems average -3.5% worse than single-agent on 14,742 DeepMind runs." My vault had 22 pro-agent notes and zero skepticism. The DeepMind data says most people building multi-agent systems are making things worse.

The vault didn't just store these findings. It connected them to existing notes, updated the knowledge graph, and flagged new open questions for future runs.

## Try it

```bash
# Install
brew install ngmeyer/tap/librarian-mcp

# Point at your vault
librarian-mcp --setup ~/my-obsidian-vault

# In Claude Code:
/librarian analyze     # see your graph
/librarian search      # find anything
/librarian daydream    # discover connections
```

The Research Skill Graph example ships with the repo — clone and start researching in 5 minutes.

**GitHub:** github.com/ngmeyer/librarian-mcp

---

## Twitter/X Thread (extracted)

**1/** I built a vault that teaches itself.

Ran one command on my 207-file Obsidian vault. It found 14 knowledge gaps, researched 8 of them, and filed sourced notes — auto-wikilinked into the graph.

The vault got smarter while I made coffee. Here's the system: 🧵

**2/** The problem: most Obsidian vaults are graveyards. You save notes, never connect them. My vault had 207 files across trading, agent architecture, and GTM. Some great. Most disconnected.

The vault STORED knowledge but didn't GROW.

**3/** Piece 1: Librarian MCP — gives Claude direct access to your vault.

17 tools: trigram search, auto-wikilinks, backlinks, graph traversal, community detection, and interactive visualization.

One command to install:
brew install ngmeyer/tap/librarian-mcp

**4/** Piece 2: Research Skill Graph — forces every topic through 6 lenses.

Technical, Economic, Historical, Geopolitical, Contrarian, First Principles.

The lenses often contradict each other. That tension is where real insight lives.

**5/** Piece 3: Self-Learn — audits your vault for 6 types of knowledge gaps.

Orphan clusters. Dead-end topics. Missing foundations. Stale knowledge. Missing perspectives. Disconnected bridges.

Then researches the internet to fill them. Automatically.

**6/** What it found in MY vault:

- 22 agent architecture notes, ZERO contrarian coverage
- 14 trading strategy notes, ZERO synthesis
- 9 broken wikilinks to deleted files
- 3 index pages nobody ever links back to

The vault looked rich. The graph said otherwise.

**7/** The trading synthesis was brutal:

"Risk infrastructure is stronger than alpha generation. No strategy has out-of-sample validation."

14 notes. Not one asked: does this actually work on fresh data?

**8/** The agent architecture synthesis was sobering:

"Multi-agent averages -3.5% vs single-agent on 14,742 DeepMind runs."

22 pro-agent notes. Zero skepticism. The data says most multi-agent systems make things WORSE.

**9/** The self-learn skill doesn't just find gaps. It:

- Evaluates sources with a 5-tier trust system
- Synthesizes across multiple sources
- Writes findings back to the vault
- Auto-wikilinks into the knowledge graph
- Logs open questions for future runs

**10/** Try it:

brew install ngmeyer/tap/librarian-mcp
librarian-mcp --setup ~/vault

Then in Claude Code:
/librarian analyze

Ships with a Research Skill Graph example vault you can clone and start using in 5 minutes.

github.com/ngmeyer/librarian-mcp

**11/** The vault is now a living system. Every time I run /self-learn, it gets smarter. Knowledge compounds across projects. Open questions from one research become the starting point for the next.

This is what a "second brain" should actually be.
