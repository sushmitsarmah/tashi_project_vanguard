use axum::{routing::post, Json, Router};
use rand::Rng;
use serde::Serialize;
use serde_json::Value;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::net::TcpListener;
use tokio::sync::Mutex;

// Import from your new library
use simulator::keys::load_or_generate_config;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct GeotabResponse { result: GeotabResult }

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct GeotabResult { data: Vec<LogRecord>, to_version: String }

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct LogRecord { id: String, latitude: f64, longitude: f64, public_key_hex: String }

struct AppState {
    // Import the struct from the library as well
    fleet: Vec<simulator::keys::DroneIdentity>,
    version_counter: u64,
}

#[tokio::main]
async fn main() {
    let fleet = load_or_generate_config();

    let state = Arc::new(Mutex::new(AppState {
        fleet,
        version_counter: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
    }));

    let app = Router::new().route("/apiv1", post(handle_geotab)).with_state(state);
    let listener = TcpListener::bind("127.0.0.1:8080").await.unwrap();
    println!(":: Simulator running on http://127.0.0.1:8080. Awaiting nodes...");
    axum::serve(listener, app).await.unwrap();
}

async fn handle_geotab(
    axum::extract::State(state): axum::extract::State<Arc<Mutex<AppState>>>,
    axum::extract::Json(_payload): axum::extract::Json<Value>,
) -> Json<GeotabResponse> {
    let mut state = state.lock().await;
    let mut rng = rand::thread_rng();
    let mut records = Vec::new();

    for drone in &state.fleet {
        records.push(LogRecord {
            id: format!("log_{}", rng.r#gen::<u32>()),
            latitude: 34.0 + rng.gen_range(0.0..0.15),
            longitude: -118.3 + rng.gen_range(0.0..0.15),
            public_key_hex: drone.public.to_string(),
        });
    }

    state.version_counter += 1;
    Json(GeotabResponse {
        result: GeotabResult { data: records, to_version: state.version_counter.to_string() },
    })
}