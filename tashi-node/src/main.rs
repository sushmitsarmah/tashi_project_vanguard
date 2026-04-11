use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::error::Error;

use tashi_vertex::{Context, Engine, KeySecret, Message, Options, Peers, Socket, Transaction};

// --- Geotab Response Schemas ---
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct GeotabResponse {
    result: GeotabResult,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct GeotabResult {
    data: Vec<LogRecord>,
    to_version: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct LogRecord {
    id: String,
    device: EntityReference,
    latitude: f64,
    longitude: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct EntityReference {
    id: String,
}

#[derive(Serialize, Deserialize, Debug)]
enum SwarmMessage {
    DroneTelemetry(LogRecord),
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("🌐 Initializing Tashi Vertex Edge Node...");

    // 1. Setup exactly mirroring pingback.rs
    let secret = KeySecret::generate();
    let context = Context::new()?;
    let socket = Socket::bind(&context, "127.0.0.1:8001").await?;
    let mut options = Options::new();
    options.set_report_gossip_events(true); // Ensure we get consistent network ticks

    let mut peers = Peers::new()?;
    peers.insert("127.0.0.1:8001", &secret.public(), Default::default())?;

    let engine = Engine::start(&context, socket, options, &secret, peers, false)?;
    println!("✅ Tashi Node joined the DAG network.");

    let http_client = Client::new();
    let mut last_version = "0".to_string();

    // 2. Jumpstart the network by doing an initial poll and submission
    poll_geotab_and_submit(&engine, &http_client, &mut last_version).await;

    println!("🎧 Consensus Listener Active. Awaiting BFT finality...");

    // 3. The exact while-let loop from pingback.rs
    // Because this executes sequentially, the C-callback is NEVER cancelled.
    while let Ok(Some(message)) = engine.recv_message().await {
        match message {
            Message::Event(event) => {
                for tx_bytes in event.transactions() {
                    if let Ok(swarm_msg) = bincode::deserialize::<SwarmMessage>(tx_bytes) {
                        let SwarmMessage::DroneTelemetry(log) = swarm_msg;
                        println!(
                            "🔒 [CONSENSUS] Drone: {} | Lat: {:.5} | Lon: {:.5} | Confirmed At: {}",
                            log.device.id,
                            log.latitude,
                            log.longitude,
                            event.consensus_at()
                        );
                    }
                }
                
                // After processing an event, check for new Geotab data
                poll_geotab_and_submit(&engine, &http_client, &mut last_version).await;
            }
            Message::SyncPoint(_) => {
                // The network heartbeat ticked. Perfect time to check for new data safely.
                poll_geotab_and_submit(&engine, &http_client, &mut last_version).await;
            }
        }
    }

    Ok(())
}

/// Helper function to handle the HTTP polling sequentially. 
/// Taking `&Engine` here is 100% safe because it all runs on the main thread loop.
async fn poll_geotab_and_submit(
    engine: &Engine,
    client: &Client,
    last_version: &mut String,
) {
    let request_payload = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "GetFeed",
        "params": {
            "typeName": "LogRecord",
            "fromVersion": last_version.clone(),
            "credentials": { "database": "mock", "sessionId": "mock", "userName": "admin" }
        }
    });

    if let Ok(res) = client.post("http://localhost:8080/apiv1").json(&request_payload).send().await {
        if let Ok(json) = res.json::<GeotabResponse>().await {
            if json.result.to_version != *last_version && !json.result.data.is_empty() {
                let records = json.result.data;
                *last_version = json.result.to_version;

                println!("📡 Pulled {} drone updates. Submitting to Tashi DAG...", records.len());

                for record in records {
                    let message = SwarmMessage::DroneTelemetry(record);
                    if let Ok(payload_bytes) = bincode::serialize(&message) {
                        let mut tx = Transaction::allocate(payload_bytes.len());
                        tx.copy_from_slice(&payload_bytes);
                        
                        // Submit synchronously, just like send_transaction_cstr in pingback.rs
                        if let Err(e) = engine.send_transaction(tx) {
                            eprintln!("❌ Broadcast failed: {e}");
                        }
                    }
                }
            }
        }
    }
}