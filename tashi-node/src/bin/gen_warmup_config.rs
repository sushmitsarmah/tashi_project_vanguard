/// src/bin/gen_warmup_config.rs
///
/// Generates a fresh `warmup_config.json` with exactly 3 nodes:
///   Node 1 = Agent A  (demo)
///   Node 2 = Agent B  (demo)
///   Node 3 = Quorum   (silent consensus participant, must be running)
///
/// BFT consensus with n=3 requires all 3 online (f=0 fault tolerance).
/// Run once, then start nodes 1, 2, and 3 in separate terminals.
///
/// Usage:
///   cargo run --bin gen_warmup_config

use std::env;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tashi_vertex::KeySecret;

pub fn get_workspace_config_path() -> PathBuf {
    let mut current_dir = env::current_dir().expect("Failed to get current directory");
    loop {
        if current_dir.join("Cargo.lock").exists() {
            return current_dir.join("warmup_config.json");
        }
        if !current_dir.pop() {
            return env::current_dir().unwrap().join("warmup_config.json");
        }
    }
}

#[derive(Serialize, Deserialize)]
struct NodeConfig {
    id: u16,
    port: u16,
    secret_key: String,
    public_key: String,
}

fn main() -> anyhow::Result<()> {
    let base_port: u16 = 9001; // Use 9xxx to avoid clashing with your drone swarm ports

    let nodes: Vec<NodeConfig> = (0..3u16)
        .map(|i| {
            let secret = KeySecret::generate();
            let public = secret.public();
            NodeConfig {
                id: i + 1,
                port: base_port + i,
                secret_key: secret.to_string(),
                public_key: public.to_string(),
            }
        })
        .collect();

    let out_path = get_workspace_config_path();
    let json = serde_json::to_string_pretty(&nodes)?;
    fs::write(&out_path, &json)?;

    println!("✅ Generated {:?}", out_path);
    println!();
    for n in &nodes {
        let pk_len = n.public_key.len();
        println!(
            "  Node {} │ port {} │ key …{}",
            n.id,
            n.port,
            &n.public_key[pk_len.saturating_sub(8)..]
        );
    }
    println!();
    println!("Next steps — open 3 terminals:");
    println!("  Terminal 1:  cargo run --bin warmup -- --id 1 --config warmup");
    println!("  Terminal 2:  cargo run --bin warmup -- --id 2 --config warmup");
    println!("  Terminal 3:  cargo run --bin warmup -- --id 3 --config warmup  (silent quorum node)");
    Ok(())
}