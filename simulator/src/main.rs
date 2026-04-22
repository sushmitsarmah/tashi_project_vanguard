use axum::{routing::post, Json, Router};
use rand::Rng;
use serde::Serialize;
use serde_json::Value;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;

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

// Bounding box for the simulated fleet (roughly downtown LA).
// The three.js viewer reads these same numbers — keep them in sync.
const LAT_MIN: f64 = 34.0;
const LAT_MAX: f64 = 34.15;
const LON_MIN: f64 = -118.3;
const LON_MAX: f64 = -118.15;

// Per-drone kinematic state, persisted across requests so positions drift
// continuously instead of teleporting to a fresh random point every poll.
struct DroneKinematics {
    lat: f64,
    lon: f64,
    vlat: f64,
    vlon: f64,
}

struct AppState {
    fleet: Vec<simulator::keys::DroneIdentity>,
    kinematics: Vec<DroneKinematics>,
    version_counter: u64,
}

#[tokio::main]
async fn main() {
    let fleet = load_or_generate_config();

    // Seed each drone with a random starting point + heading inside the box.
    let mut rng = rand::thread_rng();
    let kinematics: Vec<DroneKinematics> = (0..fleet.len())
        .map(|_| DroneKinematics {
            lat: rng.gen_range(LAT_MIN..LAT_MAX),
            lon: rng.gen_range(LON_MIN..LON_MAX),
            vlat: rng.gen_range(-0.0002..0.0002),
            vlon: rng.gen_range(-0.0002..0.0002),
        })
        .collect();

    let state = Arc::new(Mutex::new(AppState {
        fleet,
        kinematics,
        version_counter: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
    }));

    // Permissive CORS so a browser client (the three.js viewer) can POST to /apiv1.
    // Fine for local development; tighten for production.
    let cors = CorsLayer::permissive();

    let app = Router::new()
        .route("/apiv1", post(handle_geotab))
        .layer(cors)
        .with_state(state);

    let listener = TcpListener::bind("127.0.0.1:8080").await.unwrap();
    println!(":: Simulator running on http://127.0.0.1:8080. Awaiting nodes...");
    axum::serve(listener, app).await.unwrap();
}

async fn handle_geotab(
    axum::extract::State(state): axum::extract::State<Arc<Mutex<AppState>>>,
    axum::extract::Json(_payload): axum::extract::Json<Value>,
) -> Json<GeotabResponse> {
    let mut guard = state.lock().await;
    // Explicit deref so the borrow checker can split borrows across `kinematics` and `fleet`.
    let state = &mut *guard;
    let mut rng = rand::thread_rng();
    let mut records = Vec::with_capacity(state.fleet.len());

    for (d, drone) in state.kinematics.iter_mut().zip(state.fleet.iter()) {
        // Nudge the heading a little for organic motion, then cap speed.
        d.vlat += rng.gen_range(-0.00005..0.00005);
        d.vlon += rng.gen_range(-0.00005..0.00005);
        d.vlat = d.vlat.clamp(-0.0005, 0.0005);
        d.vlon = d.vlon.clamp(-0.0005, 0.0005);

        // Integrate.
        d.lat += d.vlat;
        d.lon += d.vlon;

        // Bounce off the bounding box edges.
        if d.lat < LAT_MIN { d.lat = LAT_MIN; d.vlat = d.vlat.abs(); }
        if d.lat > LAT_MAX { d.lat = LAT_MAX; d.vlat = -d.vlat.abs(); }
        if d.lon < LON_MIN { d.lon = LON_MIN; d.vlon = d.vlon.abs(); }
        if d.lon > LON_MAX { d.lon = LON_MAX; d.vlon = -d.vlon.abs(); }

        records.push(LogRecord {
            id: format!("log_{}", rng.r#gen::<u32>()),
            latitude: d.lat,
            longitude: d.lon,
            public_key_hex: drone.public.to_string(),
        });
    }

    state.version_counter += 1;
    Json(GeotabResponse {
        result: GeotabResult { data: records, to_version: state.version_counter.to_string() },
    })
}