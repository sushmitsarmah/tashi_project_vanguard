# 🌍 Decentralized Swarm Telemetry (Geotab + Tashi Network)

A high-performance Rust workspace that simulates a massive fleet of autonomous vehicles (drones, trucks, or cars) navigating a configurable urban grid. It broadcasts their real-time telemetry into a decentralized, Byzantine Fault-Tolerant (BFT) consensus network. 

This project bridges traditional enterprise telematics (using the Geotab JSON-RPC standard) with next-generation Decentralized Physical Infrastructure Networks (DePIN) powered by the **Tashi Vertex** DAG consensus engine. It is designed to be geographically agnostic and can be deployed to simulate activity in any city or country worldwide.

## 🏗️ Architecture Overview

This repository is structured as a Cargo Workspace containing two distinct services that run concurrently:

1. **`city-swarm-simulator` (The Firehose):** An asynchronous physics engine that calculates vector-based movement for 1,000 active nodes bounded within a configurable geographical grid. It exposes an HTTP mock API that perfectly mimics the **MyGeotab `GetFeed`** enterprise polling standard, serving `LogRecord` (GPS) data in real-time.

2. **`tashi-node` (The Consensus Middleware):** An edge node that bridges the centralized API into a decentralized swarm. It continuously polls the Geotab simulator, serializes the telemetry payloads using `bincode`, and submits them as memory-safe transactions into the **Tashi Vertex DAG**. This ensures all vehicle movements are cryptographically ordered and synchronized across the network in under 100 milliseconds.

---

## ⚙️  Prerequisites

Before building the project, ensure you have the following installed:

* **Rust & Cargo:** (Edition 2024 recommended)
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf [https://sh.rustup.rs](https://sh.rustup.rs) | sh
CMake: Required to compile the underlying C-core of the Tashi Vertex engine.

macOS: brew install cmake

Linux (Debian/Ubuntu): sudo apt install cmake

🚀 Getting Started
1. Configure Your Target City

By default, the simulator runs a generic urban grid. To simulate a specific city (e.g., Tokyo, London, or Mumbai), open city-swarm-simulator/src/main.rs and update the bounding box coordinates at the top of the file:

Rust
const CITY_MIN_LAT: f64 = 35.6528; // Tokyo Example
const CITY_MAX_LAT: f64 = 35.7333;
const CITY_MIN_LON: f64 = 139.6503;
const CITY_MAX_LON: f64 = 139.8394;
2. Clone & Build

Clone the repository and build the entire workspace in one go. The root Cargo.toml uses dependency resolver version 3 to handle shared dependencies across crates.

Bash
git clone <your-repo-url>
cd geotab-tashi-swarm
cargo build --workspace --release
3. Run the Simulator

In your first terminal window, start the physics engine and Geotab mock API.

Bash
cargo run --bin city-swarm-simulator --release
You should see: 🚁 Global Swarm Simulator running on port 8080

4. Run the Tashi Swarm Node

Open a second terminal window and start the edge node to begin pulling telemetry and building the DAG consensus.

Bash
cargo run --bin tashi-node --release
You should see the node initialize its cryptographic identity, bind its sockets, and begin submitting hundreds of telemetry transactions per second into the network.
