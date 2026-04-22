mod analyst;
mod state_server;

use analyst::{AnalystBuffer, run_analyst};
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;
use std::time::Duration;

use clap::Parser;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::PathBuf;

// --- CAPTIVE PORTAL & DATABASE IMPORTS ---
use axum::{extract::State, http::StatusCode, response::Html, routing::{get, post}, Json, Router};
use redb::{Database, TableDefinition};
use std::net::SocketAddr;
use tokio::sync::mpsc;
// ---------------------------------------------

use tashi_vertex::{Context, Engine, KeyPublic, KeySecret, Message, Options, Peers, Socket, Transaction};

// --- Define the Redb Table ---
const SURVIVOR_LOCATIONS: TableDefinition<&str, &[u8]> = TableDefinition::new("survivor_locations");

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

#[derive(Deserialize)]
struct NodeConfig { id: u16, port: u16, secret_key: String, public_key: String }

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeotabResponse { result: GeotabResult }
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeotabResult { data: Vec<LogRecord>, to_version: String }

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct LogRecord { id: String, latitude: f64, longitude: f64, public_key_hex: String }

// --- CAPTIVE PORTAL STRUCT ---
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct SurvivorPing {
    pub device_id: String,
    pub lat: f64,
    pub lon: f64,
}

// --- UPDATED MESSAGE ENUM ---
#[derive(Serialize, Deserialize, Debug)]
enum SwarmMessage { 
    DroneTelemetry(LogRecord),
    SurvivorLocated(SurvivorPing), // New Event
}

#[derive(Clone)]
struct PortalState {
    tashi_tx: mpsc::Sender<SurvivorPing>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let config_path = get_workspace_config_path();

    if !config_path.exists() {
        return Err(anyhow::anyhow!("❌ {:?} not found! Run the simulator first.", config_path.display()));
    }

    let data = fs::read_to_string(&config_path)?;
    let configs: Vec<NodeConfig> = serde_json::from_str(&data)?;

    let my_config = configs.iter().find(|c| c.id == args.id)
        .ok_or_else(|| anyhow::anyhow!("❌ ID {} not found in config!", args.id))?;

    let secret_key = my_config.secret_key.parse::<KeySecret>()?;
    let bind_addr = format!("127.0.0.1:{}", my_config.port);

    // --- INITIALIZE REDB DATABASE ---
    let db_filename = format!("vanguard_node_{}.redb", my_config.id);
    let db = Database::create(&db_filename).expect("Failed to create redb database");
    let write_txn = db.begin_write()?;
    { write_txn.open_table(SURVIVOR_LOCATIONS)?; }
    write_txn.commit()?;
    println!("💾 Redb Storage initialized: {}", db_filename);
    
    // --- START CAPTIVE PORTAL ---
    let (ping_tx, mut ping_rx) = mpsc::channel::<SurvivorPing>(100);
    let captive_port = 8000 + my_config.id; // Ports 8001, 8002, etc.
    let portal_state = PortalState { tashi_tx: ping_tx };
    
    tokio::spawn(async move {
        let app: Router = Router::new()
            .route("/", get(serve_portal))
            .route("/generate_204", get(serve_portal))            
            .route("/hotspot-detect.html", get(serve_portal))     
            .route("/ping", post(receive_ping))
            .with_state(portal_state);

        let addr = SocketAddr::from(([0, 0, 0, 0], captive_port));
        println!("📡 Captive Portal active on http://127.0.0.1:{}", captive_port);
        let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
        axum::serve(listener, app).await.unwrap();
    });

    let context = Box::leak(Box::new(Context::new()?));
    let secret = Box::leak(Box::new(secret_key));
    let socket = Socket::bind(context, &bind_addr).await?;
    
    let mut options = Options::new();
    options.set_report_gossip_events(true);

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

    let node_state = state_server::NodeState::new(my_config.id);
    {
        let ns = node_state.clone();
        let http_port = 9000 + my_config.id;
        tokio::spawn(async move { state_server::serve(ns, http_port).await });
    }

    let analyst_buf = Arc::new(TokioMutex::new(AnalystBuffer::new(256)));
    {
        let ab = analyst_buf.clone();
        let id = my_config.id;
        let analyst_slot = node_state.analyst_slot();
        tokio::spawn(async move {
            run_analyst(ab, Some(analyst_slot), id, Duration::from_secs(15), "llama3.2:1b").await;
        });
    }

    let http_client = Client::new();
    let mut last_version = "0".to_string();

    poll_geotab_and_filter(&engine, &http_client, &mut last_version, &my_config.public_key).await;

    // --- NON-BLOCKING EVENT LOOP ---
    loop {
        tokio::select! {
            // 1. Listen for new pings from the Captive Portal Web Server
            Some(ping) = ping_rx.recv() => {
                println!("📲 Injecting Survivor Ping into DAG: {}", ping.device_id);
                let msg = SwarmMessage::SurvivorLocated(ping);
                if let Ok(bytes) = bincode::serialize(&msg) {
                    let mut tx = Transaction::allocate(bytes.len());
                    tx.copy_from_slice(&bytes);
                    let _ = engine.send_transaction(tx);
                }
            }
            
            // 2. Listen for finalized messages from the Tashi DAG
            Ok(Some(message)) = engine.recv_message() => {
                match message {
                    Message::Event(event) => {
                        for tx_bytes in event.transactions() {
                            if let Ok(msg) = bincode::deserialize::<SwarmMessage>(tx_bytes) {
                                match msg {
                                    SwarmMessage::DroneTelemetry(log) => {
                                        let log_len = log.public_key_hex.len();
                                        println!(
                                            "🔒 [CONSENSUS] Drone: {}... | Lat: {:.5} | Lon: {:.5} | TS: {}",
                                            &log.public_key_hex[log_len - 8..], log.latitude, log.longitude, event.consensus_at()
                                        );
                                        
                                        node_state.apply_telemetry(log.clone()).await;
                                        analyst_buf.lock().await.push(event.consensus_at(), log);
                                    }
                                    SwarmMessage::SurvivorLocated(ping) => {
                                        println!("✅ [CONSENSUS] Survivor Verified: {} at {}, {}", ping.device_id, ping.lat, ping.lon);
                                        
                                        // --- UPDATED: Pass data to state_server for HUD visualization ---
                                        node_state.apply_survivor(ping.clone()).await;
                                        // ----------------------------------------------------------------

                                        let ping_bytes = serde_json::to_vec(&ping).unwrap();
                                        if let Ok(write_txn) = db.begin_write() {
                                            if let Ok(mut table) = write_txn.open_table(SURVIVOR_LOCATIONS) {
                                                let _ = table.insert(ping.device_id.as_str(), ping_bytes.as_slice());
                                            }
                                            let _ = write_txn.commit();
                                        }
                                    }
                                }
                            }
                        }
                        poll_geotab_and_filter(&engine, &http_client, &mut last_version, &my_config.public_key).await;
                    }
                    Message::SyncPoint(_) => {
                        poll_geotab_and_filter(&engine, &http_client, &mut last_version, &my_config.public_key).await;
                    }
                }
            }
        }
    }
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

// --- CAPTIVE PORTAL HANDLERS ---
async fn serve_portal() -> Html<&'static str> {
    Html(r##"
        <!DOCTYPE html>
        <html>
        <head>
            <meta name="viewport" content="width=device-width, initial-scale=1">
            <title>Emergency Mesh</title>
            <style>
                body { font-family: system-ui; text-align: center; padding-top: 20%; background: #1a1a1a; color: white; margin: 0; padding: 20px;}
                .loader { border: 4px solid #333; border-top: 4px solid #ff3b30; border-radius: 50%; width: 40px; height: 40px; animation: spin 1s linear infinite; margin: 20px auto; }
                @keyframes spin { 0% { transform: rotate(0deg); } 100% { transform: rotate(360deg); } }
                p#status { margin-top: 20px; font-size: 18px; color: #a8a8a8; line-height: 1.5; }
            </style>
        </head>
        <body>
            <h2>Rescue Mesh Active</h2>
            <div id="loader" class="loader"></div>
            <p id="status">Connecting to rescue network...<br><br><b style="color:white;">Please tap "Allow" when prompted for location.</b></p>

            <script>
                if (!localStorage.getItem("device_id")) {
                    localStorage.setItem("device_id", "survivor_" + Math.floor(Math.random() * 1000000));
                }
                window.onload = function() {
                    const statusText = document.getElementById("status");
                    const loader = document.getElementById("loader");
                    if ("geolocation" in navigator) {
                        navigator.geolocation.getCurrentPosition(async (pos) => {
                            const payload = {
                                device_id: localStorage.getItem("device_id"),
                                lat: pos.coords.latitude,
                                lon: pos.coords.longitude
                            };
                            try {
                                const response = await fetch('/ping', {
                                    method: 'POST',
                                    headers: { 'Content-Type': 'application/json' },
                                    body: JSON.stringify(payload)
                                });
                                if (response.ok) {
                                    loader.style.display = "none";
                                    statusText.innerHTML = "<b>📍 Location Sent!</b><br><br>Stay where you are. Rescue drones have your coordinates.";
                                    statusText.style.color = "#32d74b"; 
                                }
                            } catch (e) {
                                statusText.innerText = "Network Error.";
                            }
                        }, (err) => {
                            loader.style.display = "none";
                            statusText.innerHTML = "<b>⚠️ Cannot Find You</b><br>You must allow location access so rescue teams can find you.";
                            statusText.style.color = "#ff3b30"; 
                        }, { enableHighAccuracy: true });
                    }
                };
            </script>
        </body>
        </html>
    "##)
}

async fn receive_ping(State(state): State<PortalState>, Json(payload): Json<SurvivorPing>) -> StatusCode {
    match state.tashi_tx.send(payload).await {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}