//! Lane-based concurrency for gateway request processing.
//!
//! This module provides infrastructure for prioritized request processing.

#![allow(dead_code)]
//!
//! Lanes provide prioritized, concurrent request processing with:
//! - Priority ordering (Cron > Main > Subagent > Batch)
//! - Per-lane concurrency limits
//! - Fair scheduling within lanes
//! - Backpressure and overflow handling
//!
//! ## Architecture
//!
//! ```text
//! Incoming Requests
//!        │
//!        ▼
//! ┌─────────────────┐
//! │  Lane Router    │
//! └─────────────────┘
//!        │
//!   ┌────┼────┬────────┐
//!   │    │    │        │
//!   ▼    ▼    ▼        ▼
//! ┌───┐┌───┐┌────┐┌─────┐
//! │Cro││Mai││Suba││Batch│
//! │n  ││n  ││gent││     │
//! └───┘└───┘└────┘└─────┘
//!   │    │    │        │
//!   └────┼────┴────────┘
//!        │
//!        ▼
//!   Worker Pool
//! ```

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Lane types for request categorization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum LaneType {
    /// Scheduled/cron jobs - highest priority
    Cron = 0,
    /// Interactive user requests - high priority
    Main = 1,
    /// Subagent delegated tasks - medium priority
    Subagent = 2,
    /// Batch/background jobs - lowest priority
    Batch = 3,
}

impl LaneType {
    /// Get the priority (lower = higher priority)
    pub fn priority(&self) -> u8 {
        *self as u8
    }

    /// Get default concurrency limit for this lane
    pub fn default_concurrency(&self) -> usize {
        match self {
            LaneType::Cron => 2,
            LaneType::Main => 4,
            LaneType::Subagent => 8,
            LaneType::Batch => 2,
        }
    }

    /// Parse from string
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "cron" => Some(LaneType::Cron),
            "main" => Some(LaneType::Main),
            "subagent" | "sub" => Some(LaneType::Subagent),
            "batch" | "background" => Some(LaneType::Batch),
            _ => None,
        }
    }
}

impl std::fmt::Display for LaneType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LaneType::Cron => write!(f, "cron"),
            LaneType::Main => write!(f, "main"),
            LaneType::Subagent => write!(f, "subagent"),
            LaneType::Batch => write!(f, "batch"),
        }
    }
}

/// A request queued for processing
#[derive(Debug)]
pub struct QueuedRequest {
    /// Unique request ID
    pub id: String,
    /// Session ID
    pub session_id: String,
    /// Request payload
    pub payload: serde_json::Value,
    /// Time when request was queued
    pub queued_at: Instant,
    /// Lane this request belongs to
    pub lane: LaneType,
}

/// Per-lane queue and state
struct Lane {
    lane_type: LaneType,
    queue: VecDeque<QueuedRequest>,
    concurrency_limit: usize,
    active_count: usize,
    total_processed: u64,
    total_wait_time_ms: u64,
}

impl Lane {
    fn new(lane_type: LaneType, concurrency_limit: usize) -> Self {
        Self {
            lane_type,
            queue: VecDeque::new(),
            concurrency_limit,
            active_count: 0,
            total_processed: 0,
            total_wait_time_ms: 0,
        }
    }

    fn can_accept(&self) -> bool {
        self.active_count < self.concurrency_limit
    }

    fn enqueue(&mut self, request: QueuedRequest) -> usize {
        self.queue.push_back(request);
        self.queue.len()
    }

    fn dequeue(&mut self) -> Option<QueuedRequest> {
        if let Some(req) = self.queue.pop_front() {
            self.active_count += 1;
            let wait_time = req.queued_at.elapsed().as_millis() as u64;
            self.total_wait_time_ms += wait_time;
            Some(req)
        } else {
            None
        }
    }

    fn complete(&mut self) {
        self.active_count = self.active_count.saturating_sub(1);
        self.total_processed += 1;
    }

    fn pending_count(&self) -> usize {
        self.queue.len()
    }

    fn average_wait_time_ms(&self) -> f64 {
        if self.total_processed == 0 {
            0.0
        } else {
            self.total_wait_time_ms as f64 / self.total_processed as f64
        }
    }
}

/// Lane configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaneConfig {
    pub cron_concurrency: usize,
    pub main_concurrency: usize,
    pub subagent_concurrency: usize,
    pub batch_concurrency: usize,
    /// Max queue depth per lane before rejecting
    pub max_queue_depth: usize,
}

impl Default for LaneConfig {
    fn default() -> Self {
        Self {
            cron_concurrency: LaneType::Cron.default_concurrency(),
            main_concurrency: LaneType::Main.default_concurrency(),
            subagent_concurrency: LaneType::Subagent.default_concurrency(),
            batch_concurrency: LaneType::Batch.default_concurrency(),
            max_queue_depth: 100,
        }
    }
}

/// Lane statistics
#[derive(Debug, Clone, Serialize)]
pub struct LaneStats {
    pub lane: String,
    pub pending: usize,
    pub active: usize,
    pub concurrency_limit: usize,
    pub total_processed: u64,
    pub avg_wait_time_ms: f64,
}

/// Lane manager for request routing and scheduling
pub struct LaneManager {
    lanes: Mutex<[Lane; 4]>,
    config: LaneConfig,
    /// Global request counter for IDs
    request_counter: AtomicU64,
}

impl LaneManager {
    pub fn new(config: LaneConfig) -> Arc<Self> {
        Arc::new(Self {
            lanes: Mutex::new([
                Lane::new(LaneType::Cron, config.cron_concurrency),
                Lane::new(LaneType::Main, config.main_concurrency),
                Lane::new(LaneType::Subagent, config.subagent_concurrency),
                Lane::new(LaneType::Batch, config.batch_concurrency),
            ]),
            config,
            request_counter: AtomicU64::new(0),
        })
    }

    /// Enqueue a request to the appropriate lane
    ///
    /// Returns Ok(position) if queued, Err if queue is full
    pub fn enqueue(
        &self,
        lane_type: LaneType,
        session_id: String,
        payload: serde_json::Value,
    ) -> Result<(String, usize), LaneError> {
        let mut lanes = self.lanes.lock().unwrap();
        let lane = &mut lanes[lane_type as usize];

        // Check queue depth
        if lane.queue.len() >= self.config.max_queue_depth {
            return Err(LaneError::QueueFull {
                lane: lane_type,
                depth: lane.queue.len(),
            });
        }

        let id = format!(
            "{}-{}",
            lane_type,
            self.request_counter.fetch_add(1, Ordering::SeqCst)
        );

        let request = QueuedRequest {
            id: id.clone(),
            session_id,
            payload,
            queued_at: Instant::now(),
            lane: lane_type,
        };

        let position = lane.enqueue(request);

        // Note: Event emission should be done by the caller via brainpro::events
        // if they want observability for queue operations.

        Ok((id, position))
    }

    /// Try to get the next request to process
    ///
    /// Prioritizes by lane type (Cron > Main > Subagent > Batch)
    pub fn try_dequeue(&self) -> Option<QueuedRequest> {
        let mut lanes = self.lanes.lock().unwrap();

        // Process in priority order
        for lane in lanes.iter_mut() {
            if lane.can_accept() && !lane.queue.is_empty() {
                if let Some(req) = lane.dequeue() {
                    // Note: Event emission should be done by the caller via brainpro::events
                    return Some(req);
                }
            }
        }

        None
    }

    /// Mark a request as completed
    pub fn complete(&self, lane_type: LaneType) {
        let mut lanes = self.lanes.lock().unwrap();
        lanes[lane_type as usize].complete();
    }

    /// Get statistics for all lanes
    pub fn stats(&self) -> Vec<LaneStats> {
        let lanes = self.lanes.lock().unwrap();
        lanes
            .iter()
            .map(|lane| LaneStats {
                lane: lane.lane_type.to_string(),
                pending: lane.pending_count(),
                active: lane.active_count,
                concurrency_limit: lane.concurrency_limit,
                total_processed: lane.total_processed,
                avg_wait_time_ms: lane.average_wait_time_ms(),
            })
            .collect()
    }

    /// Get total pending requests across all lanes
    pub fn total_pending(&self) -> usize {
        let lanes = self.lanes.lock().unwrap();
        lanes.iter().map(|l| l.pending_count()).sum()
    }

    /// Get total active requests across all lanes
    pub fn total_active(&self) -> usize {
        let lanes = self.lanes.lock().unwrap();
        lanes.iter().map(|l| l.active_count).sum()
    }

    /// Update concurrency limit for a lane (runtime reconfiguration)
    pub fn set_concurrency(&self, lane_type: LaneType, limit: usize) {
        let mut lanes = self.lanes.lock().unwrap();
        lanes[lane_type as usize].concurrency_limit = limit;
    }
}

/// Errors from lane operations
#[derive(Debug, Clone)]
pub enum LaneError {
    QueueFull { lane: LaneType, depth: usize },
    InvalidLane { name: String },
}

impl std::fmt::Display for LaneError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LaneError::QueueFull { lane, depth } => {
                write!(f, "Queue full for lane '{}' (depth: {})", lane, depth)
            }
            LaneError::InvalidLane { name } => {
                write!(f, "Invalid lane name: '{}'", name)
            }
        }
    }
}

impl std::error::Error for LaneError {}

/// Async worker that processes requests from lanes
pub struct LaneWorker {
    manager: Arc<LaneManager>,
    shutdown: tokio::sync::watch::Receiver<bool>,
}

impl LaneWorker {
    pub fn new(manager: Arc<LaneManager>, shutdown: tokio::sync::watch::Receiver<bool>) -> Self {
        Self { manager, shutdown }
    }

    /// Run the worker, processing requests until shutdown
    pub async fn run<F, Fut>(self, handler: F)
    where
        F: Fn(QueuedRequest) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = ()> + Send,
    {
        let mut shutdown = self.shutdown;
        let poll_interval = Duration::from_millis(10);

        loop {
            tokio::select! {
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        break;
                    }
                }
                _ = tokio::time::sleep(poll_interval) => {
                    if let Some(request) = self.manager.try_dequeue() {
                        let lane = request.lane;
                        handler(request).await;
                        self.manager.complete(lane);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_lane_priority() {
        assert!(LaneType::Cron.priority() < LaneType::Main.priority());
        assert!(LaneType::Main.priority() < LaneType::Subagent.priority());
        assert!(LaneType::Subagent.priority() < LaneType::Batch.priority());
    }

    #[test]
    fn test_lane_manager_enqueue() {
        let manager = LaneManager::new(LaneConfig::default());

        let (id, pos) = manager
            .enqueue(LaneType::Main, "session-1".to_string(), json!({}))
            .unwrap();

        assert!(id.starts_with("main-"));
        assert_eq!(pos, 1);
    }

    #[test]
    fn test_lane_manager_dequeue_priority() {
        let manager = LaneManager::new(LaneConfig::default());

        // Enqueue in reverse priority order
        manager
            .enqueue(
                LaneType::Batch,
                "batch-session".to_string(),
                json!({"type": "batch"}),
            )
            .unwrap();
        manager
            .enqueue(
                LaneType::Main,
                "main-session".to_string(),
                json!({"type": "main"}),
            )
            .unwrap();
        manager
            .enqueue(
                LaneType::Cron,
                "cron-session".to_string(),
                json!({"type": "cron"}),
            )
            .unwrap();

        // Should dequeue in priority order (Cron first)
        let req1 = manager.try_dequeue().unwrap();
        assert_eq!(req1.lane, LaneType::Cron);

        let req2 = manager.try_dequeue().unwrap();
        assert_eq!(req2.lane, LaneType::Main);

        let req3 = manager.try_dequeue().unwrap();
        assert_eq!(req3.lane, LaneType::Batch);
    }

    #[test]
    fn test_lane_manager_concurrency_limit() {
        let config = LaneConfig {
            main_concurrency: 1,
            ..LaneConfig::default()
        };
        let manager = LaneManager::new(config);

        // Enqueue two main requests
        manager
            .enqueue(LaneType::Main, "s1".to_string(), json!({}))
            .unwrap();
        manager
            .enqueue(LaneType::Main, "s2".to_string(), json!({}))
            .unwrap();

        // First dequeue succeeds
        let req1 = manager.try_dequeue().unwrap();
        assert_eq!(req1.lane, LaneType::Main);

        // Second dequeue returns None (concurrency limit reached)
        assert!(manager.try_dequeue().is_none());

        // Complete the first request
        manager.complete(LaneType::Main);

        // Now second request can be dequeued
        let req2 = manager.try_dequeue().unwrap();
        assert_eq!(req2.lane, LaneType::Main);
    }

    #[test]
    fn test_lane_manager_queue_full() {
        let config = LaneConfig {
            max_queue_depth: 2,
            ..LaneConfig::default()
        };
        let manager = LaneManager::new(config);

        manager
            .enqueue(LaneType::Main, "s1".to_string(), json!({}))
            .unwrap();
        manager
            .enqueue(LaneType::Main, "s2".to_string(), json!({}))
            .unwrap();

        let result = manager.enqueue(LaneType::Main, "s3".to_string(), json!({}));
        assert!(matches!(result, Err(LaneError::QueueFull { .. })));
    }

    #[test]
    fn test_lane_stats() {
        let manager = LaneManager::new(LaneConfig::default());

        manager
            .enqueue(LaneType::Main, "s1".to_string(), json!({}))
            .unwrap();
        manager
            .enqueue(LaneType::Main, "s2".to_string(), json!({}))
            .unwrap();

        let stats = manager.stats();
        let main_stats = stats.iter().find(|s| s.lane == "main").unwrap();

        assert_eq!(main_stats.pending, 2);
        assert_eq!(main_stats.active, 0);
    }
}
