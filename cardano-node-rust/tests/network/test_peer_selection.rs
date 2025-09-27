//! P2P Peer Selection Tests
//!
//! Tests for P2P peer selection and network topology management in Cardano.
//! This covers the algorithms and strategies used to:
//! - Discover and connect to optimal peers
//! - Maintain network connectivity and resilience
//! - Manage peer reputation and scoring
//! - Handle connection limits and resource constraints
//! - Optimize network topology for performance and security
//!
//! Based on the Cardano P2P networking specification.

use std::collections::{HashMap, HashSet, VecDeque};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::{Duration, SystemTime, Instant};

/// Unique peer identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PeerId(pub [u8; 32]);

impl PeerId {
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn random(seed: u8) -> Self {
        let mut bytes = [0u8; 32];
        bytes[0] = seed;
        bytes[31] = seed.wrapping_add(1);
        Self::new(bytes)
    }
}

/// Peer connection state
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Authenticated,
    Failed(String),
    Banned,
}

/// Peer reputation score
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ReputationScore(i32);

impl ReputationScore {
    pub const MIN: i32 = -1000;
    pub const MAX: i32 = 1000;
    pub const INITIAL: i32 = 0;

    pub fn new(score: i32) -> Self {
        Self(score.clamp(Self::MIN, Self::MAX))
    }

    pub fn initial() -> Self {
        Self::new(Self::INITIAL)
    }

    pub fn adjust(&mut self, delta: i32) {
        self.0 = (self.0 + delta).clamp(Self::MIN, Self::MAX);
    }

    pub fn value(&self) -> i32 {
        self.0
    }

    pub fn is_trustworthy(&self) -> bool {
        self.0 > 100
    }

    pub fn is_banned(&self) -> bool {
        self.0 <= -500
    }
}

/// Peer information and metadata
#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub peer_id: PeerId,
    pub address: SocketAddr,
    pub connection_state: ConnectionState,
    pub reputation: ReputationScore,
    pub last_seen: SystemTime,
    pub connection_attempts: u32,
    pub successful_connections: u32,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub protocol_version: Option<u32>,
    pub services: HashSet<String>,
    pub is_relay: bool,
    pub stake_pool_id: Option<String>,
}

impl PeerInfo {
    pub fn new(peer_id: PeerId, address: SocketAddr) -> Self {
        Self {
            peer_id,
            address,
            connection_state: ConnectionState::Disconnected,
            reputation: ReputationScore::initial(),
            last_seen: SystemTime::now(),
            connection_attempts: 0,
            successful_connections: 0,
            bytes_sent: 0,
            bytes_received: 0,
            protocol_version: None,
            services: HashSet::new(),
            is_relay: false,
            stake_pool_id: None,
        }
    }

    pub fn is_available(&self) -> bool {
        matches!(self.connection_state, ConnectionState::Disconnected)
            && !self.reputation.is_banned()
    }

    pub fn is_connected(&self) -> bool {
        matches!(self.connection_state,
            ConnectionState::Connected | ConnectionState::Authenticated)
    }

    pub fn connection_success_rate(&self) -> f64 {
        if self.connection_attempts == 0 {
            0.0
        } else {
            self.successful_connections as f64 / self.connection_attempts as f64
        }
    }

    pub fn record_connection_attempt(&mut self, successful: bool) {
        self.connection_attempts += 1;
        if successful {
            self.successful_connections += 1;
            self.connection_state = ConnectionState::Connected;
            self.reputation.adjust(10); // Reward successful connections
        } else {
            self.connection_state = ConnectionState::Failed("Connection failed".to_string());
            self.reputation.adjust(-5); // Penalize failed connections
        }
    }

    pub fn record_data_transfer(&mut self, sent: u64, received: u64) {
        self.bytes_sent += sent;
        self.bytes_received += received;
        self.last_seen = SystemTime::now();

        // Reward active peers
        if sent > 0 || received > 0 {
            self.reputation.adjust(1);
        }
    }

    pub fn record_misbehavior(&mut self, severity: MisbehaviorSeverity) {
        let penalty = match severity {
            MisbehaviorSeverity::Minor => -10,
            MisbehaviorSeverity::Major => -50,
            MisbehaviorSeverity::Critical => -200,
        };
        self.reputation.adjust(penalty);

        if self.reputation.is_banned() {
            self.connection_state = ConnectionState::Banned;
        }
    }

    pub fn age(&self) -> Duration {
        SystemTime::now().duration_since(self.last_seen).unwrap_or_default()
    }
}

/// Severity levels for peer misbehavior
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MisbehaviorSeverity {
    Minor,   // Protocol violations, timeouts
    Major,   // Invalid data, repeated failures
    Critical, // Malicious behavior, attacks
}

/// Peer selection strategy configuration
#[derive(Debug, Clone)]
pub struct SelectionConfig {
    pub max_connections: usize,
    pub max_incoming: usize,
    pub max_outgoing: usize,
    pub target_connections: usize,
    pub min_reputation_threshold: i32,
    pub connection_timeout: Duration,
    pub reputation_decay_interval: Duration,
    pub peer_discovery_interval: Duration,
    pub max_connection_attempts: u32,
    pub geographic_diversity_weight: f64,
    pub stake_pool_diversity_weight: f64,
}

impl SelectionConfig {
    pub fn default() -> Self {
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

/// P2P peer selection and management system
#[derive(Debug)]
pub struct PeerSelector {
    config: SelectionConfig,
    peers: HashMap<PeerId, PeerInfo>,
    connected_peers: HashSet<PeerId>,
    connection_candidates: VecDeque<PeerId>,
    last_discovery: Instant,
    last_reputation_decay: Instant,
}

impl PeerSelector {
    pub fn new(config: SelectionConfig) -> Self {
        let now = Instant::now();
        Self {
            config,
            peers: HashMap::new(),
            connected_peers: HashSet::new(),
            connection_candidates: VecDeque::new(),
            last_discovery: now,
            last_reputation_decay: now,
        }
    }

    /// Add a newly discovered peer
    pub fn add_peer(&mut self, peer: PeerInfo) -> Result<(), PeerSelectionError> {
        if self.peers.contains_key(&peer.peer_id) {
            return Err(PeerSelectionError::PeerAlreadyExists(peer.peer_id));
        }

        self.peers.insert(peer.peer_id.clone(), peer);
        Ok(())
    }

    /// Select best peers for outgoing connections
    pub fn select_connection_candidates(&mut self, count: usize) -> Vec<PeerId> {
        let available_peers: Vec<_> = self.peers.iter()
            .filter(|(id, peer)| {
                peer.is_available()
                    && !self.connected_peers.contains(id)
                    && peer.reputation.value() >= self.config.min_reputation_threshold
                    && peer.connection_attempts < self.config.max_connection_attempts
            })
            .collect();

        // Sort by selection score (reputation + diversity factors)
        let mut scored_peers: Vec<_> = available_peers.iter()
            .map(|(id, peer)| {
                let score = self.calculate_selection_score(peer);
                (*id, score)
            })
            .collect();

        scored_peers.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        scored_peers.into_iter()
            .take(count)
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Calculate selection score for a peer
    fn calculate_selection_score(&self, peer: &PeerInfo) -> f64 {
        let mut score = peer.reputation.value() as f64;

        // Success rate bonus
        let success_rate = peer.connection_success_rate();
        score += success_rate * 100.0;

        // Recency bonus (prefer recently seen peers)
        let age_hours = peer.age().as_secs() as f64 / 3600.0;
        let recency_bonus = (24.0 - age_hours.min(24.0)) * 5.0;
        score += recency_bonus;

        // Geographic diversity bonus
        if self.config.geographic_diversity_weight > 0.0 {
            let diversity_bonus = self.calculate_geographic_diversity_bonus(&peer.address);
            score += diversity_bonus * self.config.geographic_diversity_weight * 100.0;
        }

        // Stake pool diversity bonus
        if self.config.stake_pool_diversity_weight > 0.0 {
            if let Some(_pool_id) = &peer.stake_pool_id {
                let diversity_bonus = self.calculate_stake_pool_diversity_bonus(peer);
                score += diversity_bonus * self.config.stake_pool_diversity_weight * 100.0;
            }
        }

        // Relay preference
        if peer.is_relay {
            score += 50.0;
        }

        score
    }

    fn calculate_geographic_diversity_bonus(&self, address: &SocketAddr) -> f64 {
        // Simplified geographic diversity calculation based on IP ranges
        // In practice, this would use GeoIP databases
        let ip = address.ip();
        let current_subnets: HashSet<_> = self.connected_peers.iter()
            .filter_map(|id| self.peers.get(id))
            .map(|peer| self.ip_to_subnet(&peer.address.ip()))
            .collect();

        let peer_subnet = self.ip_to_subnet(&ip);
        if current_subnets.contains(&peer_subnet) {
            0.0 // No bonus if we already have peers in this subnet
        } else {
            1.0 // Full bonus for new subnet
        }
    }

    fn ip_to_subnet(&self, ip: &IpAddr) -> u32 {
        // Simplified subnet calculation (first two octets for IPv4)
        match ip {
            IpAddr::V4(ipv4) => {
                let octets = ipv4.octets();
                ((octets[0] as u32) << 8) | (octets[1] as u32)
            }
            IpAddr::V6(_) => 0, // Simplified for testing
        }
    }

    fn calculate_stake_pool_diversity_bonus(&self, peer: &PeerInfo) -> f64 {
        if let Some(pool_id) = &peer.stake_pool_id {
            let connected_pools: HashSet<_> = self.connected_peers.iter()
                .filter_map(|id| self.peers.get(id))
                .filter_map(|peer| peer.stake_pool_id.as_ref())
                .collect();

            if connected_pools.contains(pool_id) {
                0.5 // Reduced bonus if we already have this pool
            } else {
                1.0 // Full bonus for new stake pool
            }
        } else {
            0.0 // No bonus for non-pool peers
        }
    }

    /// Handle successful peer connection
    pub fn handle_connection_success(&mut self, peer_id: &PeerId) -> Result<(), PeerSelectionError> {
        let peer = self.peers.get_mut(peer_id)
            .ok_or_else(|| PeerSelectionError::PeerNotFound(peer_id.clone()))?;

        peer.record_connection_attempt(true);
        self.connected_peers.insert(peer_id.clone());

        Ok(())
    }

    /// Handle failed peer connection
    pub fn handle_connection_failure(&mut self, peer_id: &PeerId, reason: String) -> Result<(), PeerSelectionError> {
        let peer = self.peers.get_mut(peer_id)
            .ok_or_else(|| PeerSelectionError::PeerNotFound(peer_id.clone()))?;

        peer.record_connection_attempt(false);
        peer.connection_state = ConnectionState::Failed(reason);

        Ok(())
    }

    /// Handle peer disconnection
    pub fn handle_disconnection(&mut self, peer_id: &PeerId) {
        if let Some(peer) = self.peers.get_mut(peer_id) {
            peer.connection_state = ConnectionState::Disconnected;
        }
        self.connected_peers.remove(peer_id);
    }

    /// Record peer misbehavior
    pub fn record_misbehavior(&mut self, peer_id: &PeerId, severity: MisbehaviorSeverity) -> Result<(), PeerSelectionError> {
        let peer = self.peers.get_mut(peer_id)
            .ok_or_else(|| PeerSelectionError::PeerNotFound(peer_id.clone()))?;

        peer.record_misbehavior(severity);

        if peer.reputation.is_banned() {
            self.handle_disconnection(peer_id);
        }

        Ok(())
    }

    /// Periodic maintenance tasks
    pub fn periodic_maintenance(&mut self) {
        let now = Instant::now();

        // Reputation decay
        if now.duration_since(self.last_reputation_decay) >= self.config.reputation_decay_interval {
            self.decay_reputations();
            self.last_reputation_decay = now;
        }

        // Peer discovery trigger
        if now.duration_since(self.last_discovery) >= self.config.peer_discovery_interval {
            self.trigger_peer_discovery();
            self.last_discovery = now;
        }

        // Connection management
        self.manage_connections();
    }

    fn decay_reputations(&mut self) {
        for peer in self.peers.values_mut() {
            // Gradually decay reputation towards neutral
            let current = peer.reputation.value();
            if current > 0 {
                peer.reputation.adjust(-1);
            } else if current < 0 {
                peer.reputation.adjust(1);
            }
        }
    }

    fn trigger_peer_discovery(&mut self) {
        // In a real implementation, this would trigger DNS queries,
        // peer exchange protocols, etc.
        // For testing, we'll just refresh the candidate list
        self.refresh_connection_candidates();
    }

    fn refresh_connection_candidates(&mut self) {
        let candidates = self.select_connection_candidates(10);
        self.connection_candidates.clear();
        self.connection_candidates.extend(candidates);
    }

    fn manage_connections(&mut self) {
        let current_connections = self.connected_peers.len();

        // Disconnect excess peers
        if current_connections > self.config.max_connections {
            let excess = current_connections - self.config.max_connections;
            let to_disconnect = self.select_peers_to_disconnect(excess);
            for peer_id in to_disconnect {
                self.handle_disconnection(&peer_id);
            }
        }

        // Initiate new connections if below target
        if current_connections < self.config.target_connections {
            let needed = self.config.target_connections - current_connections;
            let candidates = self.select_connection_candidates(needed);
            self.connection_candidates.extend(candidates);
        }
    }

    fn select_peers_to_disconnect(&self, count: usize) -> Vec<PeerId> {
        let mut connected: Vec<_> = self.connected_peers.iter()
            .filter_map(|id| self.peers.get(id).map(|peer| (id, peer)))
            .collect();

        // Sort by score (lowest first for disconnection)
        connected.sort_by(|a, b| {
            let score_a = self.calculate_selection_score(a.1);
            let score_b = self.calculate_selection_score(b.1);
            score_a.partial_cmp(&score_b).unwrap_or(std::cmp::Ordering::Equal)
        });

        connected.into_iter()
            .take(count)
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Get next connection candidate
    pub fn get_next_candidate(&mut self) -> Option<PeerId> {
        self.connection_candidates.pop_front()
    }

    /// Get network statistics
    pub fn get_stats(&self) -> NetworkStats {
        let total_peers = self.peers.len();
        let connected_peers = self.connected_peers.len();
        let available_peers = self.peers.values()
            .filter(|peer| peer.is_available())
            .count();
        let banned_peers = self.peers.values()
            .filter(|peer| peer.reputation.is_banned())
            .count();

        let avg_reputation = if total_peers > 0 {
            self.peers.values()
                .map(|peer| peer.reputation.value() as f64)
                .sum::<f64>() / total_peers as f64
        } else {
            0.0
        };

        NetworkStats {
            total_peers,
            connected_peers,
            available_peers,
            banned_peers,
            avg_reputation,
        }
    }

    // Getters for testing
    pub fn get_peer(&self, peer_id: &PeerId) -> Option<&PeerInfo> {
        self.peers.get(peer_id)
    }

    pub fn is_connected(&self, peer_id: &PeerId) -> bool {
        self.connected_peers.contains(peer_id)
    }

    pub fn connection_count(&self) -> usize {
        self.connected_peers.len()
    }

    pub fn peer_count(&self) -> usize {
        self.peers.len()
    }
}

/// Network statistics
#[derive(Debug, Clone)]
pub struct NetworkStats {
    pub total_peers: usize,
    pub connected_peers: usize,
    pub available_peers: usize,
    pub banned_peers: usize,
    pub avg_reputation: f64,
}

/// Peer selection errors
#[derive(Debug)]
pub enum PeerSelectionError {
    PeerAlreadyExists(PeerId),
    PeerNotFound(PeerId),
    ConnectionLimitExceeded,
    InvalidConfiguration(String),
    NetworkError(String),
}

impl std::fmt::Display for PeerSelectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PeerSelectionError::PeerAlreadyExists(id) => write!(f, "Peer already exists: {:?}", id),
            PeerSelectionError::PeerNotFound(id) => write!(f, "Peer not found: {:?}", id),
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

    fn create_test_peer(id: u8, port: u16) -> PeerInfo {
        let peer_id = PeerId::random(id);
        let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, id)), port);
        PeerInfo::new(peer_id, address)
    }

    fn create_test_peer_with_pool(id: u8, port: u16, pool_id: String) -> PeerInfo {
        let mut peer = create_test_peer(id, port);
        peer.stake_pool_id = Some(pool_id);
        peer.is_relay = true;
        peer
    }

    #[test]
    fn test_peer_reputation_scoring() {
        let mut peer = create_test_peer(1, 3000);

        // Initial reputation
        assert_eq!(peer.reputation.value(), ReputationScore::INITIAL);

        // Successful connections increase reputation
        peer.record_connection_attempt(true);
        assert!(peer.reputation.value() > ReputationScore::INITIAL);

        // Failed connections decrease reputation
        let prev_score = peer.reputation.value();
        peer.record_connection_attempt(false);
        assert!(peer.reputation.value() < prev_score);

        // Misbehavior penalties
        peer.record_misbehavior(MisbehaviorSeverity::Minor);
        assert!(peer.reputation.value() < prev_score - 5);

        peer.record_misbehavior(MisbehaviorSeverity::Critical);
        assert!(peer.reputation.value() < prev_score - 50);
    }

    #[test]
    fn test_peer_reputation_bounds() {
        let mut peer = create_test_peer(1, 3000);

        // Test upper bound
        for _ in 0..200 {
            peer.reputation.adjust(100);
        }
        assert_eq!(peer.reputation.value(), ReputationScore::MAX);

        // Test lower bound
        for _ in 0..200 {
            peer.reputation.adjust(-100);
        }
        assert_eq!(peer.reputation.value(), ReputationScore::MIN);
    }

    #[test]
    fn test_peer_selection_basic() {
        let mut selector = PeerSelector::new(SelectionConfig::default());

        // Add some test peers
        for i in 1..=5 {
            let peer = create_test_peer(i, 3000 + i as u16);
            selector.add_peer(peer).unwrap();
        }

        assert_eq!(selector.peer_count(), 5);

        // Select connection candidates
        let candidates = selector.select_connection_candidates(3);
        assert_eq!(candidates.len(), 3);

        // All selected peers should be available
        for peer_id in &candidates {
            let peer = selector.get_peer(peer_id).unwrap();
            assert!(peer.is_available());
        }
    }

    #[test]
    fn test_peer_connection_lifecycle() {
        let mut selector = PeerSelector::new(SelectionConfig::default());
        let peer = create_test_peer(1, 3000);
        let peer_id = peer.peer_id.clone();

        selector.add_peer(peer).unwrap();

        // Initially disconnected
        assert!(!selector.is_connected(&peer_id));
        assert_eq!(selector.connection_count(), 0);

        // Successful connection
        selector.handle_connection_success(&peer_id).unwrap();
        assert!(selector.is_connected(&peer_id));
        assert_eq!(selector.connection_count(), 1);

        // Disconnection
        selector.handle_disconnection(&peer_id);
        assert!(!selector.is_connected(&peer_id));
        assert_eq!(selector.connection_count(), 0);
    }

    #[test]
    fn test_peer_selection_reputation_filtering() {
        let mut selector = PeerSelector::new(SelectionConfig::default());

        // Add peers with different reputations
        for i in 1..=5 {
            let mut peer = create_test_peer(i, 3000 + i as u16);

            // Give different reputation scores
            match i {
                1 => peer.reputation = ReputationScore::new(500),  // High reputation
                2 => peer.reputation = ReputationScore::new(100),  // Medium reputation
                3 => peer.reputation = ReputationScore::new(-50),  // Low reputation
                4 => peer.reputation = ReputationScore::new(-600), // Banned
                5 => peer.reputation = ReputationScore::new(0),    // Neutral
                _ => {}
            }

            selector.add_peer(peer).unwrap();
        }

        // Should only select peers above reputation threshold
        let candidates = selector.select_connection_candidates(5);

        // Should not include banned peer (id=4) or very low reputation peer (id=3)
        for peer_id in &candidates {
            let peer = selector.get_peer(peer_id).unwrap();
            assert!(peer.reputation.value() >= selector.config.min_reputation_threshold);
            assert!(!peer.reputation.is_banned());
        }
    }

    #[test]
    fn test_peer_selection_diversity() {
        let mut config = SelectionConfig::default();
        config.geographic_diversity_weight = 1.0;
        config.stake_pool_diversity_weight = 1.0;

        let mut selector = PeerSelector::new(config);

        // Add peers from different subnets and stake pools
        let peers = vec![
            (1, "192.168.1.1", "pool1"),
            (2, "192.168.1.2", "pool1"), // Same subnet and pool as peer 1
            (3, "10.0.0.1", "pool2"),    // Different subnet and pool
            (4, "172.16.0.1", "pool3"),  // Different subnet and pool
            (5, "192.168.2.1", "pool2"), // Same pool as peer 3, different subnet
        ];

        for (id, ip_str, pool_id) in peers {
            let ip: Ipv4Addr = ip_str.parse().unwrap();
            let address = SocketAddr::new(IpAddr::V4(ip), 3000);
            let mut peer = PeerInfo::new(PeerId::random(id), address);
            peer.stake_pool_id = Some(pool_id.to_string());
            peer.is_relay = true;
            peer.reputation = ReputationScore::new(100); // Give all peers good reputation

            selector.add_peer(peer).unwrap();
        }

        // Select candidates - should prefer diversity
        let candidates = selector.select_connection_candidates(3);

        // Verify geographic diversity
        let selected_subnets: HashSet<_> = candidates.iter()
            .filter_map(|id| selector.get_peer(id))
            .map(|peer| selector.ip_to_subnet(&peer.address.ip()))
            .collect();

        // Should have peers from different subnets
        assert!(selected_subnets.len() >= 2);
    }

    #[test]
    fn test_peer_selection_connection_limits() {
        let mut config = SelectionConfig::default();
        config.max_connections = 3;
        config.target_connections = 2;

        let mut selector = PeerSelector::new(config);

        // Add more peers than the connection limit
        for i in 1..=5 {
            let peer = create_test_peer(i, 3000 + i as u16);
            selector.add_peer(peer).unwrap();
        }

        // Connect to maximum number of peers
        let candidates = selector.select_connection_candidates(5);
        for (i, peer_id) in candidates.iter().take(4).enumerate() {
            selector.handle_connection_success(peer_id).unwrap();

            // Should not exceed max connections
            if i < 3 {
                assert!(selector.connection_count() <= selector.config.max_connections);
            }
        }

        // Test maintenance - should disconnect excess peers
        selector.manage_connections();
        assert!(selector.connection_count() <= selector.config.max_connections);
    }

    #[test]
    fn test_peer_misbehavior_handling() {
        let mut selector = PeerSelector::new(SelectionConfig::default());
        let peer = create_test_peer(1, 3000);
        let peer_id = peer.peer_id.clone();

        selector.add_peer(peer).unwrap();
        selector.handle_connection_success(&peer_id).unwrap();

        // Record minor misbehavior
        selector.record_misbehavior(&peer_id, MisbehaviorSeverity::Minor).unwrap();
        let peer = selector.get_peer(&peer_id).unwrap();
        assert!(peer.reputation.value() < ReputationScore::INITIAL);

        // Record critical misbehavior - should lead to ban
        selector.record_misbehavior(&peer_id, MisbehaviorSeverity::Critical).unwrap();
        selector.record_misbehavior(&peer_id, MisbehaviorSeverity::Critical).unwrap();
        selector.record_misbehavior(&peer_id, MisbehaviorSeverity::Critical).unwrap();

        let peer = selector.get_peer(&peer_id).unwrap();
        assert!(peer.reputation.is_banned());
        assert!(!selector.is_connected(&peer_id)); // Should be disconnected
    }

    #[test]
    fn test_peer_candidate_queue_management() {
        let mut selector = PeerSelector::new(SelectionConfig::default());

        // Add test peers
        for i in 1..=5 {
            let peer = create_test_peer(i, 3000 + i as u16);
            selector.add_peer(peer).unwrap();
        }

        // Refresh candidates
        selector.refresh_connection_candidates();

        // Should have candidates queued
        let first_candidate = selector.get_next_candidate();
        assert!(first_candidate.is_some());

        let second_candidate = selector.get_next_candidate();
        assert!(second_candidate.is_some());

        // Candidates should be different
        assert_ne!(first_candidate, second_candidate);
    }

    #[test]
    fn test_peer_selection_success_rate_preference() {
        let mut selector = PeerSelector::new(SelectionConfig::default());

        // Create peers with different success rates
        let mut peer1 = create_test_peer(1, 3001);
        let mut peer2 = create_test_peer(2, 3002);

        // Peer1: 80% success rate (4/5)
        peer1.connection_attempts = 5;
        peer1.successful_connections = 4;

        // Peer2: 50% success rate (1/2)
        peer2.connection_attempts = 2;
        peer2.successful_connections = 1;

        // Give same reputation to both
        peer1.reputation = ReputationScore::new(100);
        peer2.reputation = ReputationScore::new(100);

        let peer1_id = peer1.peer_id.clone();
        let peer2_id = peer2.peer_id.clone();

        selector.add_peer(peer1).unwrap();
        selector.add_peer(peer2).unwrap();

        // Select one candidate - should prefer peer1 with higher success rate
        let candidates = selector.select_connection_candidates(1);
        assert_eq!(candidates.len(), 1);

        // Calculate scores manually to verify
        let peer1_score = selector.calculate_selection_score(selector.get_peer(&peer1_id).unwrap());
        let peer2_score = selector.calculate_selection_score(selector.get_peer(&peer2_id).unwrap());

        assert!(peer1_score > peer2_score,
            "Peer1 score ({}) should be higher than Peer2 score ({})", peer1_score, peer2_score);
    }

    #[test]
    fn test_reputation_decay() {
        let mut selector = PeerSelector::new(SelectionConfig::default());
        let mut peer = create_test_peer(1, 3000);

        // Set high reputation
        peer.reputation = ReputationScore::new(500);
        let peer_id = peer.peer_id.clone();
        selector.add_peer(peer).unwrap();

        // Force reputation decay
        selector.decay_reputations();

        let peer = selector.get_peer(&peer_id).unwrap();
        assert_eq!(peer.reputation.value(), 499); // Should decay by 1

        // Set negative reputation
        selector.peers.get_mut(&peer_id).unwrap().reputation = ReputationScore::new(-300);
        selector.decay_reputations();

        let peer = selector.get_peer(&peer_id).unwrap();
        assert_eq!(peer.reputation.value(), -299); // Should improve by 1
    }

    #[test]
    fn test_network_statistics() {
        let mut selector = PeerSelector::new(SelectionConfig::default());

        // Add peers with different states
        for i in 1..=5 {
            let mut peer = create_test_peer(i, 3000 + i as u16);
            match i {
                1 | 2 => {
                    // Connected peers
                    selector.add_peer(peer.clone()).unwrap();
                    selector.handle_connection_success(&peer.peer_id).unwrap();
                }
                3 => {
                    // Banned peer
                    peer.reputation = ReputationScore::new(-600);
                    selector.add_peer(peer).unwrap();
                }
                4 | 5 => {
                    // Available peers
                    selector.add_peer(peer).unwrap();
                }
            }
        }

        let stats = selector.get_stats();
        assert_eq!(stats.total_peers, 5);
        assert_eq!(stats.connected_peers, 2);
        assert_eq!(stats.banned_peers, 1);
        assert_eq!(stats.available_peers, 2); // Peers 4 and 5
    }

    #[test]
    fn test_peer_selection_error_handling() {
        let mut selector = PeerSelector::new(SelectionConfig::default());

        // Test adding duplicate peer
        let peer1 = create_test_peer(1, 3000);
        let peer_id = peer1.peer_id.clone();

        selector.add_peer(peer1).unwrap();

        let peer2 = PeerInfo::new(peer_id.clone(), SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 2)), 3001));
        assert!(selector.add_peer(peer2).is_err());

        // Test operations on non-existent peer
        let fake_peer_id = PeerId::random(99);
        assert!(selector.handle_connection_success(&fake_peer_id).is_err());
        assert!(selector.handle_connection_failure(&fake_peer_id, "test".to_string()).is_err());
        assert!(selector.record_misbehavior(&fake_peer_id, MisbehaviorSeverity::Minor).is_err());
    }

    #[test]
    fn test_peer_connection_attempts_limit() {
        let mut config = SelectionConfig::default();
        config.max_connection_attempts = 2;

        let mut selector = PeerSelector::new(config);
        let peer = create_test_peer(1, 3000);
        let peer_id = peer.peer_id.clone();

        selector.add_peer(peer).unwrap();

        // Fail connections up to the limit
        for _ in 0..2 {
            selector.handle_connection_failure(&peer_id, "Connection refused".to_string()).unwrap();
        }

        // Should not be selected anymore
        let candidates = selector.select_connection_candidates(5);
        assert!(!candidates.contains(&peer_id));
    }

    #[test]
    fn test_peer_data_transfer_tracking() {
        let mut selector = PeerSelector::new(SelectionConfig::default());
        let mut peer = create_test_peer(1, 3000);
        let peer_id = peer.peer_id.clone();

        let initial_reputation = peer.reputation.value();
        selector.add_peer(peer).unwrap();

        // Simulate data transfer
        selector.peers.get_mut(&peer_id).unwrap().record_data_transfer(1024, 2048);

        let peer = selector.get_peer(&peer_id).unwrap();
        assert_eq!(peer.bytes_sent, 1024);
        assert_eq!(peer.bytes_received, 2048);
        assert!(peer.reputation.value() > initial_reputation); // Should get reputation bonus
    }

    #[test]
    fn test_peer_age_calculation() {
        use std::thread;

        let peer = create_test_peer(1, 3000);
        let initial_age = peer.age();

        // Sleep briefly to ensure time passage
        thread::sleep(Duration::from_millis(10));

        let later_age = peer.age();
        assert!(later_age > initial_age);
    }

    #[test]
    fn test_periodic_maintenance() {
        let mut config = SelectionConfig::default();
        config.reputation_decay_interval = Duration::from_millis(1); // Very short for testing
        config.peer_discovery_interval = Duration::from_millis(1);

        let mut selector = PeerSelector::new(config);

        // Add a peer with high reputation
        let mut peer = create_test_peer(1, 3000);
        peer.reputation = ReputationScore::new(500);
        let peer_id = peer.peer_id.clone();
        selector.add_peer(peer).unwrap();

        let initial_reputation = selector.get_peer(&peer_id).unwrap().reputation.value();

        // Sleep to ensure intervals have passed
        std::thread::sleep(Duration::from_millis(5));

        // Run maintenance
        selector.periodic_maintenance();

        // Reputation should have decayed
        let new_reputation = selector.get_peer(&peer_id).unwrap().reputation.value();
        assert!(new_reputation < initial_reputation);
    }
}
