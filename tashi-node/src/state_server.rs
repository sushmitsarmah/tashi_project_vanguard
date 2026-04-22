// src/state_server.rs
//
// Per-node read-only HTTP endpoint exposing consensus-confirmed state.

use axum::{extract::State, routing::get, Json, Router};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tower_http::cors::CorsLayer;

use crate::{LogRecord, SurvivorPing}; // Added SurvivorPing import

/// Shared per-node state. Cheap to clone (all handles are Arc).
#[derive(Clone)]
pub struct NodeState {
    pub node_id: u16,
    drones: Arc<RwLock<HashMap<String, LogRecord>>>,
    survivors: Arc<RwLock<HashMap<String, SurvivorPing>>>, // NEW
    analyst: Arc<Mutex<Option<String>>>,
}

impl NodeState {
    pub fn new(node_id: u16) -> Self {
        Self {
            node_id,
            drones: Arc::new(RwLock::new(HashMap::new())),
            survivors: Arc::new(RwLock::new(HashMap::new())), // NEW
            analyst: Arc::new(Mutex::new(None)),
        }
    }

    /// Call this from the consensus loop whenever a DroneTelemetry tx clears consensus.
    pub async fn apply_telemetry(&self, rec: LogRecord) {
        self.drones
            .write()
            .await
            .insert(rec.public_key_hex.clone(), rec);
    }

    /// Call this when a survivor ping clears consensus.
    pub async fn apply_survivor(&self, ping: SurvivorPing) { // NEW
        self.survivors
            .write()
            .await
            .insert(ping.device_id.clone(), ping);
    }

    /// Hand this Arc to the analyst task so it can publish its latest summary.
    pub fn analyst_slot(&self) -> Arc<Mutex<Option<String>>> {
        self.analyst.clone()
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StateSnapshot {
    node_id: u16,
    drones: Vec<LogRecord>,
    survivors: Vec<SurvivorPing>, // NEW
    analyst: Option<String>,
}

async fn get_state(State(s): State<NodeState>) -> Json<StateSnapshot> {
    let drones_guard = s.drones.read().await;
    let survivors_guard = s.survivors.read().await; // NEW
    let analyst_guard = s.analyst.lock().await;
    
    let mut drones: Vec<LogRecord> = drones_guard.values().cloned().collect();
    drones.sort_by(|a, b| a.public_key_hex.cmp(&b.public_key_hex));

    // Sort survivors by ID for stable payload output
    let mut survivors: Vec<SurvivorPing> = survivors_guard.values().cloned().collect(); // NEW
    survivors.sort_by(|a, b| a.device_id.cmp(&b.device_id));

    Json(StateSnapshot {
        node_id: s.node_id,
        drones,
        survivors, // NEW
        analyst: analyst_guard.clone(),
    })
}

/// Port convention used by the viewer: 9000 + node_id.
pub async fn serve(state: NodeState, port: u16) {
    let app = Router::new()
        .route("/state", get(get_state))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = format!("127.0.0.1:{}", port);
    match tokio::net::TcpListener::bind(&addr).await {
        Ok(listener) => {
            println!("📡 State endpoint: http://{}/state", addr);
            if let Err(e) = axum::serve(listener, app).await {
                eprintln!("📡 State server error: {e}");
            }
        }
        Err(e) => eprintln!("📡 Failed to bind {addr}: {e}"),
    }
}