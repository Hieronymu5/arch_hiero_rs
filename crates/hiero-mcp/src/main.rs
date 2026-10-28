//! `arch-hiero-mcp`: a local MCP server, over stdio, that lets agents read and edit the
//! diagrams in the Arch Hiero database. Logging goes to stderr; stdout carries the protocol.
//!
//! Usage: `arch-hiero-mcp [--db <path>]`. Without `--db`, it uses `ARCH_HIERO_DB` or the
//! app's usual database.

// The MCP handlers are async because the protocol's traits are, even where they don't wait.
#![allow(clippy::unused_async, clippy::unused_async_trait_impl)]

mod server;

use rmcp::ServiceExt;

fn database_path() -> Result<std::path::PathBuf, String> {
    let mut args = std::env::args().skip(1);
    if let Some(arg) = args.next() {
        match arg.as_str() {
            "--db" => {
                return args
                    .next()
                    .map(Into::into)
                    .ok_or_else(|| "--db needs a path".to_owned());
            }
            "--help" | "-h" => {
                println!(
                    "arch-hiero-mcp [--db <path>]\n\nA local MCP server (stdio) for Arch Hiero diagrams."
                );
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    std::env::var_os("ARCH_HIERO_DB")
        .map(Into::into)
        .or_else(hiero_store::default_path)
        .ok_or_else(|| "there's no folder to keep the diagrams in".to_owned())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = database_path()?;
    let store = hiero_store::Store::open(&path)?;
    eprintln!("arch-hiero-mcp: using {}", path.display());
    let service = server::HieroServer::new(store)
        .serve(rmcp::transport::stdio())
        .await?;
    service.waiting().await?;
    Ok(())
}
