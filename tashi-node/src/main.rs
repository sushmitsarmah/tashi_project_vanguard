use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::time::Duration;
use tokio::time::sleep;

// Correct Tashi SDK imports
use tashi_vertex::{Context, Engine, KeySecret, Options, Peers, Socket, Transaction};

// --- Geotab Response Schemas ---
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct GeotabResponse { result: GeotabResult }

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
struct EntityReference { id: String }

// --- Swarm Payload ---
#[derive(Serialize, Deserialize, Debug)]
enum SwarmMessage {
    DroneTelemetry(LogRecord),
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("🌐 Initializing Tashi Vertex Edge Node...");
    
    // 1. Initialize the Tashi Consensus Engine
    // Generate a temporary cryptographic identity for this session
    let secret = KeySecret::generate();
    
    // Initialize an empty peers list (we are running a solo node for this demo)
    let peers = Peers::new()?;
    
    // The Context handles internal resource management and async coordination
    let context = Context::new()?;
    
    // Bind the async UDP/TCP socket for Tashi gossip protocol
    let socket = Socket::bind(&context, "127.0.0.1:8001").await?;
    
    let options = Options::new();
    
    // Start the consensus engine (consumes socket and peers)
    let engine = Engine::start(&context, socket, options, &secret, peers, true)?;
    println!("✅ Tashi Node joined the DAG network.");

    let http_client = Client::new();
    let mut last_version = "0".to_string();

    // 2. Poll & Broadcast Loop
    loop {
        let request_payload = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "GetFeed",
            "params": {
                "typeName": "LogRecord",
                "fromVersion": last_version,
                "credentials": { "database": "mock", "sessionId": "mock", "userName": "admin" }
            }
        });

        match http_client.post("http://localhost:8080/apiv1")
            .json(&request_payload)
            .send()
            .await 
        {
            Ok(res) => {
                if let Ok(json) = res.json::<GeotabResponse>().await {
                    if json.result.to_version != last_version && !json.result.data.is_empty() {
                        let records = json.result.data;
                        last_version = json.result.to_version;

                        println!("📡 Pulled {} drone updates. Submitting to Tashi DAG...", records.len());

                        // 3. Serialize and submit to the decentralized swarm
                        for record in records {
                            let message = SwarmMessage::DroneTelemetry(record);
                            let payload_bytes = bincode::serialize(&message)?;
                            
                            // Allocate a memory-safe transaction buffer for the Vertex C-engine
                            let mut transaction = Transaction::allocate(payload_bytes.len());
                            transaction.copy_from_slice(&payload_bytes);
                            
                            // Submit the transaction for consensus ordering
                            engine.send_transaction(transaction)?;
                        }
                    }
                }
            }
            Err(e) => eprintln!("Waiting for Simulator... ({})", e),
        }

        sleep(Duration::from_millis(500)).await;
    }
}