//! KeepAlive Protocol (node-to-node, protocol #8)
//!
//! Monitors connection health through periodic cookie-based challenge-response exchanges.
//!
//! ## Protocol Overview
//!
//! The KeepAlive protocol is a simple ping-pong mechanism that:
//! - Detects dead or stalled connections
//! - Measures round-trip latency
//! - Validates bidirectional communication
//! - Uses random cookies to prevent replay attacks
//!
//! ## State Machine
//!
//! ```text
//! ┌────────┐
//! │ Client │─────MsgKeepAlive(cookie)────▶┌────────┐
//! │        │◀────MsgKeepAliveResponse─────│ Server │
//! └────────┘                               └────────┘
//!
//! Flow:
//! 1. Client sends MsgKeepAlive with random 16-bit cookie
//! 2. Server responds with MsgKeepAliveResponse echoing the same cookie
//! 3. Client validates cookie matches the sent value
//! 4. If timeout or mismatch occurs, connection is considered dead
//! ```
//!
//! ## Timing
//!
//! - **Interval**: Typically 60 seconds between pings
//! - **Timeout**: 30 seconds to receive response
//! - **Max Failures**: 3 consecutive failures before marking connection dead
//!
//! ## Use Cases
//!
//! 1. **Connection monitoring**: Detect TCP half-open states
//! 2. **Latency measurement**: Track RTT for peer selection
//! 3. **NAT traversal**: Keep NAT mappings alive
//! 4. **Idle connections**: Maintain long-lived connections
//!
//! ## Reference
//!
//! Based on IntersectMBO/ouroboros-network KeepAlive module

use crate::{NetworkError, Result};
use bytes::Bytes;
use minicbor::{Decode, Encode};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::{Duration, Instant};

/// Protocol number for KeepAlive (node-to-node)
pub const PROTOCOL_NUM: u16 = 8;

/// Default interval between keepalive pings (60 seconds)
pub const DEFAULT_KEEPALIVE_INTERVAL: Duration = Duration::from_secs(60);

/// Default timeout waiting for keepalive response (30 seconds)
pub const DEFAULT_KEEPALIVE_TIMEOUT: Duration = Duration::from_secs(30);

/// Maximum consecutive failures before marking connection dead
pub const MAX_CONSECUTIVE_FAILURES: u32 = 3;

/// KeepAlive protocol states
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum State {
    /// Idle, waiting to send next ping
    Idle,
    /// Waiting for response to sent ping
    WaitingForResponse,
    /// Protocol terminated or connection dead
    Done,
}

/// KeepAlive protocol messages
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Encode, Decode)]
pub enum Message {
    /// Client sends ping with random cookie
    #[n(0)]
    MsgKeepAlive {
        #[n(0)]
        cookie: u16,
    },

    /// Server echoes cookie back to client
    #[n(1)]
    MsgKeepAliveResponse {
        #[n(0)]
        cookie: u16,
    },

    /// Either party terminates the protocol
    #[n(2)]
    MsgDone,
}

/// KeepAlive statistics for monitoring
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeepAliveStats {
    /// Total pings sent
    pub pings_sent: u64,
    /// Total successful responses received
    pub responses_received: u64,
    /// Total timeouts
    pub timeouts: u64,
    /// Total cookie mismatches
    pub mismatches: u64,
    /// Consecutive failures
    pub consecutive_failures: u32,
    /// Last successful round-trip time in milliseconds
    pub last_rtt_ms: Option<u64>,
    /// Average round-trip time in milliseconds
    pub avg_rtt_ms: Option<u64>,
}

impl Default for KeepAliveStats {
    fn default() -> Self {
        Self {
            pings_sent: 0,
            responses_received: 0,
            timeouts: 0,
            mismatches: 0,
            consecutive_failures: 0,
            last_rtt_ms: None,
            avg_rtt_ms: None,
        }
    }
}

impl KeepAliveStats {
    /// Check if connection should be considered dead
    pub fn is_connection_dead(&self) -> bool {
        self.consecutive_failures >= MAX_CONSECUTIVE_FAILURES
    }

    /// Get success rate as percentage (0-100)
    pub fn success_rate(&self) -> f64 {
        if self.pings_sent == 0 {
            return 100.0;
        }
        (self.responses_received as f64 / self.pings_sent as f64) * 100.0
    }
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            State::Idle => write!(f, "Idle"),
            State::WaitingForResponse => write!(f, "WaitingForResponse"),
            State::Done => write!(f, "Done"),
        }
    }
}

impl fmt::Display for KeepAliveStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "KeepAlive Stats: {}/{} pings successful ({:.1}%), {} timeouts, {} mismatches, {} consecutive failures",
            self.responses_received,
            self.pings_sent,
            self.success_rate(),
            self.timeouts,
            self.mismatches,
            self.consecutive_failures
        )?;
        if let Some(rtt) = self.last_rtt_ms {
            write!(f, ", last RTT: {}ms", rtt)?;
        }
        if let Some(avg_rtt) = self.avg_rtt_ms {
            write!(f, ", avg RTT: {}ms", avg_rtt)?;
        }
        Ok(())
    }
}

/// KeepAlive protocol handler
pub struct KeepAlive {
    state: State,
    current_cookie: Option<u16>,
    ping_sent_at: Option<Instant>,
    stats: KeepAliveStats,
    rtt_history: Vec<u64>,
}

impl KeepAlive {
    /// Create a new KeepAlive protocol instance
    pub fn new() -> Self {
        Self {
            state: State::Idle,
            current_cookie: None,
            ping_sent_at: None,
            stats: KeepAliveStats::default(),
            rtt_history: Vec::new(),
        }
    }

    /// Get the current protocol state
    pub fn state(&self) -> State {
        self.state
    }

    /// Get protocol statistics
    pub fn stats(&self) -> &KeepAliveStats {
        &self.stats
    }

    /// Check if connection is considered dead
    pub fn is_connection_dead(&self) -> bool {
        self.stats.is_connection_dead()
    }

    /// Generate a keepalive ping with random cookie
    pub fn send_ping(&mut self) -> Result<Message> {
        match self.state {
            State::Idle => {
                // Generate random 16-bit cookie
                let cookie = generate_cookie();

                self.current_cookie = Some(cookie);
                self.ping_sent_at = Some(Instant::now());
                self.state = State::WaitingForResponse;
                self.stats.pings_sent += 1;

                Ok(Message::MsgKeepAlive { cookie })
            }
            _ => Err(NetworkError::ProtocolError(format!(
                "Cannot send ping in state {:?}",
                self.state
            ))),
        }
    }

    /// Handle an incoming message
    pub fn handle_message(&mut self, msg: Message) -> Result<Option<Message>> {
        match (&self.state, &msg) {
            // Server receives ping, echoes cookie back
            (State::Idle, Message::MsgKeepAlive { cookie }) => {
                Ok(Some(Message::MsgKeepAliveResponse { cookie: *cookie }))
            }

            // Client receives response to ping
            (State::WaitingForResponse, Message::MsgKeepAliveResponse { cookie }) => {
                self.handle_response(*cookie)?;
                self.state = State::Idle;
                Ok(None)
            }

            // Any state can receive Done
            (_, Message::MsgDone) => {
                self.state = State::Done;
                Ok(None)
            }

            _ => Err(NetworkError::ProtocolError(format!(
                "Invalid message {:?} in state {:?}",
                msg, self.state
            ))),
        }
    }

    /// Handle a keepalive response
    fn handle_response(&mut self, received_cookie: u16) -> Result<()> {
        match self.current_cookie {
            Some(expected_cookie) if expected_cookie == received_cookie => {
                // Success - calculate RTT
                if let Some(sent_at) = self.ping_sent_at {
                    let rtt = sent_at.elapsed();
                    let rtt_ms = rtt.as_millis() as u64;

                    self.stats.last_rtt_ms = Some(rtt_ms);
                    self.rtt_history.push(rtt_ms);

                    // Keep last 10 RTT measurements for average
                    if self.rtt_history.len() > 10 {
                        self.rtt_history.remove(0);
                    }

                    // Calculate average RTT
                    let avg = self.rtt_history.iter().sum::<u64>() / self.rtt_history.len() as u64;
                    self.stats.avg_rtt_ms = Some(avg);
                }

                self.stats.responses_received += 1;
                self.stats.consecutive_failures = 0;
                self.current_cookie = None;
                self.ping_sent_at = None;

                Ok(())
            }
            Some(expected_cookie) => {
                // Cookie mismatch
                self.stats.mismatches += 1;
                self.stats.consecutive_failures += 1;
                self.current_cookie = None;
                self.ping_sent_at = None;

                Err(NetworkError::ProtocolError(format!(
                    "Cookie mismatch: expected {}, received {}",
                    expected_cookie, received_cookie
                )))
            }
            None => Err(NetworkError::ProtocolError(
                "Received response without pending ping".to_string(),
            )),
        }
    }

    /// Handle timeout (no response received within timeout period)
    pub fn handle_timeout(&mut self) -> Result<()> {
        match self.state {
            State::WaitingForResponse => {
                self.stats.timeouts += 1;
                self.stats.consecutive_failures += 1;
                self.state = State::Idle;
                self.current_cookie = None;
                self.ping_sent_at = None;

                if self.is_connection_dead() {
                    self.state = State::Done;
                    Err(NetworkError::ProtocolError(
                        "Connection dead: too many consecutive failures".to_string(),
                    ))
                } else {
                    Ok(())
                }
            }
            _ => Err(NetworkError::ProtocolError(format!(
                "Unexpected timeout in state {:?}",
                self.state
            ))),
        }
    }

    /// Encode a message to bytes
    pub fn encode_message(msg: &Message) -> Result<Bytes> {
        let vec = minicbor::to_vec(msg)
            .map_err(|e| NetworkError::EncodingError(format!("CBOR encode error: {}", e)))?;
        Ok(Bytes::from(vec))
    }

    /// Decode a message from bytes
    pub fn decode_message(bytes: &[u8]) -> Result<Message> {
        minicbor::decode(bytes)
            .map_err(|e| NetworkError::ProtocolError(format!("CBOR decode error: {}", e)))
    }
}

impl Default for KeepAlive {
    fn default() -> Self {
        Self::new()
    }
}

/// Generate a random 16-bit cookie for keepalive challenge
fn generate_cookie() -> u16 {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};

    let random_state = RandomState::new();
    let mut hasher = random_state.build_hasher();
    hasher.write_u64(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64,
    );
    (hasher.finish() & 0xFFFF) as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keepalive_creation() {
        let keepalive = KeepAlive::new();
        assert_eq!(keepalive.state(), State::Idle);
        assert_eq!(keepalive.stats().pings_sent, 0);
        assert_eq!(keepalive.stats().responses_received, 0);
        assert!(!keepalive.is_connection_dead());
    }

    #[test]
    fn test_send_ping() {
        let mut keepalive = KeepAlive::new();

        let msg = keepalive.send_ping().unwrap();
        assert!(matches!(msg, Message::MsgKeepAlive { .. }));
        assert_eq!(keepalive.state(), State::WaitingForResponse);
        assert_eq!(keepalive.stats().pings_sent, 1);
    }

    #[test]
    fn test_successful_ping_pong() {
        let mut keepalive = KeepAlive::new();

        // Send ping
        let ping = keepalive.send_ping().unwrap();
        let cookie = match ping {
            Message::MsgKeepAlive { cookie } => cookie,
            _ => panic!("Expected MsgKeepAlive"),
        };

        // Receive response
        let response = Message::MsgKeepAliveResponse { cookie };
        keepalive.handle_message(response).unwrap();

        assert_eq!(keepalive.state(), State::Idle);
        assert_eq!(keepalive.stats().responses_received, 1);
        assert_eq!(keepalive.stats().consecutive_failures, 0);
        assert!(keepalive.stats().last_rtt_ms.is_some());
    }

    #[test]
    fn test_cookie_mismatch() {
        let mut keepalive = KeepAlive::new();

        // Send ping
        keepalive.send_ping().unwrap();

        // Receive response with wrong cookie
        let wrong_response = Message::MsgKeepAliveResponse { cookie: 0xFFFF };
        let result = keepalive.handle_message(wrong_response);

        assert!(result.is_err());
        assert_eq!(keepalive.stats().mismatches, 1);
        assert_eq!(keepalive.stats().consecutive_failures, 1);
    }

    #[test]
    fn test_timeout_handling() {
        let mut keepalive = KeepAlive::new();

        // Send ping
        keepalive.send_ping().unwrap();
        assert_eq!(keepalive.state(), State::WaitingForResponse);

        // Timeout
        keepalive.handle_timeout().unwrap();

        assert_eq!(keepalive.state(), State::Idle);
        assert_eq!(keepalive.stats().timeouts, 1);
        assert_eq!(keepalive.stats().consecutive_failures, 1);
    }

    #[test]
    fn test_connection_death() {
        let mut keepalive = KeepAlive::new();

        // Simulate 3 consecutive timeouts
        for _ in 0..MAX_CONSECUTIVE_FAILURES {
            keepalive.send_ping().unwrap();
            if keepalive.stats().consecutive_failures < MAX_CONSECUTIVE_FAILURES - 1 {
                keepalive.handle_timeout().unwrap();
            }
        }

        // Final timeout should mark connection as dead
        let result = keepalive.handle_timeout();
        assert!(result.is_err());
        assert!(keepalive.is_connection_dead());
        assert_eq!(keepalive.state(), State::Done);
    }

    #[test]
    fn test_server_echo() {
        let mut server = KeepAlive::new();

        // Server receives ping
        let ping = Message::MsgKeepAlive { cookie: 12345 };
        let response = server.handle_message(ping).unwrap();

        assert_eq!(
            response,
            Some(Message::MsgKeepAliveResponse { cookie: 12345 })
        );
        assert_eq!(server.state(), State::Idle);
    }

    #[test]
    fn test_stats_success_rate() {
        let mut stats = KeepAliveStats::default();
        assert_eq!(stats.success_rate(), 100.0);

        stats.pings_sent = 10;
        stats.responses_received = 8;
        assert_eq!(stats.success_rate(), 80.0);

        stats.pings_sent = 100;
        stats.responses_received = 95;
        assert_eq!(stats.success_rate(), 95.0);
    }

    #[test]
    fn test_encode_decode_roundtrip() {
        let msg = Message::MsgKeepAlive { cookie: 54321 };

        let encoded = KeepAlive::encode_message(&msg).unwrap();
        let decoded = KeepAlive::decode_message(&encoded).unwrap();

        assert_eq!(msg, decoded);
    }

    #[test]
    fn test_cookie_generation() {
        // Generate multiple cookies and verify the function works
        let _cookie1 = generate_cookie();
        let _cookie2 = generate_cookie();
        let _cookie3 = generate_cookie();
        // Test passes if generate_cookie() doesn't panic
    }

    #[test]
    fn test_rtt_tracking() {
        let mut keepalive = KeepAlive::new();

        // Simulate multiple successful pings
        for _ in 0..5 {
            let ping = keepalive.send_ping().unwrap();
            let cookie = match ping {
                Message::MsgKeepAlive { cookie } => cookie,
                _ => panic!("Expected MsgKeepAlive"),
            };

            std::thread::sleep(std::time::Duration::from_millis(10));

            let response = Message::MsgKeepAliveResponse { cookie };
            keepalive.handle_message(response).unwrap();
        }

        assert!(keepalive.stats().last_rtt_ms.is_some());
        assert!(keepalive.stats().avg_rtt_ms.is_some());
        assert_eq!(keepalive.stats().responses_received, 5);
    }

    #[test]
    fn test_done_message() {
        let mut keepalive = KeepAlive::new();

        keepalive.handle_message(Message::MsgDone).unwrap();
        assert_eq!(keepalive.state(), State::Done);
    }
}
