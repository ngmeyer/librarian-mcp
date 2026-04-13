use std::path::PathBuf;

/// Bundled /librarian skill definition, installed to ~/.claude/skills/librarian/ during setup.
const SKILL_CONTENT: &str = include_str!("../skill/SKILL.md");

pub fn find_binary_path() -> String {
    std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "librarian-mcp".to_string())
}

pub fn claude_desktop_config_path() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        dirs::home_dir().map(|h| h.join("Library/Application Support/Claude/claude_desktop_config.json"))
    }
    #[cfg(target_os = "linux")]
    {
        dirs::home_dir().map(|h| h.join(".config/Claude/claude_desktop_config.json"))
    }
    #[cfg(target_os = "windows")]
    {
        std::env::var("APPDATA").ok().map(|a| PathBuf::from(a).join("Claude/claude_desktop_config.json"))
    }
}

pub fn claude_code_config_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude/settings.json"))
}

pub fn run_setup(vault_paths: &[PathBuf]) -> Result<(), Box<dyn std::error::Error>> {
    let binary = find_binary_path();
    let vault_args: Vec<String> = vault_paths.iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    let mut configured = Vec::new();

    // Claude Desktop
    if let Some(config_path) = claude_desktop_config_path() {
        if let Some(parent) = config_path.parent() {
            if parent.exists() {
                let mut config: serde_json::Value = if config_path.exists() {
                    let content = std::fs::read_to_string(&config_path)?;
                    serde_json::from_str(&content).unwrap_or(serde_json::json!({}))
                } else {
                    serde_json::json!({})
                };

                if config_path.exists() {
                    let backup = config_path.with_extension("json.bak");
                    std::fs::copy(&config_path, &backup)?;
                }

                let mcp_servers = config
                    .as_object_mut().unwrap()
                    .entry("mcpServers")
                    .or_insert(serde_json::json!({}));

                mcp_servers.as_object_mut().unwrap().insert(
                    "librarian".to_string(),
                    serde_json::json!({
                        "command": binary,
                        "args": vault_args,
                    }),
                );

                std::fs::write(&config_path, serde_json::to_string_pretty(&config)?)?;
                configured.push(format!("Claude Desktop ({})", config_path.display()));
            }
        }
    }

    // Claude Code
    if let Some(config_path) = claude_code_config_path() {
        if let Some(parent) = config_path.parent() {
            if parent.exists() {
                let mut config: serde_json::Value = if config_path.exists() {
                    let content = std::fs::read_to_string(&config_path)?;
                    serde_json::from_str(&content).unwrap_or(serde_json::json!({}))
                } else {
                    serde_json::json!({})
                };

                if config_path.exists() {
                    let backup = config_path.with_extension("json.bak");
                    std::fs::copy(&config_path, &backup)?;
                }

                let mcp_servers = config
                    .as_object_mut().unwrap()
                    .entry("mcpServers")
                    .or_insert(serde_json::json!({}));

                mcp_servers.as_object_mut().unwrap().insert(
                    "librarian".to_string(),
                    serde_json::json!({
                        "command": binary,
                        "args": vault_args,
                    }),
                );

                std::fs::write(&config_path, serde_json::to_string_pretty(&config)?)?;
                configured.push(format!("Claude Code ({})", config_path.display()));
            }
        }
    }

    // Install /librarian skill to ~/.claude/skills/librarian/
    let mut skill_installed = false;
    if let Some(home) = dirs::home_dir() {
        let skill_dir = home.join(".claude").join("skills").join("librarian");
        let skill_path = skill_dir.join("SKILL.md");
        if let Err(e) = std::fs::create_dir_all(&skill_dir) {
            eprintln!("Warning: could not create {}: {}", skill_dir.display(), e);
        } else {
            let old_path = home.join(".claude").join("commands").join("librarian.md");
            if old_path.exists() {
                let _ = std::fs::remove_file(&old_path);
            }
            match std::fs::write(&skill_path, SKILL_CONTENT) {
                Ok(_) => {
                    configured.push(format!("/librarian skill ({})", skill_path.display()));
                    skill_installed = true;
                }
                Err(e) => {
                    eprintln!("Warning: could not write skill file: {}", e);
                }
            }
        }
    }

    if configured.is_empty() {
        eprintln!("No Claude installations found. Install Claude Desktop or Claude Code first.");
        eprintln!("You can manually add this to your MCP config:");
        eprintln!();
        eprintln!("  \"librarian\": {{");
        eprintln!("    \"command\": \"{}\",", binary);
        eprintln!("    \"args\": {:?}", vault_args);
        eprintln!("  }}");
    } else {
        println!("Librarian configured for:");
        for target in &configured {
            println!("  ✓ {}", target);
        }
        println!();
        println!("Vault{}: {}", if vault_args.len() > 1 { "s" } else { "" }, vault_args.join(", "));
        if skill_installed {
            println!();
            println!("Skill installed: /librarian (ingest, search, connect, daily, graph, status, analyze)");
        }
        println!();
        println!("Restart Claude to connect your vault.");
    }

    Ok(())
}
