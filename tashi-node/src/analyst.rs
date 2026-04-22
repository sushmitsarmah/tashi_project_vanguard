// src/analyst.rs
//
// Local AI "tactical analyst" that runs on each drone.
// Consumes consensus-confirmed telemetry, asks a local Ollama model for a
// one-line situational summary. Requires `ollama serve` running locally with
// a small model pulled (e.g. `ollama pull llama3.2:1b`).

use rig::client::{CompletionClient, Nothing};
use rig::completion::Prompt;
use rig::providers::ollama;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

use crate::LogRecord; // re-uses the type already defined in main.rs

/// Rolling window of recent consensus-confirmed telemetry, keyed only by order.
pub struct AnalystBuffer {
    events: VecDeque<(u64, LogRecord)>, // (consensus_at, record)
    capacity: usize,
}

impl AnalystBuffer {
    pub fn new(capacity: usize) -> Self {
        Self { events: VecDeque::with_capacity(capacity), capacity }
    }
    pub fn push(&mut self, ts: u64, rec: LogRecord) {
        if self.events.len() >= self.capacity {
            self.events.pop_front();
        }
        self.events.push_back((ts, rec));
    }
    fn snapshot_latest_per_drone(&self) -> Vec<serde_json::Value> {
        // Keep only the most recent fix per drone to keep the prompt small.
        let mut latest: HashMap<&str, (u64, f64, f64)> = HashMap::new();
        for (ts, r) in &self.events {
            latest
                .entry(r.public_key_hex.as_str())
                .and_modify(|v| { if *ts > v.0 { *v = (*ts, r.latitude, r.longitude); } })
                .or_insert((*ts, r.latitude, r.longitude));
        }
        latest
            .into_iter()
            .map(|(pk, (ts, lat, lon))| {
                serde_json::json!({
                    // Short tag so the LLM has something human-readable to refer to.
                    "unit": &pk[pk.len().saturating_sub(8)..],
                    "ts": ts,
                    "lat": format!("{:.5}", lat),
                    "lon": format!("{:.5}", lon),
                })
            })
            .collect()
    }
}

/// Background task: every `interval` seconds, summarize the recent swarm state
/// through a local Ollama model and print it.
pub async fn run_analyst(
    buffer: Arc<Mutex<AnalystBuffer>>,
    analyst_slot: Option<Arc<Mutex<Option<String>>>>,
    node_id: u16,
    interval: Duration,
    model: &str,
) {
    // Ollama defaults to http://localhost:11434 — no credentials needed.
    let client = match ollama::Client::new(Nothing) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("🧠 [ANALYST {}] Ollama client init failed: {e}. Disabled.", node_id);
            return;
        }
    };

    let agent = client
        .agent(model)
        .preamble(
            "You are the on-board tactical analyst for an autonomous drone swarm. \
             You will receive a JSON list of the most recent position fix per drone, \
             from consensus. Produce ONE concise sentence (max ~20 words) describing \
             current swarm activity: formation shape, bearing, outliers, clustering. \
             Military tone. No preamble, no markdown, no list — one sentence.",
        )
        .build();

    loop {
        tokio::time::sleep(interval).await;

        let payload = {
            let buf = buffer.lock().await;
            buf.snapshot_latest_per_drone()
        };
        if payload.is_empty() {
            continue;
        }

        let prompt = format!(
            "Swarm state (latest fix per drone):\n{}",
            serde_json::to_string(&payload).unwrap_or_else(|_| "[]".into())
        );

        // match agent.prompt(&prompt).await {
        //     Ok(resp) => println!("🧠 [ANALYST {}] {}", node_id, resp.trim()),
        //     Err(e)   => eprintln!("🧠 [ANALYST {}] LLM error: {e}", node_id),
        // }
        match agent.prompt(&prompt).await {
            Ok(resp) => {
                let line = resp.trim().to_string();
                println!("🧠 [ANALYST {}] {}", node_id, line);
                // Publish to the read-only state endpoint so the HUD can show it.
                if let Some(slot) = &analyst_slot {
                    *slot.lock().await = Some(line);
                }
            }
            Err(e) => eprintln!("🧠 [ANALYST {}] LLM error: {e}", node_id),
        }
    }
}