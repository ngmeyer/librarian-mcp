---
name: librarian
description: Orchestrate vault knowledge workflows using the Librarian MCP server. Ingest solutions, search prior art, explore the knowledge graph, log daily learnings.
argument-hint: "<command> [args]"
---

# /librarian

Orchestrate high-level knowledge workflows using Librarian MCP tools.

## Prerequisites

Librarian MCP server must be configured and running. Verify by checking that `library_stats` responds. If it fails, tell the user to run `librarian-mcp --setup /path/to/vault` and restart Claude.

## Commands

```
/librarian ingest [path]           Ingest a solution doc into the vault
/librarian import <file>           Convert local document (PDF, DOCX, image) to vault markdown
/librarian from gmail <query>      Import Gmail threads matching a search into the vault
/librarian from web <url>          Import any web page into the vault as markdown
/librarian from twitter <url>      Import an X/Twitter thread into the vault
/librarian from calendar [query]   Import calendar events/meeting notes into the vault
/librarian from claude [session]   Extract learnings from a Claude Code session into the vault
/librarian search <query>          Deep search vault for prior art
/librarian connect [path]          Find and apply missing wikilinks
/librarian daily [text]            Append a learning to today's daily note
/librarian graph [note]            Explore vault structure or a note's neighborhood
/librarian analyze                 Full vault analysis: god nodes, communities, viz, report
/librarian daydream [focus]        Discover non-obvious connections across vault notes
/librarian status                  Vault health overview
/librarian project init <name>     Initialize a project decision & state page
/librarian project log <project>   Append an experiment-outcome entry to a project page
/librarian propose <project>       Draft ranked, cited candidate next moves for a project
```

If no command is given, show the usage summary above.

---

## ingest

**Purpose:** Import a `docs/solutions/` file (or any local markdown) into the vault, gaining auto-wikilinks and knowledge graph integration. This is the bridge between per-project documentation (like ce:compound output) and the persistent vault.

### Behavior

1. **Locate the source file:**
   - If `[path]` is provided, use it directly
   - If no path, scan `docs/solutions/` for the most recently modified `.md` file
   - If `docs/solutions/` doesn't exist or is empty, tell the user and stop

2. **Read the source file** using the Read tool (local filesystem)

3. **Determine vault destination:**
   - Parse the source file's YAML frontmatter for `category` (or infer from parent directory name)
   - Map to vault path: `Solutions/<category>/<filename>.md`
   - Categories map directly: `build-errors/` -> `Solutions/build-errors/`, etc.

4. **Write to vault** using `library_write` with the file content
   - `library_write` auto-links mentions of existing vault notes as `[[wikilinks]]`
   - Report what links were auto-added

5. **Run `library_suggest_links`** on the newly written file
   - Report any additional link suggestions found
   - If suggestions exist, ask user if they want to apply them (re-write with links)

6. **Report:**
   ```
   Ingested: docs/solutions/build-errors/vite-hmr-timeout.md
        -> Solutions/build-errors/vite-hmr-timeout.md

   Auto-linked: [[Vite]], [[HMR]], [[SvelteKit]]
   Suggestions: 2 additional links available

   The solution is now searchable and connected to your knowledge graph.
   ```

### Batch mode

If the user says `/librarian ingest all` or `/librarian ingest docs/solutions/`:
- Find all `.md` files in `docs/solutions/` recursively
- Ingest each one, reporting progress
- Summarize: total ingested, total links added, any failures

---

## import

**Purpose:** Convert any non-markdown document into markdown and store it in the vault. Uses Microsoft's MarkItDown for conversion, then auto-wikilinks the content.

### Supported formats

PDF, DOCX, XLSX, PPTX, images (OCR), audio (transcription), HTML, CSV, JSON, XML, ZIP — anything MarkItDown supports.

### Prerequisites

MarkItDown must be installed: `pip install markitdown`

If the user hasn't installed it, tell them and stop. Don't try to work around it.

### Behavior

1. **Determine source and destination:**
   - `<file>` is the local filesystem path to the document
   - If the user provides a vault destination path, use it
   - Otherwise, infer from the file: `Imports/<filename-stem>.md`

2. **Call `library_import`** with:
   - `source_path`: the local file path
   - `library_path`: the vault destination
   - `title`: filename stem or user-provided title

3. **Report:**
   ```
   Imported: quarterly-report.pdf
        -> Imports/quarterly-report.md (12,847 bytes, 2,103 words)

   Auto-linked: [[Q1 Revenue]], [[Product Roadmap]]
   ```

### Batch mode

If the user provides a directory path:
- List all non-markdown files in the directory
- Confirm with the user before proceeding
- Import each file, reporting progress
- Summarize: total imported, total words, failures

---

## from

**Purpose:** Import content from external connectors (Gmail, web, Twitter/X, Google Calendar) into the vault. Each connector extracts content, converts it to a structured markdown note with frontmatter, writes it via `library_write` for auto-wikilinks, and integrates it into the knowledge graph.

All `from` subcommands follow the same output pattern:
- YAML frontmatter with `source`, `imported`, `type`, and `tags`
- Structured markdown body
- Key takeaways or summary section
- Written via `library_write` for auto-wikilinks

### from gmail

**Purpose:** Import Gmail threads into the vault as structured notes. Useful for preserving important email conversations, research threads, or decisions.

**Connector:** Claude.ai Gmail MCP (`search_threads`, `get_thread`)

**Behavior:**

1. **Search for threads:**
   - Call `search_threads` with the user's query (same syntax as Gmail search bar)
   - Examples: `"from:editor@publisher.com"`, `"subject:manuscript feedback"`, `"has:attachment newer_than:7d"`
   - Display results (subject, from, date, snippet) and let user pick which to import
   - If only one result, proceed directly

2. **Extract full thread:**
   - Call `get_thread` with `messageFormat: "FULL_CONTENT"` for each selected thread
   - Extract: participants, dates, subject, full message bodies

3. **Build the vault note:**
   ```markdown
   ---
   title: "<subject line>"
   participants: ["sender@email.com", "recipient@email.com"]
   source: gmail
   thread_id: "<thread_id>"
   imported: YYYY-MM-DD
   type: email
   tags:
     - source/gmail
     - <topic tags inferred from content>
   ---

   # <subject line>

   **Participants:** sender, recipient | **Date range:** YYYY-MM-DD to YYYY-MM-DD

   ---

   ## Message 1 — From: sender@email.com (YYYY-MM-DD HH:MM)

   <message body>

   ## Message 2 — From: recipient@email.com (YYYY-MM-DD HH:MM)

   <message body>

   ---

   ## Key points

   <3-5 bullet point summary of the thread's decisions, action items, or key information>
   ```

4. **Vault path:** `Emails/<YYYY>/<slugified-subject>.md`

5. **Write via `library_write`**, report auto-linked notes.

6. **Report:**
   ```
   Imported: "Re: Manuscript feedback round 2" (5 messages)
        -> Emails/2026/manuscript-feedback-round-2.md

   Auto-linked: [[Publishing]], [[Draft Review]]
   Key points: 3 action items extracted
   ```

### from web

**Purpose:** Import any web page into the vault as clean markdown. Uses Exa or Tavily for extraction.

**Connectors:** Exa (`web_fetch_exa`) preferred, Tavily (`tavily_extract`) as fallback.

**Behavior:**

1. **Extract the page:**
   - Try `web_fetch_exa` with the URL first (returns clean markdown)
   - If Exa fails or returns insufficient content, fall back to `tavily_extract` with `extract_depth: "advanced"`
   - If both fail, tell the user and stop

2. **Build the vault note:**
   ```markdown
   ---
   title: "<page title>"
   url: "<source URL>"
   author: "<author if detectable>"
   imported: YYYY-MM-DD
   type: article
   tags:
     - source/web
     - <topic tags inferred from content>
   ---

   # <page title>

   **Source:** [<domain>](<url>) | **Imported:** YYYY-MM-DD

   ---

   <cleaned markdown content>

   ---

   ## Key takeaways

   <3-5 bullet point summary>
   ```

3. **Vault path:** `Web/<YYYY>/<slugified-title>.md`

4. **Write via `library_write`**, report auto-linked notes.

5. **Batch mode:** If given multiple URLs, process each sequentially.

### from twitter

**Purpose:** Import an X/Twitter thread into the vault.

**Connector:** `bird` CLI via Bash (from agent-reach tooling).

**Behavior:**

1. **Extract the thread:**
   - Run: `bird thread <url>` via Bash
   - If URL is a single tweet (not a thread), run: `bird read <url>`
   - If `bird` fails, fall back: try `tavily_extract` on the URL, or ask user to paste text

2. **Parse the output:**
   - Extract: author handle, date, individual tweets
   - Identify the thread topic from the first tweet

3. **Build the vault note:**
   ```markdown
   ---
   title: "<thread topic summary>"
   author: "@handle"
   source: "<original URL>"
   imported: YYYY-MM-DD
   type: thread
   tags:
     - source/twitter
     - <topic tags inferred from content>
   ---

   # <thread topic summary>

   **Author:** [@handle](<profile url>) | **Date:** YYYY-MM-DD | **[Original thread](<url>)**

   ---

   <thread content, preserving tweet boundaries with --- separators>

   ---

   ## Key takeaways

   <3-5 bullet point summary of the thread's main points>
   ```

4. **Vault path:** `Threads/<YYYY>/<slugified-topic>.md`

5. **Write via `library_write`**, report auto-linked notes.

6. **Batch mode:** If given multiple URLs, process each sequentially.

### from calendar

**Purpose:** Import calendar events into the vault, useful for meeting notes, event context, or archiving.

**Connector:** Claude.ai Google Calendar MCP (`gcal_list_events`, `gcal_get_event`)

**Behavior:**

1. **Find events:**
   - If `[query]` is provided, use it as a search term via `gcal_list_events` with `q` parameter
   - If no query, list today's events via `gcal_list_events` with `timeMin`/`timeMax` set to today
   - Display results and let user pick which to import

2. **Get full event details:**
   - Call `gcal_get_event` for each selected event
   - Extract: title, date/time, attendees, description, location, attachments

3. **Build the vault note:**
   ```markdown
   ---
   title: "<event summary>"
   date: YYYY-MM-DD
   attendees: ["person1@email.com", "person2@email.com"]
   source: google-calendar
   event_id: "<event_id>"
   imported: YYYY-MM-DD
   type: meeting
   tags:
     - source/calendar
     - <topic tags inferred from content>
   ---

   # <event summary>

   **Date:** YYYY-MM-DD HH:MM - HH:MM | **Location:** <location>
   **Attendees:** person1, person2

   ---

   ## Event description

   <description from the calendar event>

   ---

   ## Notes

   <placeholder for user to fill in meeting notes>
   ```

4. **Vault path:** `Meetings/<YYYY>/<YYYY-MM-DD>-<slugified-title>.md`

5. **Write via `library_write`**, report auto-linked notes.

6. **Batch mode:** If importing a date range, ask user: `/librarian from calendar --week` imports all events from the current week.

### from claude

**Purpose:** Extract key learnings, decisions, and solutions from a Claude Code session transcript and archive them in the vault. Closes the knowledge loop — your conversations become searchable vault knowledge.

**Data source:** Local JSONL files at `~/.claude/projects/<project>/<session-id>.jsonl`

**Behavior:**

1. **Find the session:**
   - If `[session]` is a UUID, use it directly
   - If `[session]` is a project name or path fragment, find matching project directory under `~/.claude/projects/`
   - If no argument, list the 10 most recent sessions across all projects with date, size, project, and first user message as topic hint. Let the user pick.
   - To list sessions: use Bash to scan `~/.claude/projects/*/` for `.jsonl` files, sort by mtime descending

2. **Parse the JSONL transcript:**
   - Read the file using the Read tool (it's local plaintext)
   - Each line is a JSON object with a `type` field: `user`, `assistant`, `system`, `file-history-snapshot`, `permission-mode`, `attachment`
   - Focus on `user` and `assistant` messages:
     - `user` messages have `message.content` (string or array of `{type: "text", text: "..."}` blocks)
     - `assistant` messages have `message.content` as array of blocks: `thinking`, `text`, `tool_use`
     - Extract only `text` blocks from assistant messages (skip `thinking` and `tool_use`)
   - Skip `system`, `file-history-snapshot`, `permission-mode`, and `attachment` types

3. **Synthesize the session into a vault note:**
   - Read through the conversation and extract:
     - **Topic:** What was the session about? (infer from first user message and overall flow)
     - **Decisions made:** Architecture choices, approach selections, trade-offs resolved
     - **Problems solved:** Bugs fixed, issues resolved, with root causes
     - **Key artifacts:** Files created/modified, commands established
     - **Open items:** Things deferred, noted for future, or left incomplete
   - Be selective — not every message is worth archiving. Focus on knowledge that compounds.

4. **Build the vault note:**
   ```markdown
   ---
   title: "<session topic summary>"
   project: "<project path>"
   session_id: "<uuid>"
   date: YYYY-MM-DD
   source: claude-code
   imported: YYYY-MM-DD
   type: session
   tags:
     - source/claude-code
     - <project tag>
     - <topic tags inferred from content>
   ---

   # <session topic summary>

   **Project:** <project path> | **Date:** YYYY-MM-DD | **Messages:** N

   ---

   ## Decisions

   - <decision 1 — what was chosen and why>
   - <decision 2>

   ## Solutions

   ### <problem 1 title>
   **Problem:** <what was wrong>
   **Root cause:** <why>
   **Fix:** <what was done>

   ## Artifacts

   - Created: `path/to/file.md`
   - Modified: `src/main.rs` — added skill installation to --setup

   ## Open items

   - <anything deferred or left for future sessions>
   ```

5. **Vault path:** `Sessions/<YYYY>/<YYYY-MM-DD>-<slugified-topic>.md`

6. **Write via `library_write`**, report auto-linked notes.

7. **Report:**
   ```
   Imported session: "Librarian skill + MCP connector integration"
        -> Sessions/2026/2026-04-11-librarian-skill-mcp-connectors.md

   Auto-linked: [[librarian-mcp]], [[ce-compound]], [[Obsidian]]
   Extracted: 4 decisions, 2 solutions, 1 open item
   ```

### Tips for from claude

- **Current session:** You can import the current session — just use the session ID from the JSONL filename. The transcript is written incrementally, so it includes everything up to now.
- **Pairs with session-close:** Run `/session-close` to update project memory, then `/librarian from claude` to archive the knowledge in the vault.
- **Large sessions:** For sessions with 100+ messages, focus extraction on the second half where solutions typically land. Skip early exploration/dead-ends unless they contain useful "what didn't work" context.
- **Privacy:** Session transcripts may contain API keys, passwords, or sensitive data that was read by tools. The synthesis step should never copy raw credentials — only extract the conceptual knowledge.

---

## search

**Purpose:** Deep search across the vault for prior art on a topic. More than a raw search -- synthesizes results into actionable context.

### Behavior

1. **Call `library_search`** with the query (limit: 10)

2. **For the top 3-5 results**, call `library_read` to get full content

3. **Synthesize** a brief summary:
   - What the vault knows about this topic
   - Which notes are most relevant (with paths)
   - Key insights or solutions found
   - Related tags discovered

4. **Call `library_traverse`** from the most relevant result (depth: 1) to find connected notes the search might have missed

5. **Report:**
   ```
   Vault knowledge on "<query>":

   Direct matches (N files):
   - Solutions/build-errors/vite-hmr-timeout.md — HMR fix for Tauri apps
   - Research/Deep Dives/vite-internals.md — Vite architecture notes

   Key insights:
   - [synthesized takeaways from the matched content]

   Connected notes (via graph):
   - [[SvelteKit]] (1 hop from vite-hmr-timeout)

   Tags: #build, #vite, #frontend
   ```

---

## connect

**Purpose:** Find unlinked mentions across the vault and optionally apply them. Strengthens the knowledge graph.

### Behavior

1. **If `[path]` is provided:**
   - Run `library_suggest_links` on that specific file
   - Show suggestions
   - Ask user to confirm before applying

2. **If no path (vault-wide scan):**
   - Call `library_stats` to get orphan notes
   - For each orphan (up to 10), run `library_suggest_links`
   - Report which orphans could be connected and how
   - Ask user before applying any changes

3. **Applying links:**
   - Read the file via `library_read`
   - Write it back via `library_write` (which auto-links on write)
   - Report the links that were added

---

## daily

**Purpose:** Append a learning, note, or reflection to today's daily note.

### Behavior

1. **If `[text]` is provided:**
   - Call `library_daily` with `append: <text>`
   - Done

2. **If no text:**
   - Summarize the current conversation's key outcomes (what was solved, what was learned, what decisions were made)
   - Format as a bullet list under a `## Learnings` section
   - Call `library_daily` with the formatted summary
   - Show what was appended

3. **Always report** the daily note path (e.g., `Journal/2026/2026-04-11.md`)

---

## graph

**Purpose:** Explore vault structure or a specific note's neighborhood in the knowledge graph.

### Behavior

1. **If `[note]` is provided:**
   - Call `library_traverse` with `start: <note>`, `depth: 2`
   - Call `library_links` on the note for backlinks/outgoing detail
   - Render a text-based neighborhood map:
     ```
     Topic Neighborhood: "Vite"

        [[SvelteKit]] ---> [[Vite]] <--- [[HMR]]
                             |
                             v
                        [[Tauri Build]]

     Backlinks (3): SvelteKit, HMR, Frontend Tooling
     Outgoing (2): Tauri Build, ESBuild
     2 hops: 8 notes reachable
     ```

2. **If no note (vault-wide):**
   - Call `library_stats` for overview
   - Call `library_graph_analysis` for structure
   - Report: file count, word count, connected components, hub notes, bridge notes, orphan count
   - Highlight the top 5 hub notes (most connected)
   - Flag orphans that might need linking

---

## analyze

**Purpose:** Run full vault intelligence analysis — community detection, structural importance ranking (god nodes), cross-community bridge detection, and interactive visualization. Generates two artifacts in the vault root.

### Behavior

1. **Call `library_report`** (no params, uses default output path)
   - Runs the full pipeline: graph build → community detection → betweenness centrality → PageRank → surprising connections
   - Writes `GRAPH_REPORT.md` to vault root
   - Report includes: god nodes table, community breakdown, surprising cross-topic connections, suggested questions

2. **Call `library_visualize`** (no params, uses default output path)
   - Generates self-contained interactive HTML with force-directed graph layout
   - Nodes colored by community, sized by structural importance
   - Click to inspect, search to filter
   - Writes `GRAPH_VIZ.html` to vault root

3. **Report:**
   ```
   Vault analysis complete.

   Report: GRAPH_REPORT.md
   - 847 notes, 2,341 links, 12 communities, 43 orphans
   - God nodes: [[Claude]], [[Obsidian]], [[Knowledge Management]]
   - 8 surprising cross-community connections found

   Visualization: GRAPH_VIZ.html
   - Open in browser to explore the interactive graph

   Next steps:
   - /librarian connect — link orphan notes into the graph
   - /librarian from claude — archive this session's learnings
   ```

### What the report contains

| Section | Content |
|---------|---------|
| **God Nodes** | Top 10 structurally important notes ranked by composite score (degree + betweenness centrality + PageRank) |
| **Communities** | Topic clusters with member lists, detected by modularity optimization |
| **Surprising Connections** | High-betweenness edges that bridge different communities — the cross-topic links |
| **Suggested Questions** | 5 questions the graph is uniquely positioned to answer |

---

## daydream

**Purpose:** Discover non-obvious connections between vault notes using multi-agent combinatorial exploration. Inspired by Gwern's LLM Daydreaming essay and glebis's Daydream skill — implements the brain's "default mode network" for your vault.

### How it works

The command pairs random notes and asks parallel sub-agents to find surprising connections, then filters with critics. Only genuinely novel, coherent, and useful insights survive.

### Behavior

1. **Sample notes from the vault:**
   - Call `library_list` to get all note paths
   - Call `library_read` on a random sample of 50 notes
   - Weight toward recent notes (by date frontmatter or filename)
   - If `[focus]` provided: use `library_search` to find focus-relevant notes, pair them with random notes from outside that cluster

2. **Generate random pairs:**
   - Create 50 unique note pairs from the sample
   - Check `Daydreams/history.json` (via `library_read`) for already-processed pairs
   - Skip duplicates, proceed with remaining pairs

3. **Synthesize connections (parallel sub-agents):**
   - Launch 10 parallel sub-agents (Sonnet model), each processing 5 pairs
   - Each agent reads both notes and explores:
     - Abstract analogies between concepts
     - Similar problems or solutions in different domains
     - Potential combinations or hybrid ideas
     - Revealing contradictions or tensions
   - Each agent returns structured results: title, connection description, source notes, reasoning

4. **Critique and filter (parallel sub-agents):**
   - Launch 10 parallel critic sub-agents (Haiku model)
   - Each critic scores connections on three dimensions (1-10 each):
     - **Novelty:** Is this surprising and non-obvious?
     - **Coherence:** Is the reasoning logical and well-grounded?
     - **Usefulness:** Could this lead to new work, insights, or decisions?
   - Accept only connections with average score >= 7.0

5. **Write accepted insights to vault:**
   - For each accepted insight, call `library_write` to create `Daydreams/<YYYYMMDD>-<slug>.md`
   - Frontmatter includes: title, source_notes (paths), scores (novelty/coherence/usefulness), date, tags (daydream, source topics)
   - Body includes: the connection description, reasoning, and source note excerpts
   - `library_write` auto-wikilinks the insight into the knowledge graph

6. **Update history:**
   - Read or create `Daydreams/history.json` via `library_read`/`library_write`
   - Add all processed pairs (accepted or rejected) to prevent re-processing
   - Track: pair hashes, dates, accept/reject status

7. **Report:**
   ```
   Daydream complete.

   Processed: 42 pairs (8 skipped as duplicates)
   Accepted: 7 insights (17% acceptance rate)

   Insights written:
   - Daydreams/20260411-authentication-as-trust-boundary.md
     "Authentication patterns in the auth module mirror trust boundary
      concepts from the game theory notes — both define threshold functions."
   - Daydreams/20260411-vite-hmr-and-neural-plasticity.md
     ...

   Run /librarian analyze to see how these integrate into the knowledge graph.
   ```

### Cost

Approximately $0.40-0.50 per run (50 pairs) using Sonnet for synthesis and Haiku for critique.

### Tips

- Run weekly or after adding substantial new content to the vault
- Use `[focus]` to explore connections around a specific topic: `/librarian daydream authentication`
- Daydream insights compound — they become seeds for future runs, creating a discovery flywheel
- Run `/librarian analyze` after daydream to see how insights integrate into the graph

---

## status

**Purpose:** Quick vault health check.

### Behavior

1. Call `library_stats`
2. Report in compact format:
   ```
   Vault: The Labyrinth
   Files: 1,247 | Words: 389,102 | Links: 3,891 | Tags: 156
   Orphans: 23 (run /librarian connect to fix)
   Solutions: 45 ingested
   Last daily: 2026-04-11
   ```

---

## projects

**Purpose:** The `project init`, `project log`, and `propose` subcommands maintain a per-project **decision & state page** — an outcome-anchored experiment log that retains judgment so it stops being re-derived at the start of every session. This section documents the page convention the three subcommands operate on.

### Page convention

**Default location:** `Projects/<project>.md` in the vault. Operators may relocate; the `type: decision-state` frontmatter marker is the canonical signal — location is convention, not constraint.

**Frontmatter schema:**

```yaml
---
type: decision-state
project: <project-name>
anchor_outcome: <metric>          # string or list, e.g., "Sharpe" or ["CTR", "conversion"]
research_scope:                    # at least one key; multiple allowed
  folders: [<path>, ...]           # vault folders to draw research from
  communities: [<label>, ...]      # Louvain community labels (see library_cluster)
  tags: [<tag>, ...]               # notes carrying these tags
  wikilinks: [<stem>, ...]         # explicit anchor notes (neighborhood expanded via library_traverse)
repo: <optional path or url>
status: active                     # active | archived
---
```

**Section layout** (operators author all sections except the auto-managed candidate-next-moves block):

```markdown
# <Project>

## Anchor outcomes
<current value, target — operator authors and updates>

## Experiment & outcome log
<append-only entries. Each entry: dated bold header, then bullet sub-items for what changed, expected effect, observed delta on the anchor, link to source plan/PR/research>

## Ratified decisions
<append-only entries — decisions an experiment outcome confirmed or killed>

## Candidate next moves (auto)
<MANAGED BY THE AGENT — written by `/librarian propose`. Operators never edit this block by hand; re-running propose overwrites it. Everything outside this block is byte-stable across propose runs.>
```

### Operator contract (load-bearing)

The whole loop depends on one habit: **logging the outcome delta when an experiment completes**. If deltas aren't logged, the page decays into a narrative log and the falsifiability advantage evaporates. `project log` exists specifically to make this cheap.

### Example page (Voltron)

```markdown
---
type: decision-state
project: Voltron
anchor_outcome: Sharpe
research_scope:
  communities: ["QuantFlow"]
  tags: ["voltron", "trading"]
repo: /Users/nealme/Projects/voltron
status: active
---

# Voltron

## Anchor outcomes
- **Sharpe:** current 1.24 (last measured 2026-05-15) · target ≥ 1.50

## Experiment & outcome log

**2026-04-20 — Added regime-slope gate (slope ≥ 0.003)**
- Expected: reduce chop-day false entries
- Observed: Sharpe 1.18 → 1.24 over 30 trading days
- Source: [[Voltron VWAP mean reversion achieves Sharpe 1.24 on 59-day backtest using 5K fixed bets and 3pct stops]]

**2026-05-01 — Tightened stop from 3% → 2.5%**
- Expected: reduce per-loss magnitude
- Observed: Sharpe 1.24 → 1.21; reverted 2026-05-05
- Source: backtest in repo

## Ratified decisions
- **Keep regime-slope gate** (ratified by 2026-04-20 experiment)
- **Keep 3% stop** (kill experiment 2026-05-01: tighter stop reduced Sharpe)

## Candidate next moves (auto)
<!-- The /librarian propose subcommand writes this section. -->
```

### Generic by design

No project name, anchor metric, folder path, or community label is baked into the subcommands themselves. Every vault-specific value comes from the operator's frontmatter. Substituting `SignUpSpark` + anchor `CTR` + scope `tags: ["gtm", "signupspark"]` works with no code change.

---

## project init

**Purpose:** Scaffold a new project decision & state page in the vault, capturing the operator's declared anchor outcome(s) and research scope so future `project log` and `propose` runs have the structure they need. See the `projects` section above for the convention.

### Behavior

1. **Parse the argument:**
   - Required: `<project-name>` — used as the file stem and the `project:` frontmatter value
   - Optional: `--path <relpath>` to override the default location

2. **Determine the page path:**
   - Default: `Projects/<project-name>.md` (create `Projects/` if it doesn't exist)
   - With `--path`, use the relative path as given

3. **Refuse to overwrite an existing page:**
   - Call `library_read` on the target path
   - If the file exists, report: "A page already exists at `<path>`. Edit it directly, or pass a different `<project-name>` / `--path`." and stop. Do NOT silently overwrite.

4. **Interactively gather frontmatter values from the operator:**
   - **Anchor outcome(s):** ask for one or more metric names (Sharpe, CTR, conversion, retention, etc.). Capture as a string (single) or list (multiple).
   - **Research scope:** ask which scope keys to populate. At least one of `folders`, `communities`, `tags`, `wikilinks` must be non-empty (an empty scope is a slip — re-prompt rather than write).
     - `folders` — vault folder paths the project draws research from
     - `communities` — Louvain community labels (run `library_cluster` if the operator wants to see candidates)
     - `tags` — tag names
     - `wikilinks` — explicit anchor notes
   - **Optional `repo`:** path or URL to the project's source repo
   - **Status:** default `active`

5. **Compose the page:**

   ```markdown
   ---
   type: decision-state
   project: <name>
   anchor_outcome: <value or list>
   research_scope:
     <populated keys>
   repo: <value or omit>
   status: active
   ---

   # <Project>

   ## Anchor outcomes
   <empty — operator fills current value and target>

   ## Experiment & outcome log
   <empty — append via /librarian project log>

   ## Ratified decisions
   <empty — operator authors directly or via project log ratification>
   ```

   Do **not** pre-create the `## Candidate next moves (auto)` block — `/librarian propose` writes it on first run.

6. **Write the page** via `library_write` (auto-link is fine here — the project name will pick up backlinks naturally; isolation is honored automatically).

7. **Report:**
   ```
   Initialized: Projects/<name>.md
   Anchor outcome(s): <values>
   Research scope: <summary>

   Next steps:
   - Fill in current value + target for the anchor outcome(s).
   - Log past experiments via `/librarian project log <name>` to backfill history.
   - Run `/librarian propose <name>` once history is seeded.
   ```

### Errors and edge cases

- Page already exists → report and stop (R14: don't silently invent or overwrite)
- Empty research scope → re-prompt (a project with no scope can't be proposed against meaningfully; better to ask again than write a useless page)
- Project name with characters unsafe for filenames → sanitize (replace `/`, `:`, leading dots), echo the sanitized name back, ask for confirmation

---

## project log

**Purpose:** Append a single experiment-outcome entry to a project's decision & state page. This is the load-bearing operator habit — every shipped experiment / change worth tracking against the anchor metric gets one log entry. The page accumulates a falsifiable record that compounds over time.

### Behavior

1. **Locate the project page:**
   - Resolve `Projects/<project>.md` (or accept `--path <relpath>`)
   - Call `library_read`; if the file doesn't exist, report: "No project page at `<path>`. Run `/librarian project init <project>` first." and stop.

2. **Validate structure:**
   - Confirm frontmatter parses (use `library_metadata` or parse inline) and `type: decision-state` is present
   - Confirm the page has an `## Experiment & outcome log` section
   - If structure is missing, report the missing piece and point at `project init` — do NOT silently invent sections (R14: degrade gracefully, don't invent)

3. **Interactively gather the entry fields:**
   - **What changed** — short description (one line; the entry header)
   - **Expected effect** — what the operator predicted before the change
   - **Observed delta** — the actual movement on the anchor metric (operator brings this from the source-of-truth: backtest, A/B platform, analytics, spreadsheet)
   - **Source link** — link to the plan, PR, research note, or document that motivated the change. Wikilinks (`[[note]]`) preferred for vault-internal sources
   - **Ratifies / kills a decision?** Optional — if the outcome confirms or rejects a prior call, ask for the decision text so it also lands in `## Ratified decisions`

4. **Compose the log entry:**

   ```markdown
   **YYYY-MM-DD — <what changed>**
   - Expected: <expected effect>
   - Observed: <delta on anchor>
   - Source: <link or wikilink>
   ```

5. **Append to the log section:**
   - Read the current page content
   - Locate the `## Experiment & outcome log` header
   - Identify the section's end (the next `\n## ` heading, or end of file)
   - Append the new entry at the end of the section (after existing entries, before the next H2)
   - Preserve existing entries byte-identical — append-only, never rewrite

6. **If the entry ratifies/kills a decision:**
   - Compose a matching one-line entry: `- **<decision>** — ratified by <date> entry: <link>` (or `killed by` for negative outcomes)
   - Append to the `## Ratified decisions` section the same way (locate header → insert at end → preserve prior entries)

7. **Write the updated page** via `library_write`. The `## Candidate next moves (auto)` block, if present, must remain byte-identical — only the log (and possibly Ratified decisions) sections change.

8. **Report:**
   ```
   Logged to Projects/<project>.md
   Entry: <date> — <what changed>
   Anchor delta: <observed>
   Ratified: <decision or "none">
   ```

### Errors and edge cases

- Page missing → report and point at `project init`
- `## Experiment & outcome log` section missing → report and stop; suggest re-running `project init` or adding the section manually
- Observed delta is empty → prompt again; the load-bearing value of the log is the delta. If the operator genuinely doesn't have a measurement yet (still running), record the entry with `Observed: pending` and remind them to update it
- The `## Candidate next moves (auto)` block exists → verify it remains byte-identical after the write (sanity check; should hold because the log section is independent)

---

## propose

**Purpose:** Draft a ranked, evidence-cited list of candidate next moves for a project, written into a fenced managed block (`## Candidate next moves (auto)`) on the project's decision & state page. This is the **steering** half of the Judgment layer: the operator brings the anchor and the scope; the agent reads accumulated research + outcome history and proposes what to try next, grounded in citations rather than instinct.

The subcommand runs on demand. There is no daemon and no automatic refresh — the operator invokes propose at a steering moment, reviews the candidates, picks one (or none), and the loop closes when they log the next outcome.

### Behavior

1. **Parse the argument:**
   - Required: `<project>` — resolves to `Projects/<project>.md` (or accept `--path <relpath>`)
   - Optional: `--count N` for number of candidates (default 5; clamp to 3–10)

2. **Read the project page:**
   - `library_read` the path; report a clear error and stop if missing
   - Parse frontmatter (`library_metadata` is convenient). Required fields: `anchor_outcome`, `research_scope`. If either is missing or `research_scope` has no populated keys, report what's missing and point at `project init`. Do NOT proceed with an empty scope (R14: degrade gracefully but don't fabricate)
   - Also parse the page body to extract:
     - Prior `## Experiment & outcome log` entries (informs ranking — what has and hasn't moved the metric)
     - Prior `## Ratified decisions` (avoid re-proposing what's already decided)

3. **Compose the research corpus** from declared scope. Use existing librarian primitives — no new tools required:
   - For each `folders` entry: `library_list` then `library_read` per `.md` file under that folder
   - For each `communities` entry: `library_cluster` returns community membership; gather all member stems whose community label matches, then `library_read` each. (Note: `library_cluster` returns stems; resolve to relative paths via `library_metadata` or by querying the cache via `library_links` if needed.)
   - For each `tags` entry: `library_tags` returns notes carrying the tag; `library_read` each
   - For each `wikilinks` entry: `library_traverse` depth 1 from the named stem; `library_read` neighbors
   - Union the results, dedupe by relative path
   - **Cap aggressive corpora at ~80 notes total.** If the union exceeds that, prefer notes more recent (modification time via `library_changes` if available, otherwise the cache's mtime), and notes the operator's prior log entries already reference. Note the cap in the output so the operator knows the corpus was truncated.

4. **Apply the isolation filter:**
   - Read `.librarianisolate` at the vault root (one folder name per line, comments and blanks ignored)
   - For each candidate corpus note, derive its top-level folder (the first path component). If that folder appears in `.librarianisolate`, drop the note from the corpus. This honors `.librarianisolate` end-to-end: the agent never cites a note from an isolated folder.
   - If after isolation the corpus is empty, report "scope produced no citable notes after isolation filter" and stop (do not write an empty managed block).

5. **Read the corpus content and rank candidate next moves:**
   - For each candidate, draw on:
     - **Prior outcome history on this project** (what has moved the anchor, what hasn't; what's already been tried and killed)
     - **Research evidence strength** (multiple sources agreeing, recency, methodological rigor noted in the source, whether the source is itself a synthesis/digest or a raw thread)
     - **Confidence level** — explicit `high | medium | low` based on the above
   - Generate `<count>` candidates (default 5). Each candidate carries:
     - **A concrete proposed move** (action, not aspiration — "Add a Markov persistence gate at τ=0.85" not "Improve regime detection")
     - **Expected effect on the anchor** (qualitative or quantitative; reference the metric by name from frontmatter)
     - **Confidence:** `high | medium | low`
     - **Citations** — at least one, ideally 2–3, as wikilinks (`[[note title]]`) or relative paths to specific corpus notes that motivated the candidate
     - **Rationale** — one or two sentences tying the citations to the proposed move
   - Rank candidates with the highest expected-impact / highest-confidence first. Break ties with confidence, then recency of supporting evidence.

6. **Construct the managed-block content:**

   ```markdown
   ## Candidate next moves (auto)

   _Generated YYYY-MM-DD by `/librarian propose`. Corpus: N notes scoped from <summary>. Re-running this command overwrites only this section._

   ### 1. <Proposed move> · confidence: <level>

   **Expected:** <effect on anchor>

   **Why:** <rationale tying citations to the move>

   **Citations:** [[note A]], [[note B]], [[note C]]

   ### 2. <Proposed move> · confidence: <level>

   ...
   ```

7. **Upsert the block into the page** (mirrors the `## Related (auto)` upsert pattern from v0.1.2's `library_optimize`):
   - Read the current page content
   - Search for the marker `## Candidate next moves (auto)`
     - **If found:** replace from the marker through the start of the next `\n## ` heading (or end of file). Everything before and after the block is byte-identical.
     - **If not found:** append the block to the end of the page (preceded by a blank line and `---` separator if the prior section doesn't end with one).
   - Write the updated page via `library_write`

8. **Report a short summary:**
   ```
   Wrote N candidates to Projects/<project>.md
   Corpus: <count> notes (after isolation filter)
   Top candidate: <name> · confidence: <level>
   ```

### Sparse-history fallback (R14)

If the page has no prior `## Experiment & outcome log` entries:
- Proceed using research evidence alone
- Cap confidence at `medium` for every candidate (no project-specific outcome history exists to grant `high` confidence)
- Note this in the managed block's header line: `_No prior outcome history — candidates drawn from research evidence only._`

### Errors and edge cases

- Page missing → report and point at `project init`
- Frontmatter missing required fields → report which fields are missing
- Empty research scope after parsing → point at `project init` to fill the scope
- Empty corpus after isolation → report and stop without writing
- Operator prose accidentally inside the managed-block range (e.g., a prior manual edit) → the upsert preserves only the boundary markers; warn the operator that hand-edits inside the block are overwritten. The block is agent-owned by contract.

### Loop closure

After the operator reviews the block and chooses a move:
1. They make the change in the project (outside this skill's scope — the work lives in the repo).
2. When the outcome is measurable, they run `/librarian project log <project>` to record the delta.
3. On the next `/librarian propose <project>` run, the new log entry is read in step 2 and informs the next ranking.

That closes the loop: candidate → action → outcome → updated history → better next candidate.

---

## Tool Reference

This skill orchestrates these Librarian MCP tools:

| Tool | Used by |
|------|---------|
| `library_search` | search, daydream (focus mode), propose (when tag/keyword scoping) |
| `library_read` | search, ingest, connect, daydream, project log, propose |
| `library_write` | ingest, from *, connect, daydream, project init, project log, propose |
| `library_list` | ingest (scan docs/solutions/), import (batch), daydream, propose (folder scope) |
| `library_links` | graph |
| `library_tags` | search, propose (tag scope) |
| `library_metadata` | ingest (read frontmatter), project init, project log, propose |
| `library_daily` | daily |
| `library_stats` | status, graph |
| `library_suggest_links` | ingest, connect |
| `library_traverse` | search, graph, propose (wikilink scope) |
| `library_shortest_path` | graph (on request) |
| `library_graph_analysis` | graph (vault-wide) |
| `library_cluster` | analyze, graph, propose (community scope) |
| `library_visualize` | analyze |
| `library_report` | analyze |
| `library_import` | import (MarkItDown conversion) |

### Claude.ai MCP connectors

| Connector | Tools used | Used by |
|-----------|-----------|---------|
| Gmail | `search_threads`, `get_thread` | from gmail |
| Exa | `web_fetch_exa` | from web |
| Tavily | `tavily_extract` | from web (fallback), from twitter (fallback) |
| Google Calendar | `gcal_list_events`, `gcal_get_event` | from calendar |

### Local data sources

| Source | Location | Used by |
|--------|----------|---------|
| Claude Code sessions | `~/.claude/projects/<project>/<session>.jsonl` | from claude |

### External CLI tools

| Tool | Used by | Install |
|------|---------|---------|
| `markitdown` | import | `pip install markitdown` |
| `bird` | from twitter | Via agent-reach / bird CLI |

## Integration with ce:compound

The primary integration pattern:

```
/ce:compound              # Documents a solution in docs/solutions/
/librarian ingest         # Imports latest solution into vault (auto-wikilinked)
```

This bridges project-scoped documentation into the persistent, cross-project knowledge graph. The next time you `/librarian search` for a similar problem, the solution surfaces with full graph context.
