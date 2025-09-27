use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

/// Unique identifier for a peer in the P2P network
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PeerId {
    /// Blake2b-256 hash of the peer's public key
    pub id: [u8; 32],
}

impl PeerId {
    /// Create a new PeerId from a 32-byte array
    pub fn new(id: [u8; 32]) -> Self {
        Self { id }
    }

    /// Create a random PeerId for testing
    #[cfg(test)]
    pub fn random(seed: u8) -> Self {
        let mut id = [0u8; 32];
        id[0] = seed;
        // Fill with deterministic pattern for reproducible tests
        for i in 1..32 {
            id[i] = (seed.wrapping_mul(i as u8)).wrapping_add(i as u8);
        }
        Self { id }
    }

    /// Convert to hex string for display
    pub fn to_hex(&self) -> String {
        hex::encode(self.id)
    }

    /// Parse from hex string
    pub fn from_hex(hex: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let bytes = hex::decode(hex)?;
        if bytes.len() != 32 {
            return Err("PeerId must be 32 bytes".into());
        }
        let mut id = [0u8; 32];
        id.copy_from_slice(&bytes);
        Ok(Self { id })
    }
}

impl std::fmt::Display for PeerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", &self.to_hex()[..12]) // Show first 12 chars
    }
}

/// Current connection state of a peer
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    /// Never attempted connection
    Disconnected,
    /// Currently attempting to connect
    Connecting,
    /// Successfully connected
    Connected,
    /// Connection attempt failed with reason
    Failed(String),
    /// Peer is banned due to misbehavior
    Banned,
}

impl ConnectionState {
    /// Check if peer is available for connection attempts
    pub fn is_available(&self) -> bool {
        matches!(self, ConnectionState::Disconnected | ConnectionState::Failed(_))
    }

    /// Check if peer is currently connected
    pub fn is_connected(&self) -> bool {
        matches!(self, ConnectionState::Connected)
    }

    /// Check if peer is banned
    pub fn is_banned(&self) -> bool {
        matches!(self, ConnectionState::Banned)
    }
}

/// Peer reputation scoring system
#[derive(Debug, Clone, PartialEq)]
pub struct ReputationScore {
    /// Current reputation value
    value: i32,
}

impl ReputationScore {
    /// Initial reputation for new peers
    pub const INITIAL: i32 = 0;
    /// Maximum reputation value
    pub const MAX: i32 = 1000;
    /// Minimum reputation value (ban threshold)
    pub const MIN: i32 = -500;
    /// Reputation threshold below which peer is considered banned
    pub const BAN_THRESHOLD: i32 = -400;

    /// Create new reputation score
    pub fn new(value: i32) -> Self {
        Self {
            value: value.clamp(Self::MIN, Self::MAX),
        }
    }

    /// Get current reputation value
    pub fn value(&self) -> i32 {
        self.value
    }

    /// Adjust reputation by delta
    pub fn adjust(&mut self, delta: i32) {
        self.value = (self.value + delta).clamp(Self::MIN, Self::MAX);
    }

    /// Check if peer is banned based on reputation
    pub fn is_banned(&self) -> bool {
        self.value <= Self::BAN_THRESHOLD
    }

    /// Reset reputation to initial value
    pub fn reset(&mut self) {
        self.value = Self::INITIAL;
    }
}

impl Default for ReputationScore {
    fn default() -> Self {
        Self::new(Self::INITIAL)
    }
}

/// Severity levels for peer misbehavior
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MisbehaviorSeverity {
    /// Minor protocol violations (e.g., sending duplicate messages)
    Minor,
    /// Moderate violations (e.g., invalid message format)
    Moderate,
    /// Serious violations (e.g., sending invalid blocks)
    Serious,
    /// Critical violations (e.g., attempting network attacks)
    Critical,
}

impl MisbehaviorSeverity {
    /// Get reputation penalty for this misbehavior level
    pub fn penalty(&self) -> i32 {
        match self {
            MisbehaviorSeverity::Minor => -5,
            MisbehaviorSeverity::Moderate => -15,
            MisbehaviorSeverity::Serious => -35,
            MisbehaviorSeverity::Critical => -75,
        }
    }
}

/// Comprehensive information about a peer
#[derive(Debug, Clone)]
pub struct PeerInfo {
    /// Unique peer identifier
    pub peer_id: PeerId,
    /// Network address for connection
    pub address: SocketAddr,
    /// Current connection state
    pub connection_state: ConnectionState,
    /// Reputation score and history
    pub reputation: ReputationScore,
    /// Whether this peer is a relay node
    pub is_relay: bool,
    /// Associated stake pool ID (for SPO nodes)
    pub stake_pool_id: Option<String>,
    /// Total connection attempts made
    pub connection_attempts: u32,
    /// Number of successful connections
    pub successful_connections: u32,
    /// When this peer was first discovered
    pub discovered_at: Instant,
    /// Last time we attempted connection
    pub last_connection_attempt: Option<Instant>,
    /// Last successful connection time
    pub last_successful_connection: Option<Instant>,
    /// Data transfer statistics
    pub bytes_sent: u64,
    pub bytes_received: u64,
    /// Protocol version supported by peer
    pub protocol_version: Option<u32>,
    /// Custom metadata
    pub metadata: HashMap<String, String>,
}

impl PeerInfo {
    /// Create new peer info
    pub fn new(peer_id: PeerId, address: SocketAddr) -> Self {
        Self {
            peer_id,
            address,
            connection_state: ConnectionState::Disconnected,
            reputation: ReputationScore::default(),
            is_relay: false,
            stake_pool_id: None,
            connection_attempts: 0,
            successful_connections: 0,
            discovered_at: Instant::now(),
            last_connection_attempt: None,
            last_successful_connection: None,
            bytes_sent: 0,
            bytes_received: 0,
            protocol_version: None,
            metadata: HashMap::new(),
        }
    }

    /// Check if peer is available for connection
    pub fn is_available(&self) -> bool {
        self.connection_state.is_available() && !self.reputation.is_banned()
    }

    /// Calculate connection success rate
    pub fn connection_success_rate(&self) -> f64 {
        if self.connection_attempts == 0 {
            0.0
        } else {
            self.successful_connections as f64 / self.connection_attempts as f64
        }
    }

    /// Get age since discovery
    pub fn age(&self) -> Duration {
        self.discovered_at.elapsed()
    }

    /// Record a connection attempt
    pub fn record_connection_attempt(&mut self, success: bool) {
        self.connection_attempts += 1;
        self.last_connection_attempt = Some(Instant::now());

        if success {
            self.successful_connections += 1;
            self.last_successful_connection = Some(Instant::now());
            self.connection_state = ConnectionState::Connected;
            // Reward successful connections
            self.reputation.adjust(10);
        } else {
            self.connection_state = ConnectionState::Failed("Connection failed".to_string());
            // Penalize failed connections
            self.reputation.adjust(-2);
        }
    }

    /// Record misbehavior and adjust reputation
    pub fn record_misbehavior(&mut self, severity: MisbehaviorSeverity) {
        self.reputation.adjust(severity.penalty());

        // Ban peer if reputation drops too low
        if self.reputation.is_banned() {
            self.connection_state = ConnectionState::Banned;
        }
    }

    /// Record data transfer
    pub fn record_data_transfer(&mut self, sent: u64, received: u64) {
        self.bytes_sent += sent;
        self.bytes_received += received;

        // Small reputation bonus for data transfer activity
        let transfer_bonus = ((sent + received) / 1024).min(5) as i32; // Max 5 points per call
        self.reputation.adjust(transfer_bonus);
    }

    /// Update connection state
    pub fn set_connection_state(&mut self, state: ConnectionState) {
        self.connection_state = state;
    }

    /// Add metadata
    pub fn add_metadata(&mut self, key: String, value: String) {
        self.metadata.insert(key, value);
    }

    /// Get metadata
    pub fn get_metadata(&self, key: &str) -> Option<&String> {
        self.metadata.get(key)
    }
}

/// Selection configuration for peer management
#[derive(Debug, Clone)]
pub struct SelectionConfig {
    /// Maximum total connections
    pub max_connections: usize,
    /// Maximum incoming connections
    pub max_incoming: usize,
    /// Maximum outgoing connections
    pub max_outgoing: usize,
    /// Target number of connections to maintain
    pub target_connections: usize,
    /// Minimum reputation required for connection
    pub min_reputation_threshold: i32,
    /// Connection attempt timeout
    pub connection_timeout: Duration,
    /// How often to decay reputation scores
    pub reputation_decay_interval: Duration,
    /// How often to run peer discovery
    pub peer_discovery_interval: Duration,
    /// Maximum connection attempts before giving up on a peer
    pub max_connection_attempts: u32,
    /// Weight for geographic diversity in selection (0.0-1.0)
    pub geographic_diversity_weight: f64,
    /// Weight for stake pool diversity in selection (0.0-1.0)
    pub stake_pool_diversity_weight: f64,
}

impl Default for SelectionConfig {
    fn default() -> Self {
        Self {
            max_connections: 50,
            max_incoming: 25,
            max_outgoing: 25,
            target_connections: 20,
            min_reputation_threshold: -100,
            connection_timeout: Duration::from_secs(30),
            reputation_decay_interval: Duration::from_secs(3600), // 1 hour
            peer_discovery_interval: Duration::from_secs(300),    // 5 minutes
            max_connection_attempts: 3,
            geographic_diversity_weight: 0.3,
            stake_pool_diversity_weight: 0.4,
        }
    }
}

/// Network statistics
#[derive(Debug, Clone)]
pub struct NetworkStats {
    /// Total number of known peers
    pub total_peers: usize,
    /// Number of currently connected peers
    pub connected_peers: usize,
    /// Number of peers available for connection
    pub available_peers: usize,
    /// Number of banned peers
    pub banned_peers: usize,
    /// Average reputation across all peers
    pub avg_reputation: f64,
}

/// Errors that can occur during peer selection
#[derive(Debug)]
pub enum PeerSelectionError {
    /// Peer already exists in the system
    PeerAlreadyExists(PeerId),
    /// Peer not found in the system
    PeerNotFound(PeerId),
    /// Connection limit has been exceeded
    ConnectionLimitExceeded,
    /// Invalid configuration provided
    InvalidConfiguration(String),
    /// Network-related error
    NetworkError(String),
}

impl std::fmt::Display for PeerSelectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PeerSelectionError::PeerAlreadyExists(id) => write!(f, "Peer already exists: {}", id),
            PeerSelectionError::PeerNotFound(id) => write!(f, "Peer not found: {}", id),
            PeerSelectionError::ConnectionLimitExceeded => write!(f, "Connection limit exceeded"),
            PeerSelectionError::InvalidConfiguration(msg) => write!(f, "Invalid peer configuration: {}", msg),
            PeerSelectionError::NetworkError(msg) => write!(f, "Network error: {}", msg),
        }
    }
}

impl std::error::Error for PeerSelectionError {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, IpAddr};

    #[test]
    fn test_peer_id_creation() {
        let id = [1u8; 32];
        let peer_id = PeerId::new(id);
        assert_eq!(peer_id.id, id);
    }

    #[test]
    fn test_peer_id_random() {
        let peer_id1 = PeerId::random(1);
        let peer_id2 = PeerId::random(2);
        assert_ne!(peer_id1, peer_id2);
    }

    #[test]
    fn test_peer_id_hex() {
        let peer_id = PeerId::random(1);
        let hex = peer_id.to_hex();
        let parsed = PeerId::from_hex(&hex).unwrap();
        assert_eq!(peer_id, parsed);
    }

    #[test]
    fn test_connection_state() {
        assert!(ConnectionState::Disconnected.is_available());
        assert!(!ConnectionState::Connected.is_available());
        assert!(!ConnectionState::Banned.is_available());
        assert!(ConnectionState::Failed("error".to_string()).is_available());
    }

    #[test]
    fn test_reputation_score() {
        let mut rep = ReputationScore::default();
        assert_eq!(rep.value(), ReputationScore::INITIAL);

        rep.adjust(100);
        assert_eq!(rep.value(), 100);

        rep.adjust(-600); // Should clamp to MIN
        assert_eq!(rep.value(), ReputationScore::MIN);

        rep.adjust(2000); // Should clamp to MAX
        assert_eq!(rep.value(), ReputationScore::MAX);
    }

    #[test]
    fn test_reputation_banned() {
        let mut rep = ReputationScore::new(-450);
        assert!(rep.is_banned());

        rep.adjust(100);
        assert!(!rep.is_banned());
    }

    #[test]
    fn test_misbehavior_penalties() {
        assert_eq!(MisbehaviorSeverity::Minor.penalty(), -5);
        assert_eq!(MisbehaviorSeverity::Moderate.penalty(), -15);
        assert_eq!(MisbehaviorSeverity::Serious.penalty(), -35);
        assert_eq!(MisbehaviorSeverity::Critical.penalty(), -75);
    }

    #[test]
    fn test_peer_info_creation() {
        let peer_id = PeerId::random(1);
        let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 3000);
        let peer = PeerInfo::new(peer_id.clone(), address);

        assert_eq!(peer.peer_id, peer_id);
        assert_eq!(peer.address, address);
        assert_eq!(peer.connection_state, ConnectionState::Disconnected);
        assert!(peer.is_available());
    }

    #[test]
    fn test_peer_connection_attempts() {
        let mut peer = PeerInfo::new(
            PeerId::random(1),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 3000)
        );

        let initial_rep = peer.reputation.value();

        // Successful connection
        peer.record_connection_attempt(true);
        assert_eq!(peer.connection_attempts, 1);
        assert_eq!(peer.successful_connections, 1);
        assert!(peer.reputation.value() > initial_rep);
        assert_eq!(peer.connection_state, ConnectionState::Connected);

        // Failed connection
        peer.record_connection_attempt(false);
        assert_eq!(peer.connection_attempts, 2);
        assert_eq!(peer.successful_connections, 1);
        assert_eq!(peer.connection_success_rate(), 0.5);
    }

    #[test]
    fn test_peer_misbehavior() {
        let mut peer = PeerInfo::new(
            PeerId::random(1),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 3000)
        );

        let initial_rep = peer.reputation.value();

        // Record minor misbehavior
        peer.record_misbehavior(MisbehaviorSeverity::Minor);
        assert!(peer.reputation.value() < initial_rep);

        // Multiple critical misbehaviors should lead to ban
        for _ in 0..6 {
            peer.record_misbehavior(MisbehaviorSeverity::Critical);
        }
        assert!(peer.reputation.is_banned());
        assert_eq!(peer.connection_state, ConnectionState::Banned);
    }

    #[test]
    fn test_peer_data_transfer() {
        let mut peer = PeerInfo::new(
            PeerId::random(1),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 3000)
        );

        let initial_rep = peer.reputation.value();

        peer.record_data_transfer(1024, 2048);
        assert_eq!(peer.bytes_sent, 1024);
        assert_eq!(peer.bytes_received, 2048);
        assert!(peer.reputation.value() > initial_rep);
    }

    #[test]
    fn test_selection_config_default() {
        let config = SelectionConfig::default();
        assert_eq!(config.max_connections, 50);
        assert_eq!(config.target_connections, 20);
        assert_eq!(config.min_reputation_threshold, -100);
        assert_eq!(config.max_connection_attempts, 3);
    }
}
