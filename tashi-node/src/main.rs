use clap::Parser;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::PathBuf;

use tashi_vertex::{Context, Engine, KeyPublic, KeySecret, Message, Options, Peers, Socket, Transaction};

// --- Add the exact same resolver here ---
pub fn get_workspace_config_path() -> PathBuf {
    let mut current_dir = env::current_dir().expect("Failed to get current directory");
    loop {
        if current_dir.join("Cargo.lock").exists() {
            return current_dir.join("swarm_config.json");
        }
        if !current_dir.pop() {
            return env::current_dir().unwrap().join("swarm_config.json");
        }
    }
}

#[derive(Parser)]
struct Args {
    #[clap(long)]
    id: u16,
}

// --- Config Schema ---
#[derive(Deserialize)]
struct NodeConfig { id: u16, port: u16, secret_key: String, public_key: String }

// --- Telemetry Schemas ---
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeotabResponse { result: GeotabResult }
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeotabResult { data: Vec<LogRecord>, to_version: String }

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct LogRecord { id: String, latitude: f64, longitude: f64, public_key_hex: String }

#[derive(Serialize, Deserialize, Debug)]
enum SwarmMessage { DroneTelemetry(LogRecord) }

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let config_path = get_workspace_config_path(); // Resolve the path

    // 1. Read the Config
    if !config_path.exists() {
        return Err(anyhow::anyhow!("❌ {:?} not found! Run the simulator first.", config_path.display()));
    }

    let data = fs::read_to_string(&config_path)?;
    let configs: Vec<NodeConfig> = serde_json::from_str(&data)?;

    // 2. Find My Identity
    let my_config = configs.iter().find(|c| c.id == args.id)
        .ok_or_else(|| anyhow::anyhow!("❌ ID {} not found in config!", args.id))?;

    let secret_key = my_config.secret_key.parse::<KeySecret>()?;
    let bind_addr = format!("127.0.0.1:{}", my_config.port);

    // 3. Setup Tashi Context
    let context = Box::leak(Box::new(Context::new()?));
    let secret = Box::leak(Box::new(secret_key));
    let socket = Socket::bind(context, &bind_addr).await?;
    
    let mut options = Options::new();
    options.set_report_gossip_events(true);

    // 4. Auto-Configure Peers
    let mut peers = Peers::with_capacity(configs.len())?;
    for c in &configs {
        if c.id != my_config.id {
            let pub_key = c.public_key.parse::<KeyPublic>()?;
            let addr = format!("127.0.0.1:{}", c.port);
            peers.insert(&addr, &pub_key, Default::default())?;
        }
    }
    peers.insert(&bind_addr, &secret.public(), Default::default())?;

    let pk_len = my_config.public_key.len();
    println!("🌐 Starting Node {} [Port: {} | PubKey: {}...]", my_config.id, my_config.port, &my_config.public_key[pk_len - 8..]);
    let engine = Engine::start(context, socket, options, secret, peers, false)?;

    let http_client = Client::new();
    let mut last_version = "0".to_string();

    poll_geotab_and_filter(&engine, &http_client, &mut last_version, &my_config.public_key).await;

    while let Ok(Some(message)) = engine.recv_message().await {
        match message {
            Message::Event(event) => {
                for tx_bytes in event.transactions() {
                    if let Ok(SwarmMessage::DroneTelemetry(log)) = bincode::deserialize(tx_bytes) {
                        let log_len = log.public_key_hex.len();
                        println!(
                            "🔒 [CONSENSUS] Drone: {}... | Lat: {:.5} | Lon: {:.5} | TS: {}",
                            &log.public_key_hex[log_len - 8..], log.latitude, log.longitude, event.consensus_at()
                        );
                    }
                }
                poll_geotab_and_filter(&engine, &http_client, &mut last_version, &my_config.public_key).await;
            }
            Message::SyncPoint(_) => {
                poll_geotab_and_filter(&engine, &http_client, &mut last_version, &my_config.public_key).await;
            }
        }
    }

    Ok(())
}

async fn poll_geotab_and_filter(engine: &Engine, client: &Client, last_version: &mut String, my_pub_key: &str) {
    let request_payload = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "GetFeed",
        "params": { "typeName": "LogRecord", "fromVersion": last_version.clone(), "credentials": { "database": "mock", "sessionId": "mock", "userName": "admin" } }
    });

    if let Ok(res) = client.post("http://localhost:8080/apiv1").json(&request_payload).send().await {
        if let Ok(json) = res.json::<GeotabResponse>().await {
            if json.result.to_version != *last_version && !json.result.data.is_empty() {
                let records = json.result.data;
                *last_version = json.result.to_version;

                for record in records {
                    if record.public_key_hex == my_pub_key {
                        let message = SwarmMessage::DroneTelemetry(record);
                        if let Ok(payload_bytes) = bincode::serialize(&message) {
                            let mut tx = Transaction::allocate(payload_bytes.len());
                            tx.copy_from_slice(&payload_bytes);
                            let _ = engine.send_transaction(tx);
                        }
                    }
                }
            }
        }
    }
}