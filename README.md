# 🌍 Decentralized Swarm Telemetry (Geotab + Tashi Network)

A high-performance Rust workspace that simulates a massive fleet of autonomous vehicles (drones, trucks, or cars) navigating a configurable urban grid. It broadcasts their real-time telemetry into a decentralized, Byzantine Fault-Tolerant (BFT) consensus network.

This project bridges traditional enterprise telematics (using the Geotab JSON-RPC standard) with next-generation Decentralized Physical Infrastructure Networks (DePIN) powered by the **Tashi Vertex** DAG consensus engine. It is designed to be geographically agnostic and can be deployed to simulate activity in any city or country worldwide.

---

## 🎬 Demo

**P2P Warm-up — Discovery, Heartbeats & State Replication**
[![P2P Warm-up Demo](https://img.youtube.com/vi/GGLbCZIoI08/maxresdefault.jpg)](https://youtu.be/GGLbCZIoI08)

> Demonstrates: node discovery & signed handshake · bidirectional heartbeats · replicated JSON state · role toggle propagation (<1s) · failure injection & automatic recovery.

---

## 🏗️ Architecture Overview

This repository is structured as a Cargo Workspace containing three distinct services:

1. **`city-swarm-simulator` (The Firehose):** An asynchronous physics engine that calculates vector-based movement for 1,000 active nodes bounded within a configurable geographical grid. It exposes an HTTP mock API that perfectly mimics the **MyGeotab `GetFeed`** enterprise polling standard, serving `LogRecord` (GPS) data in real-time.

2. **`tashi-node` (The Consensus Middleware):** An edge node that bridges the centralized API into a decentralized swarm. It continuously polls the Geotab simulator, serializes the telemetry payloads using `bincode`, and submits them as memory-safe transactions into the **Tashi Vertex DAG**. This ensures all vehicle movements are cryptographically ordered and synchronized across the network in under 100 milliseconds.

3. **`warmup` (The P2P Primitives Demo):** A standalone two-agent demo that proves the core P2P coordination primitives — discovery, signed handshakes, heartbeats, replicated state, and failure recovery — independently of the full swarm simulation.

---

## ⚙️ Prerequisites

Before building the project, ensure you have the following installed:

- **Rust & Cargo** (Edition 2024 recommended)
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```

- **CMake** — required to compile the underlying C-core of the Tashi Vertex engine.
  ```bash
  # macOS
  brew install cmake

  # Linux (Debian/Ubuntu)
  sudo apt install cmake
  ```

---

## 🚀 Getting Started

### 1. Configure Your Target City

By default, the simulator runs a generic urban grid. To simulate a specific city (e.g., Tokyo, London, or Mumbai), open `city-swarm-simulator/src/main.rs` and update the bounding box coordinates at the top of the file:

```rust
const CITY_MIN_LAT: f64 = 35.6528; // Tokyo Example
const CITY_MAX_LAT: f64 = 35.7333;
const CITY_MIN_LON: f64 = 139.6503;
const CITY_MAX_LON: f64 = 139.8394;
```

### 2. Clone & Build

Clone the repository and build the entire workspace in one go. The root `Cargo.toml` uses dependency resolver version 3 to handle shared dependencies across crates.

```bash
git clone <your-repo-url>
cd geotab-tashi-swarm
cargo build --workspace --release
```

### 3. Initialize the Swarm Configuration

Run the simulator first. On its initial boot, it will generate a swarm_config.json in the workspace root. This file contains the unique cryptographic identities (Ed25519) and port assignments for all 5 nodes.

Bash
#### Terminal 1
    cargo run --bin city-swarm-simulator
    Wait for the message: === 1. GENERATING NEW SWARM CONFIG ===

### 4. Launch the Tashi Peer Network

Tashi Vertex requires a Byzantine Fault-Tolerant (BFT) quorum to process data. For a 5-node swarm, you must have at least 3 nodes online before any telemetry is finalized in the DAG.

Open 3 to 5 additional terminals and launch the peers by their ID:

Bash
#### Terminal 2
    cargo run --bin tashi-node -- --id 1

#### Terminal 3
    cargo run --bin tashi-node -- --id 2

#### Terminal 4 (Quorum reached here!)
    cargo run --bin tashi-node -- --id 3

### 5. Verify Consensus

Once the 3rd node joins, all terminals will begin streaming finalized telemetry logs:
🔒 [CONSENSUS] Drone: ...vGhYxYyq | Lat: 34.05185 | Lon: -118.27685 | TS: 177588822

You should see the node initialize its cryptographic identity, bind its sockets, and begin submitting hundreds of telemetry transactions per second into the network.

---

## 🔗 P2P Warm-up: Discovery, Heartbeats & Shared State

This warm-up proves the fundamental P2P coordination primitives before the full autonomous fleet is deployed. It runs independently of the swarm simulator.

### What it demonstrates

| Requirement | How it's implemented |
|---|---|
| **Discovery & Handshake** | On consensus forming, each node broadcasts a signed `Hello`. Any node that receives a `Hello` from a new peer replies with a `HelloAck`, completing a two-way handshake. |
| **Heartbeats** | Every node sends a `Heartbeat` transaction every 3 seconds. Both directions are visible in the logs. |
| **Replicated State** | Each node maintains a live `peer_map` of `{ peer_id, last_seen_ms, role, status }` updated on every inbound message. |
| **Role Toggle** | Agent A (Node 1) automatically flips its role to `Scout` after 20 seconds and broadcasts a `StateUpdate`. Agent B mirrors the change and prints `[MIRROR]`. |
| **Failure & Recovery** | Killing any node causes the others to print `[STALE]` after 8 seconds of silence. Restarting it triggers `[RECOVERY]` immediately. |

### Setup

> **BFT quorum note:** Tashi Vertex requires **all configured nodes to be online simultaneously** before any transaction commits. The warm-up uses a dedicated 3-node config (2 demo agents + 1 silent quorum participant) so consensus can form with all three running.

**Step 1 — Generate a fresh 3-node config** (run once):

```bash
cargo run --bin gen_warmup_config
```

This writes `warmup_config.json` to the workspace root with new keypairs on ports `9001–9003`.

**Step 2 — Open 3 terminal windows and start all nodes:**

```bash
# Terminal 1 — Agent A (triggers role toggle at t=20s)
cargo run --bin warmup -- --id 1 --config warmup_config

# Terminal 2 — Agent B (mirrors state changes)
cargo run --bin warmup -- --id 2 --config warmup_config

# Terminal 3 — Silent quorum node (required for consensus, no demo output)
cargo run --bin warmup -- --id 3 --config warmup_config --silent
```

Once all three are running you will see consensus form and the handshake sequence begin:

```
🌐  [CONNECTED]  Consensus formed — broadcasting Hello
   ↳ 🤝 [HELLO] Sent │ role: Carrier │ status: Ready
🤝  [HELLO]        Peer …Ty4gx4wE │ role: Scout │ status: Ready │ msg age 31ms
   ↳ 🤝 [HELLO ACK] → peer …Ty4gx4wE │ handshake complete ✓
💓  [HEARTBEAT]    Peer …Ty4gx4wE │ role: Scout │ status: Ready │ msg age 34ms
✅  [ALIVE]   Peer …Ty4gx4wE │ role: Scout │ status: Ready │ 1.2s ago
```

### Failure injection

```bash
# 1. After ~30s of stable heartbeats, kill Terminal 3 (Ctrl-C)
# 2. Wait 10 seconds — Agents A and B will print:
#    ⚠️  [STALE] Peer …xxxxxxxx │ silent 10.3s ← STALE

# 3. Restart the quorum node:
cargo run --bin warmup -- --id 3 --config warmup_config --silent
# 4. Agents A and B immediately print:
#    🔁 [RECOVERY] Peer …xxxxxxxx back after 10.3s — connection resumed
```

### CLI reference

| Flag | Default | Description |
|---|---|---|
| `--id` | *(required)* | Node ID (must match an entry in the config) |
| `--config` | `warmup_config` | Config file stem (without `.json`) |
| `--toggle-after` | `20` | Seconds before Agent A toggles its role. Set `0` to disable. |
| `--stale-secs` | `8` | Seconds of silence before a peer is flagged stale |
| `--silent` | off | Participate in consensus without demo output (use for Node 3) |