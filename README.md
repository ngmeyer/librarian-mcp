# Librarian

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Claude Code](https://img.shields.io/badge/Claude_Code-Skill-blueviolet?logo=anthropic)](https://claude.ai/code)
[![MCP](https://img.shields.io/badge/MCP-Server-orange?logo=data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSIyNCIgaGVpZ2h0PSIyNCIgdmlld0JveD0iMCAwIDI0IDI0IiBmaWxsPSJ3aGl0ZSI+PHBhdGggZD0iTTEyIDJMMyA3djEwbDkgNSA5LTV2LTEweiIvPjwvc3ZnPg==)](https://modelcontextprotocol.io)
[![Homebrew](https://img.shields.io/badge/Homebrew-Available-FBB040?logo=homebrew&logoColor=white)](https://github.com/ngmeyer/homebrew-tap)
[![Platform](https://img.shields.io/badge/Platform-macOS%20%7C%20Linux%20%7C%20Windows-lightgrey)]()

Give Claude a librarian for your markdown vault.

Librarian is an MCP server that connects [Claude](https://claude.ai) to your [Obsidian](https://obsidian.md) vault or any folder of markdown files. Search your notes, auto-link mentions as wikilinks, explore backlinks and tags, import documents — all from inside Claude.

Inspired by [Karpathy's wiki-as-memory](https://x.com/karpathy) approach: use a knowledge base as persistent, structured memory for Claude instead of ephemeral conversation context.

## Install

### Homebrew (macOS)

```bash
brew install ngmeyer/tap/librarian-mcp
```

### Pre-built binaries

Download from [GitHub Releases](https://github.com/ngmeyer/librarian-mcp/releases) for macOS (arm64/x86_64), Linux, and Windows.

### Build from source

```bash
cargo install --git https://github.com/ngmeyer/librarian-mcp --bin librarian-mcp
```

## Setup

Point Librarian at your vault and auto-configure Claude:

```bash
librarian-mcp --setup ~/my-vault
```

This writes the MCP server entry into Claude Desktop and Claude Code configs (with backups). Restart Claude to connect.

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
LIBRARIAN_VAULTS=/vault/one:/vault/two # Multiple vaults (colon-separated)
```

## Tools

Librarian exposes 14 tools to Claude:

| Tool | Description |
|------|-------------|
| `library_search` | Full-text search across all vault files (trigram-indexed) |
| `library_read` | Read a file by relative path |
| `library_write` | Write a file with auto-wikilink detection for Obsidian graph compatibility |
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
| `library_import` | Convert PDF, DOCX, images, etc. to markdown via MarkItDown |

## Features

### Auto-linking

When writing files, Librarian scans for mentions of existing note titles and wraps them in `[[wikilinks]]`. Links use canonical file names so they resolve correctly in Obsidian's graph view — even on case-sensitive filesystems.

Frontmatter aliases are supported: if a note has `aliases: [ML, machine learning]`, mentions of "ML" or "machine learning" in other notes will auto-link to it using `[[Note Name|ML]]` format.

### Knowledge graph traversal

Librarian builds a bidirectional graph from your vault's `[[wikilinks]]` and exposes three graph tools:

- **Traverse** — BFS from any note, N hops deep. "Show me everything connected to this topic."
- **Shortest path** — Find the link chain between two notes. "How are these ideas connected?"
- **Graph analysis** — Connected components, hub notes, bridge notes, orphans. "What's the structure of my vault?"

### Search

Search is backed by an in-memory trigram index built on startup. Fast enough for vaults with 10,000+ files.

### Exclusion patterns

By default, Librarian skips `.obsidian/`, `.trash/`, `.git/`, and `node_modules/`. Drop a `.librarianignore` file (gitignore syntax) in your vault root to customize:

```gitignore
# .librarianignore
.obsidian/
.trash/
templates/
private/
```

## Obsidian compatibility

Librarian is designed to work seamlessly with Obsidian vaults:

- Reads and writes standard `[[wikilinks]]` and `[[note|display text]]`
- Respects YAML frontmatter and `aliases`
- Skips `.obsidian/` and `.trash/` directories
- Auto-linked wikilinks resolve in Obsidian's graph view
- Daily notes follow `Journal/YYYY/YYYY-MM-DD.md` convention

## License

MIT
