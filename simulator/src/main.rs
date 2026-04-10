use axum::{extract::State, routing::post, Json, Router};
use chrono::Utc;
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::{Arc, Mutex};
use tokio::time::{sleep, Duration};

// --- Geotab Mock Schemas ---
#[derive(Deserialize, Debug)]
struct RpcRequest { method: String, id: Option<Value> }

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct EntityReference { id: String }

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct LogRecord {
    id: String,
    device: EntityReference,
    date_time: String,
    latitude: f64,
    longitude: f64,
    speed: f32,
}

// --- Physics State ---
struct Drone {
    id: String,
    lat: f64,
    lon: f64,
    heading_lat: f64,
    heading_lon: f64,
}

struct AppState {
    drones: Mutex<Vec<Drone>>,
    version: Mutex<u64>,
}

#[tokio::main]
async fn main() {
    let mut rng = rand::thread_rng();
    let mut initial_drones = Vec::with_capacity(1000);

    // Initialize 1000 drones over Manhattan
    for i in 1..=1000 {
        initial_drones.push(Drone {
            id: format!("drone_{}", i),
            lat: rng.gen_range(40.7000..40.8500),
            lon: rng.gen_range(-74.0200..-73.9300),
            heading_lat: rng.gen_range(-0.0005..0.0005),
            heading_lon: rng.gen_range(-0.0005..0.0005),
        });
    }

    let state = Arc::new(AppState {
        drones: Mutex::new(initial_drones),
        version: Mutex::new(1),
    });

    // Background Task: Physics Engine updating positions every second
    let physics_state = Arc::clone(&state);
    tokio::spawn(async move {
        loop {
            sleep(Duration::from_millis(1000)).await;
            let mut drones = physics_state.drones.lock().unwrap();
            for drone in drones.iter_mut() {
                // Move drone along its vector
                drone.lat += drone.heading_lat;
                drone.lon += drone.heading_lon;
                
                // Keep them inside the Manhattan bounding box
                if drone.lat > 40.8500 || drone.lat < 40.7000 { drone.heading_lat *= -1.0; }
                if drone.lon > -73.9300 || drone.lon < -74.0200 { drone.heading_lon *= -1.0; }
            }
            *physics_state.version.lock().unwrap() += 1;
        }
    });

    let app = Router::new()
        .route("/apiv1", post(geotab_handler))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    println!("🚁 NYC Drone Simulator running on port 8080");
    axum::serve(listener, app).await.unwrap();
}

async fn geotab_handler(State(state): State<Arc<AppState>>, Json(payload): Json<RpcRequest>) -> Json<Value> {
    // Respond to Authenticate so SDKs don't crash
    if payload.method == "Authenticate" {
        return Json(serde_json::json!({
            "jsonrpc": "2.0", "id": payload.id, "result": { "credentials": { "sessionId": "mock" } }
        }));
    }

    // Serve the live physics state
    let current_version = state.version.lock().unwrap().to_string();
    let drones = state.drones.lock().unwrap();
    let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);

    let records: Vec<LogRecord> = drones.iter().map(|d| LogRecord {
        id: format!("{}_{}", d.id, current_version),
        device: EntityReference { id: d.id.clone() },
        date_time: now.clone(),
        latitude: d.lat,
        longitude: d.lon,
        speed: 15.0, // m/s
    }).collect();

    Json(serde_json::json!({
        "jsonrpc": "2.0",
        "id": payload.id,
        "result": {
            "data": records,
            "toVersion": current_version
        }
    }))
}