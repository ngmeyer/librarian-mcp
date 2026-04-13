# Librarian

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Claude Code](https://img.shields.io/badge/Claude_Code-Skill-blueviolet?logo=anthropic)](https://claude.ai/code)
[![MCP](https://img.shields.io/badge/MCP-Server-orange?logo=data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSIyNCIgaGVpZ2h0PSIyNCIgdmlld0JveD0iMCAwIDI0IDI0IiBmaWxsPSJ3aGl0ZSI+PHBhdGggZD0iTTEyIDJMMyA3djEwbDkgNSA5LTV2LTEweiIvPjwvc3ZnPg==)](https://modelcontextprotocol.io)
[![Homebrew](https://img.shields.io/badge/Homebrew-Available-FBB040?logo=homebrew&logoColor=white)](https://github.com/ngmeyer/homebrew-tap)
[![Platform](https://img.shields.io/badge/Platform-macOS%20%7C%20Linux%20%7C%20Windows-lightgrey)]()

Give Claude a librarian for your markdown vault.

Librarian is an MCP server that connects [Claude](https://claude.ai) to your [Obsidian](https://obsidian.md) vault or any folder of markdown files. Search your notes, auto-link mentions as wikilinks, explore backlinks and tags, detect topic communities, and visualize your knowledge graph — all from inside Claude.

> **Runs entirely locally.** Your vault data never leaves your machine. Librarian reads and writes files on disk and communicates with Claude over stdio. No network calls, no telemetry, no cloud storage.

## Quick Start

```bash
# 1. Install
brew install ngmeyer/tap/librarian-mcp

# 2. Point at your vault and auto-configure Claude
librarian-mcp --setup ~/my-vault

# 3. Restart Claude, then try:
#    /librarian analyze     — see your vault's knowledge graph
#    /librarian search      — find anything across all notes
#    /librarian status      — vault health overview
```

`--setup` writes the MCP server config into Claude Desktop and Claude Code (with backups), and installs the `/librarian` skill so you get 12 slash commands out of the box.

## Install

### Homebrew (macOS)

```bash
brew install ngmeyer/tap/librarian-mcp
```

### Pre-built binaries

Download from [GitHub Releases](https://github.com/ngmeyer/librarian-mcp/releases) for macOS (arm64/x86_64), Linux (arm64/x86_64), and Windows. Place the binary on your PATH.

### Build from source (requires Rust)

```bash
cargo install --git https://github.com/ngmeyer/librarian-mcp
```

## How It Works

Librarian is a **standalone binary** — it works directly on your markdown files. Obsidian does not need to be running (and they can run simultaneously without conflict). Claude connects via the [Model Context Protocol](https://modelcontextprotocol.io) over stdio.

### How is this different from...

| | Read files directly | Obsidian Copilot | mcp-obsidian | **Librarian** |
|---|---|---|---|---|
| Trigram-indexed search | No | Plugin | Plugin | **Yes** |
| Auto-wikilinks on write | No | No | No | **Yes** |
| Knowledge graph traversal | No | No | Partial | **Yes (BFS, shortest path)** |
| Community detection | No | No | No | **Yes (Louvain)** |
| Interactive graph viz | No | No | No | **Yes (D3.js)** |
| Works without Obsidian | Yes | No | No | **Yes** |
| Works in Claude Code | Yes (manual) | No | No | **Yes** |
| Standalone binary | N/A | No | No | **Yes** |

## Setup

```bash
librarian-mcp --setup ~/my-vault
```

This auto-configures Claude Desktop and Claude Code (with backups of existing configs). Restart Claude to connect.

### Multiple vaults

```bash
librarian-mcp --setup ~/vaults/notes ~/vaults/research
```

### Manual configuration

Add to your Claude Desktop config (`~/Library/Application Support/Claude/claude_desktop_config.json`) or Claude Code config (`~/.claude/settings.json`):

```json
{
  "mcpServers": {
    "librarian": {
      "command": "librarian-mcp",
      "args": ["/path/to/your/vault"]
    }
  }
}
```

### Environment variables

```bash
LIBRARIAN_VAULT=/path/to/vault        # Single vault
LIBRARIAN_VAULTS=/vault/one:/vault/two # Multiple vaults (colon-separated, Unix only)
```

## Tools

Librarian exposes 17 tools to Claude:

| Tool | Description |
|------|-------------|
| `library_search` | Full-text search across all vault files (trigram-indexed) |
| `library_read` | Read a file by relative path |
| `library_write` | Write a file with auto-wikilink detection |
| `library_list` | List files and directories (shows vault roots when multi-vault) |
| `library_links` | Get backlinks and outgoing wikilinks for a file |
| `library_tags` | List all #tags with counts, optionally filtered by prefix |
| `library_metadata` | Read YAML frontmatter from a file |
| `library_daily` | Create or append to a daily note (Journal/YYYY/YYYY-MM-DD.md) |
| `library_stats` | Vault statistics: file count, word count, link density, orphan notes |
| `library_suggest_links` | Find unlinked mentions of existing notes (including aliases) |
| `library_traverse` | BFS traversal from a note — explore the topic neighborhood N hops deep |
| `library_shortest_path` | Find the shortest link chain between two notes |
| `library_graph_analysis` | Connected components, hub notes, bridges, orphans |
| `library_import` | Convert PDF, DOCX, images, etc. to markdown (requires [MarkItDown](https://github.com/microsoft/markitdown)) |
| `library_cluster` | Detect topic communities using Louvain modularity optimization |
| `library_visualize` | Generate interactive HTML graph visualization (D3.js) |
| `library_report` | Comprehensive vault analysis: god nodes, communities, bridges |

## The /librarian Skill

`--setup` installs a Claude Code skill that gives you 12 high-level commands built on the tools above:

| Command | What it does |
|---------|-------------|
| `/librarian search <query>` | Deep search with synthesis across top results |
| `/librarian analyze` | Full vault analysis — communities, hubs, visualization |
| `/librarian graph [note]` | Explore a note's neighborhood or vault-wide structure |
| `/librarian connect [path]` | Find and apply missing wikilinks |
| `/librarian daily [text]` | Append to today's daily note |
| `/librarian status` | Quick vault health check |
| `/librarian ingest [path]` | Import a local markdown file into the vault |
| `/librarian import <file>` | Convert PDF/DOCX/images to vault markdown |
| `/librarian from web <url>` | Import a web page as a vault note |
| `/librarian from twitter <url>` | Import an X/Twitter thread |
| `/librarian from gmail <query>` | Import Gmail threads |
| `/librarian from calendar` | Import calendar events |

## Example: Research Skill Graph

The repo includes a 16-file example vault demonstrating multi-lens research analysis — 6 analytical lenses that force different perspectives on any topic.

To try it after cloning:

```bash
git clone https://github.com/ngmeyer/librarian-mcp
cd librarian-mcp
librarian-mcp --setup examples/research-skill-graph
```

Then in Claude Code: "Follow the execution instructions in index.md. Research: Why are prediction market edges compressing?"

The vault includes source evaluation (5-tier trust system), contradiction protocol, synthesis rules, and a knowledge layer that compounds across projects. See [`examples/research-skill-graph/index.md`](examples/research-skill-graph/index.md) for the full system.

## Features

### Auto-linking

When Claude writes files via `library_write`, Librarian scans for mentions of existing note titles and wraps them in `[[wikilinks]]`. This happens **only on explicit writes** — Librarian never modifies files you didn't ask it to write.

Links use canonical file names so they resolve correctly in Obsidian's graph view, even on case-sensitive filesystems. Frontmatter aliases are supported: if a note has `aliases: [ML, machine learning]`, mentions will auto-link using `[[Note Name|ML]]` format.

Auto-linking skips code blocks, inline code, URLs, and existing wikilinks to avoid corrupting content.

### Knowledge graph traversal

Librarian builds a bidirectional graph from your vault's `[[wikilinks]]` and exposes three graph tools:

- **Traverse** — BFS from any note, N hops deep. "Show me everything connected to this topic."
- **Shortest path** — Find the link chain between two notes. "How are these ideas connected?"
- **Graph analysis** — Connected components, hub notes, bridge notes, orphans. "What's the structure of my vault?"

### Community detection

Louvain modularity optimization identifies topic clusters in your vault. Combined with betweenness centrality and PageRank, the report tool ranks "god nodes" (structurally important notes) and finds surprising cross-community connections.

### Search

Search is backed by an in-memory trigram index built on startup. Handles vaults with 10,000+ files.

### Exclusion patterns

By default, Librarian skips `.obsidian/`, `.trash/`, `.git/`, and `node_modules/`. Drop a `.librarianignore` file (gitignore syntax) in your vault root to customize:

```gitignore
# .librarianignore
templates/
private/
```

## Obsidian compatibility

Librarian works seamlessly alongside Obsidian — both can be open at the same time:

- Reads and writes standard `[[wikilinks]]` and `[[note|display text]]`
- Respects YAML frontmatter and `aliases`
- Skips `.obsidian/` and `.trash/` directories
- Auto-linked wikilinks resolve in Obsidian's graph view
- Daily notes follow `Journal/YYYY/YYYY-MM-DD.md` convention

## License

MIT
