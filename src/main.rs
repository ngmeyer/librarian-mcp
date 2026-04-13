mod cache;
mod graph;
mod report;
mod search;
mod server;
mod setup;
mod tools;
mod viz;

use clap::Parser;
use rmcp::ServiceExt;
use std::path::PathBuf;
use std::sync::Mutex;

use cache::VaultCache;
use server::LibraryServer;

/// Give Claude a librarian for your markdown vault.
///
/// Librarian is an MCP server that connects Claude to your Obsidian vault
/// or any folder of markdown files. It provides search, auto-linking,
/// backlinks, tags, and more.
#[derive(Parser)]
#[command(name = "librarian-mcp", version, about)]
struct Cli {
    /// Vault paths to serve (can specify multiple)
    #[arg(value_name = "VAULT_PATH")]
    vaults: Vec<PathBuf>,

    /// Auto-configure Claude Desktop and Claude Code to use Librarian
    #[arg(long)]
    setup: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let default_ignores = vec![
        ".obsidian/".to_string(),
        ".trash/".to_string(),
        ".git/".to_string(),
        "node_modules/".to_string(),
    ];

    // Resolve vault paths: CLI args > env vars > default
    let library_paths: Vec<PathBuf> = if !cli.vaults.is_empty() {
        cli.vaults.clone()
    } else if let Ok(vaults) = std::env::var("LIBRARIAN_VAULTS") {
        vaults.split(':').map(PathBuf::from).collect()
    } else if let Ok(vault) = std::env::var("LIBRARIAN_VAULT") {
        vec![PathBuf::from(vault)]
    } else {
        eprintln!("No vault specified. Usage: librarian-mcp /path/to/vault");
        let default = dirs::home_dir()
            .map(|h| h.join("vault"))
            .unwrap_or_else(|| PathBuf::from("."));
        vec![default]
    };

    // Handle --setup
    if cli.setup {
        return setup::run_setup(&library_paths).map_err(|e| e.into());
    }

    // Validate paths
    for path in &library_paths {
        if !path.exists() {
            eprintln!("Warning: vault path does not exist: {}", path.display());
        }
    }

    let vault_display: Vec<_> = library_paths.iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    // Build a temporary server to access all_md_files for cache building
    let server = LibraryServer {
        library_paths,
        default_ignores,
        cache: std::sync::Arc::new(Mutex::new(VaultCache::default())),
        tool_router: LibraryServer::new_tool_router(),
    };

    // Build full vault cache (search index, graph, titles) in one pass
    let vault_cache = VaultCache::build_full(&server);
    *server.cache.lock().unwrap() = vault_cache;

    if vault_display.len() == 1 {
        eprintln!("Librarian MCP starting — vault: {}", vault_display[0]);
    } else {
        eprintln!("Librarian MCP starting — {} vaults: {}", vault_display.len(), vault_display.join(", "));
    }

    let transport = rmcp::transport::stdio();
    let service = server.serve(transport).await?;
    service.waiting().await?;

    Ok(())
}
