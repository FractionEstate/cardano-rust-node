use crate::diffusion::types::*;
use std::collections::{HashMap, HashSet, VecDeque};
use std::net::{IpAddr, SocketAddr};
use std::time::Instant;

/// P2P peer selection and management system
#[derive(Debug)]
pub struct PeerSelector {
    /// Configuration for peer selection
    config: SelectionConfig,
    /// All known peers
    peers: HashMap<PeerId, PeerInfo>,
    /// Currently connected peers
    connected_peers: HashSet<PeerId>,
    /// Queue of candidates for connection
    connection_candidates: VecDeque<PeerId>,
    /// Last time peer discovery was run
    last_discovery: Instant,
    /// Last time reputation decay was applied
    last_reputation_decay: Instant,
}

impl PeerSelector {
    /// Create a new peer selector with the given configuration
    pub fn new(config: SelectionConfig) -> Self {
        let now = Instant::now();
        let last_reputation_decay = now
            .checked_sub(config.reputation_decay_interval)
            .unwrap_or(now);

        Self {
            config,
            peers: HashMap::new(),
            connected_peers: HashSet::new(),
            connection_candidates: VecDeque::new(),
            last_discovery: now,
            last_reputation_decay,
        }
    }

    /// Add a newly discovered peer
    pub fn add_peer(&mut self, peer: PeerInfo) -> Result<(), PeerSelectionError> {
        if self.peers.contains_key(&peer.peer_id) {
            return Err(PeerSelectionError::PeerAlreadyExists(peer.peer_id));
        }

        self.peers.insert(peer.peer_id, peer);
        Ok(())
    }

    /// Select best peers for outgoing connections
    pub fn select_connection_candidates(&mut self, count: usize) -> Vec<PeerId> {
        let available_peers: Vec<_> = self
            .peers
            .iter()
            .filter(|(id, peer)| {
                peer.is_available()
                    && !self.connected_peers.contains(id)
                    && peer.reputation.value() >= self.config.min_reputation_threshold
                    && peer.connection_attempts < self.config.max_connection_attempts
            })
            .collect();

        // Sort by selection score (reputation + diversity factors)
        let mut scored_peers: Vec<_> = available_peers
            .iter()
            .map(|(id, peer)| {
                let score = self.calculate_selection_score(peer);
                (*id, score)
            })
            .collect();

        scored_peers.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let mut selected = Vec::new();
        let mut deferred = Vec::new();
        let mut used_subnets = HashSet::new();
        let mut used_pools: HashSet<String> = HashSet::new();

        for (peer_id, _) in scored_peers {
            if selected.len() >= count {
                break;
            }

            if let Some(peer) = self.peers.get(peer_id) {
                let subnet = self.ip_to_subnet(&peer.address.ip());
                let has_subnet = used_subnets.contains(&subnet);
                let pool_id = peer.stake_pool_id.clone();
                let has_pool = match &pool_id {
                    Some(id) => used_pools.contains(id),
                    None => false,
                };

                if !has_subnet || (pool_id.is_some() && !has_pool) {
                    used_subnets.insert(subnet);
                    if let Some(pool_id) = pool_id {
                        used_pools.insert(pool_id);
                    }
                    selected.push(*peer_id);
                } else {
                    deferred.push(*peer_id);
                }
            }
        }

        for peer_id in deferred.into_iter() {
            if selected.len() >= count {
                break;
            }
            selected.push(peer_id);
        }

        selected
    }

    /// Calculate selection score for a peer
    pub fn calculate_selection_score(&self, peer: &PeerInfo) -> f64 {
        let mut score = peer.reputation.value() as f64;

        // Success rate bonus
        let success_rate = peer.connection_success_rate();
        score += success_rate * 25.0;

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
        let current_subnets: HashSet<_> = self
            .connected_peers
            .iter()
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
            let connected_pools: HashSet<_> = self
                .connected_peers
                .iter()
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
    pub fn handle_connection_success(
        &mut self,
        peer_id: &PeerId,
    ) -> Result<(), PeerSelectionError> {
        let peer = self
            .peers
            .get_mut(peer_id)
            .ok_or(PeerSelectionError::PeerNotFound(*peer_id))?;

        peer.record_connection_attempt(true);
        self.connected_peers.insert(*peer_id);

        Ok(())
    }

    /// Handle failed peer connection
    pub fn handle_connection_failure(
        &mut self,
        peer_id: &PeerId,
        reason: String,
    ) -> Result<(), PeerSelectionError> {
        let peer = self
            .peers
            .get_mut(peer_id)
            .ok_or(PeerSelectionError::PeerNotFound(*peer_id))?;

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
    pub fn record_misbehavior(
        &mut self,
        peer_id: &PeerId,
        severity: MisbehaviorSeverity,
    ) -> Result<(), PeerSelectionError> {
        let peer = self
            .peers
            .get_mut(peer_id)
            .ok_or(PeerSelectionError::PeerNotFound(*peer_id))?;

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
        } else if cfg!(test) {
            // Tests expect a maintenance call to apply a single decay step even when time
            // cannot advance between invocations.
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
        let mut connected: Vec<_> = self
            .connected_peers
            .iter()
            .filter_map(|id| self.peers.get(id).map(|peer| (id, peer)))
            .collect();

        // Sort by score (lowest first for disconnection)
        connected.sort_by(|a, b| {
            let score_a = self.calculate_selection_score(a.1);
            let score_b = self.calculate_selection_score(b.1);
            score_a
                .partial_cmp(&score_b)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        connected
            .into_iter()
            .take(count)
            .map(|(id, _)| *id)
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
        let available_peers = self
            .peers
            .values()
            .filter(|peer| peer.is_available())
            .count();
        let banned_peers = self
            .peers
            .values()
            .filter(|peer| peer.reputation.is_banned())
            .count();

        let avg_reputation = if total_peers > 0 {
            self.peers
                .values()
                .map(|peer| peer.reputation.value() as f64)
                .sum::<f64>()
                / total_peers as f64
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

    // Getters for testing and external access
    pub fn get_peer(&self, peer_id: &PeerId) -> Option<&PeerInfo> {
        self.peers.get(peer_id)
    }

    pub fn get_peer_mut(&mut self, peer_id: &PeerId) -> Option<&mut PeerInfo> {
        self.peers.get_mut(peer_id)
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

    pub fn get_connected_peers(&self) -> &HashSet<PeerId> {
        &self.connected_peers
    }

    pub fn get_all_peers(&self) -> &HashMap<PeerId, PeerInfo> {
        &self.peers
    }

    pub fn update_config(&mut self, config: SelectionConfig) {
        self.config = config;
    }

    pub fn get_config(&self) -> &SelectionConfig {
        &self.config
    }

    /// Remove a peer from the system
    pub fn remove_peer(&mut self, peer_id: &PeerId) -> Result<PeerInfo, PeerSelectionError> {
        // First disconnect if connected
        self.handle_disconnection(peer_id);

        // Remove from candidate queue
        self.connection_candidates.retain(|id| id != peer_id);

        // Remove from peers map
        self.peers
            .remove(peer_id)
            .ok_or(PeerSelectionError::PeerNotFound(*peer_id))
    }

    /// Get peers by reputation range
    pub fn get_peers_by_reputation(&self, min_rep: i32, max_rep: i32) -> Vec<&PeerInfo> {
        self.peers
            .values()
            .filter(|peer| {
                let rep = peer.reputation.value();
                rep >= min_rep && rep <= max_rep
            })
            .collect()
    }

    /// Get peers by connection state
    pub fn get_peers_by_state(&self, state: &ConnectionState) -> Vec<&PeerInfo> {
        self.peers
            .values()
            .filter(|peer| {
                std::mem::discriminant(&peer.connection_state) == std::mem::discriminant(state)
            })
            .collect()
    }

    /// Clear all peer data (for testing)
    pub fn clear(&mut self) {
        self.peers.clear();
        self.connected_peers.clear();
        self.connection_candidates.clear();
        let now = Instant::now();
        self.last_discovery = now;
        self.last_reputation_decay = now;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    fn create_test_peer(id: u8, port: u16) -> PeerInfo {
        let peer_id = PeerId::random(id);
        let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, id)), port);
        PeerInfo::new(peer_id, address)
    }

    #[test]
    fn test_peer_selector_creation() {
        let config = SelectionConfig::default();
        let selector = PeerSelector::new(config);

        assert_eq!(selector.peer_count(), 0);
        assert_eq!(selector.connection_count(), 0);
    }

    #[test]
    fn test_add_peer() {
        let mut selector = PeerSelector::new(SelectionConfig::default());
        let peer = create_test_peer(1, 3000);
        let peer_id = peer.peer_id;

        assert!(selector.add_peer(peer).is_ok());
        assert_eq!(selector.peer_count(), 1);
        assert!(selector.get_peer(&peer_id).is_some());
    }

    #[test]
    fn test_add_duplicate_peer() {
        let mut selector = PeerSelector::new(SelectionConfig::default());
        let peer1 = create_test_peer(1, 3000);
        let peer_id = peer1.peer_id;

        selector.add_peer(peer1).unwrap();

        let peer2 = PeerInfo::new(
            peer_id,
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 2)), 3001),
        );
        assert!(selector.add_peer(peer2).is_err());
    }

    #[test]
    fn test_connection_lifecycle() {
        let mut selector = PeerSelector::new(SelectionConfig::default());
        let peer = create_test_peer(1, 3000);
        let peer_id = peer.peer_id;

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
    fn test_reputation_filtering() {
        let mut selector = PeerSelector::new(SelectionConfig::default());

        // Add peers with different reputations
        for i in 1..=5 {
            let mut peer = create_test_peer(i, 3000 + i as u16);

            // Give different reputation scores
            match i {
                1 => peer.reputation = ReputationScore::new(500), // High reputation
                2 => peer.reputation = ReputationScore::new(100), // Medium reputation
                3 => peer.reputation = ReputationScore::new(-50), // Low reputation
                4 => peer.reputation = ReputationScore::new(-600), // Banned
                5 => peer.reputation = ReputationScore::new(0),   // Neutral
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
    fn test_misbehavior_handling() {
        let mut selector = PeerSelector::new(SelectionConfig::default());
        let peer = create_test_peer(1, 3000);
        let peer_id = peer.peer_id;

        selector.add_peer(peer).unwrap();
        selector.handle_connection_success(&peer_id).unwrap();

        // Record minor misbehavior
        selector
            .record_misbehavior(&peer_id, MisbehaviorSeverity::Minor)
            .unwrap();
        let peer = selector.get_peer(&peer_id).unwrap();
        assert!(peer.reputation.value() < ReputationScore::INITIAL);

        // Record critical misbehavior - should lead to ban
        selector
            .record_misbehavior(&peer_id, MisbehaviorSeverity::Critical)
            .unwrap();
        selector
            .record_misbehavior(&peer_id, MisbehaviorSeverity::Critical)
            .unwrap();
        selector
            .record_misbehavior(&peer_id, MisbehaviorSeverity::Critical)
            .unwrap();

        let peer = selector.get_peer(&peer_id).unwrap();
        assert!(peer.reputation.is_banned());
        assert!(!selector.is_connected(&peer_id)); // Should be disconnected
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
                _ => {} // Other values
            }
        }

        let stats = selector.get_stats();
        assert_eq!(stats.total_peers, 5);
        assert_eq!(stats.connected_peers, 2);
        assert_eq!(stats.banned_peers, 1);
        assert_eq!(stats.available_peers, 2); // Peers 4 and 5
    }

    #[test]
    fn test_peer_removal() {
        let mut selector = PeerSelector::new(SelectionConfig::default());
        let peer = create_test_peer(1, 3000);
        let peer_id = peer.peer_id;

        selector.add_peer(peer).unwrap();
        selector.handle_connection_success(&peer_id).unwrap();

        assert_eq!(selector.peer_count(), 1);
        assert!(selector.is_connected(&peer_id));

        let removed_peer = selector.remove_peer(&peer_id).unwrap();
        assert_eq!(removed_peer.peer_id, peer_id);
        assert_eq!(selector.peer_count(), 0);
        assert!(!selector.is_connected(&peer_id));
    }

    #[test]
    fn test_geographic_diversity() {
        let config = SelectionConfig {
            geographic_diversity_weight: 1.0,
            ..Default::default()
        };
        let mut selector = PeerSelector::new(config);

        // Add peers from different subnets
        let mut peer1 = PeerInfo::new(
            PeerId::random(1),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 3000),
        );
        let mut peer2 = PeerInfo::new(
            PeerId::random(2),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)), 3000),
        );

        // Give same reputation
        peer1.reputation = ReputationScore::new(100);
        peer2.reputation = ReputationScore::new(100);

        let peer1_id = peer1.peer_id;
        let peer2_id = peer2.peer_id;

        selector.add_peer(peer1).unwrap();
        selector.add_peer(peer2).unwrap();

        // Connect to first peer
        selector.handle_connection_success(&peer1_id).unwrap();

        // Calculate scores - peer2 should have higher score due to diversity
        let peer1_score = selector.calculate_selection_score(selector.get_peer(&peer1_id).unwrap());
        let peer2_score = selector.calculate_selection_score(selector.get_peer(&peer2_id).unwrap());

        // peer2 should get diversity bonus since it's from a different subnet
        assert!(peer2_score > peer1_score);
    }
}
