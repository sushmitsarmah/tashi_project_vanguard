use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::PathBuf;
use tashi_vertex::{KeyPublic, KeySecret};

// --- Add this robust path resolver ---
pub fn get_workspace_config_path() -> PathBuf {
    let mut current_dir = env::current_dir().expect("Failed to get current directory");
    loop {
        // If we see Cargo.lock, we know we are at the root of the workspace
        if current_dir.join("Cargo.lock").exists() {
            return current_dir.join("swarm_config.json");
        }
        // Move up one directory. If we hit the root of the OS, fallback to current dir
        if !current_dir.pop() {
            return env::current_dir().unwrap().join("swarm_config.json");
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct NodeConfig {
    pub id: u16,
    pub port: u16,
    pub secret_key: String,
    pub public_key: String,
}

#[allow(dead_code)]
pub struct DroneIdentity {
    pub secret: KeySecret,
    pub public: KeyPublic,
    pub port: u16,
}

pub fn load_or_generate_config() -> Vec<DroneIdentity> {
    let config_path = get_workspace_config_path(); // Use the new resolver
    let mut configs: Vec<NodeConfig> = Vec::new();

    if config_path.exists() {
        println!("=== 1. LOADING EXISTING SWARM CONFIG ===");
        let data = fs::read_to_string(&config_path).unwrap();
        configs = serde_json::from_str(&data).unwrap();
        println!(":: Loaded from {:?}", config_path.display());
    } else {
        println!("=== 1. GENERATING NEW SWARM CONFIG ===");
        for i in 1..=5 {
            let secret = KeySecret::generate();
            configs.push(NodeConfig {
                id: i,
                port: 8000 + i,
                secret_key: secret.to_string(),
                public_key: secret.public().to_string(),
            });
        }
        let json = serde_json::to_string_pretty(&configs).unwrap();
        fs::write(&config_path, json).unwrap();
        println!(":: Saved to {:?}", config_path.display());
    }

    configs.into_iter().map(|c| DroneIdentity {
        secret: c.secret_key.parse().unwrap(),
        public: c.public_key.parse().unwrap(),
        port: c.port,
    }).collect()
}