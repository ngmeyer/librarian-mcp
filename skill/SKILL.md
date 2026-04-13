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

## Tool Reference

This skill orchestrates these Librarian MCP tools:

| Tool | Used by |
|------|---------|
| `library_search` | search, daydream (focus mode) |
| `library_read` | search, ingest, connect, daydream |
| `library_write` | ingest, from *, connect, daydream |
| `library_list` | ingest (scan docs/solutions/), import (batch), daydream |
| `library_links` | graph |
| `library_tags` | search |
| `library_metadata` | ingest (read frontmatter) |
| `library_daily` | daily |
| `library_stats` | status, graph |
| `library_suggest_links` | ingest, connect |
| `library_traverse` | search, graph |
| `library_shortest_path` | graph (on request) |
| `library_graph_analysis` | graph (vault-wide) |
| `library_cluster` | analyze, graph |
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
