use axum::{extract::State, routing::post, Json, Router};
use chrono::Utc;
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::{Arc, Mutex};
use tokio::time::{sleep, Duration};

// --- Target City Configuration ---
// Easily swap these coordinates for any city or country bounding box.
// (Example coordinates form a roughly 15km x 15km generic urban grid)
const CITY_MIN_LAT: f64 = 34.0000;
const CITY_MAX_LAT: f64 = 34.1500;
const CITY_MIN_LON: f64 = -118.3000;
const CITY_MAX_LON: f64 = -118.1500;
const FLEET_SIZE: usize = 1000;

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
struct Vehicle {
    id: String,
    lat: f64,
    lon: f64,
    heading_lat: f64,
    heading_lon: f64,
}

struct AppState {
    fleet: Mutex<Vec<Vehicle>>,
    version: Mutex<u64>,
}

#[tokio::main]
async fn main() {
    let mut rng = rand::thread_rng();
    let mut initial_fleet = Vec::with_capacity(FLEET_SIZE);

    // Initialize the swarm randomly distributed across the target city grid
    for i in 1..=FLEET_SIZE {
        initial_fleet.push(Vehicle {
            id: format!("node_{}", i),
            lat: rng.gen_range(CITY_MIN_LAT..CITY_MAX_LAT),
            lon: rng.gen_range(CITY_MIN_LON..CITY_MAX_LON),
            heading_lat: rng.gen_range(-0.0005..0.0005),
            heading_lon: rng.gen_range(-0.0005..0.0005),
        });
    }

    let state = Arc::new(AppState {
        fleet: Mutex::new(initial_fleet),
        version: Mutex::new(1),
    });

    // Background Task: Physics Engine updating positions every second
    let physics_state = Arc::clone(&state);
    tokio::spawn(async move {
        loop {
            sleep(Duration::from_millis(1000)).await;
            let mut fleet = physics_state.fleet.lock().unwrap();
            for vehicle in fleet.iter_mut() {
                // Move vehicle along its vector
                vehicle.lat += vehicle.heading_lat;
                vehicle.lon += vehicle.heading_lon;
                
                // Keep them inside the designated city bounding box
                if vehicle.lat > CITY_MAX_LAT || vehicle.lat < CITY_MIN_LAT { vehicle.heading_lat *= -1.0; }
                if vehicle.lon > CITY_MAX_LON || vehicle.lon < CITY_MIN_LON { vehicle.heading_lon *= -1.0; }
            }
            *physics_state.version.lock().unwrap() += 1;
        }
    });

    let app = Router::new()
        .route("/apiv1", post(geotab_handler))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    println!("🚁 Global Swarm Simulator running on port 8080");
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
    let fleet = state.fleet.lock().unwrap();
    let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);

    let records: Vec<LogRecord> = fleet.iter().map(|v| LogRecord {
        id: format!("{}_{}", v.id, current_version),
        device: EntityReference { id: v.id.clone() },
        date_time: now.clone(),
        latitude: v.lat,
        longitude: v.lon,
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