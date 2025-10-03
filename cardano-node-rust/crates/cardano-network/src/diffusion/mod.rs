//! P2P Diffusion and Peer Selection Module
//!
//! This module implements the P2P diffusion layer for the Cardano Node Rust implementation.
//! It provides peer selection algorithms, reputation management, and gossip protocols
//! for efficient peer discovery and network topology optimization.
//!
//! ## Features
//!
//! - **Peer Selection**: Smart peer selection based on reputation, geographic diversity,
//!   and stake pool distribution
//! - **Reputation System**: Dynamic reputation scoring with misbehavior tracking and
//!   automatic decay
//! - **Gossip Protocol**: Peer advertisement and discovery through decentralized gossip
//! - **Connection Management**: Automatic connection lifecycle management with limits
//!   and quality-based prioritization
//!
//! ## Usage
//!
//! ```rust
//! use cardano_network::diffusion::{PeerSelector, SelectionConfig, PeerGossip, GossipConfig};
//! use std::time::Duration;
//!
//! // Create peer selector with custom configuration
//! let mut config = SelectionConfig::default();
//! config.max_connections = 100;
//! config.target_connections = 50;
//! config.geographic_diversity_weight = 0.5;
//!
//! let mut selector = PeerSelector::new(config);
//!
//! // Add discovered peers
//! // selector.add_peer(peer_info)?;
//!
//! // Select peers for connection
//! let candidates = selector.select_connection_candidates(10);
//!
//! // Handle connection events
//! // selector.handle_connection_success(&peer_id)?;
//! // selector.handle_connection_failure(&peer_id, "Connection timeout".to_string())?;
//!
//! // Run periodic maintenance
//! selector.periodic_maintenance();
//! ```

pub mod gossip;
pub mod selector;
pub mod types;

#[cfg(test)]
mod tests;

// Re-export main types and traits for easy access
pub use types::{
    ConnectionState, MisbehaviorSeverity, NetworkStats, PeerId, PeerInfo, PeerSelectionError,
    ReputationScore, SelectionConfig,
};

pub use selector::PeerSelector;

pub use gossip::{
    AdvertisedPeer, GossipConfig, GossipError, GossipStats, PeerAdvertisement, PeerDiscovery,
    PeerGossip, ProcessResult,
};

/// Main P2P diffusion manager that combines peer selection and gossip protocols
#[derive(Debug)]
pub struct P2PDiffusion {
    /// Peer selection and management
    selector: PeerSelector,
    /// Peer gossip and discovery
    discovery: PeerDiscovery,
}

impl P2PDiffusion {
    /// Create a new P2P diffusion manager
    pub fn new(
        selection_config: SelectionConfig,
        gossip_config: GossipConfig,
        local_peer_id: PeerId,
        dns_seeds: Vec<String>,
    ) -> Self {
        let selector = PeerSelector::new(selection_config);
        let discovery = PeerDiscovery::new(gossip_config, local_peer_id, dns_seeds);

        Self {
            selector,
            discovery,
        }
    }

    /// Get mutable reference to peer selector
    pub fn selector_mut(&mut self) -> &mut PeerSelector {
        &mut self.selector
    }

    /// Get reference to peer selector
    pub fn selector(&self) -> &PeerSelector {
        &self.selector
    }

    /// Get mutable reference to peer discovery
    pub fn discovery_mut(&mut self) -> &mut PeerDiscovery {
        &mut self.discovery
    }

    /// Get reference to peer discovery
    pub fn discovery(&self) -> &PeerDiscovery {
        &self.discovery
    }

    /// Bootstrap initial peers through DNS discovery
    pub async fn bootstrap(&mut self) -> Result<usize, Box<dyn std::error::Error>> {
        let peers = self.discovery.dns_bootstrap().await?;
        let mut added = 0;

        for peer in peers {
            if self.selector.add_peer(peer).is_ok() {
                added += 1;
            }
        }

        Ok(added)
    }

    /// Process a received peer advertisement
    pub fn process_peer_advertisement(
        &mut self,
        advertisement: PeerAdvertisement,
    ) -> Result<ProcessResult, GossipError> {
        let result = self
            .discovery
            .gossip_mut()
            .process_advertisement(advertisement)?;

        // Add new peers to selector if any
        if let ProcessResult::NewPeers { ref peers, .. } = result {
            for peer in peers {
                let _ = self.selector.add_peer(peer.clone());
            }
        }

        Ok(result)
    }

    /// Create peer advertisement for known peers
    pub fn create_peer_advertisement(&mut self) -> Option<PeerAdvertisement> {
        let peers = self.selector.get_all_peers();
        self.discovery.gossip_mut().create_advertisement(peers)
    }

    /// Select peers for new connections
    pub fn select_connection_candidates(&mut self, count: usize) -> Vec<PeerId> {
        self.selector.select_connection_candidates(count)
    }

    /// Handle successful peer connection
    pub fn handle_connection_success(
        &mut self,
        peer_id: &PeerId,
    ) -> Result<(), PeerSelectionError> {
        self.selector.handle_connection_success(peer_id)?;

        // Add peer to gossip advertisement list
        self.discovery.gossip_mut().add_peer_to_advertise(*peer_id);

        Ok(())
    }

    /// Handle failed peer connection
    pub fn handle_connection_failure(
        &mut self,
        peer_id: &PeerId,
        reason: String,
    ) -> Result<(), PeerSelectionError> {
        self.selector.handle_connection_failure(peer_id, reason)
    }

    /// Handle peer disconnection
    pub fn handle_disconnection(&mut self, peer_id: &PeerId) {
        self.selector.handle_disconnection(peer_id);

        // Remove from gossip advertisement list
        self.discovery
            .gossip_mut()
            .remove_peer_from_advertise(peer_id);
    }

    /// Record peer misbehavior
    pub fn record_misbehavior(
        &mut self,
        peer_id: &PeerId,
        severity: MisbehaviorSeverity,
    ) -> Result<(), PeerSelectionError> {
        self.selector.record_misbehavior(peer_id, severity)
    }

    /// Run periodic maintenance for both selector and discovery
    pub fn periodic_maintenance(&mut self) {
        self.selector.periodic_maintenance();
        self.discovery.periodic_maintenance();
    }

    /// Get comprehensive network statistics
    pub fn get_network_stats(&self) -> NetworkStats {
        self.selector.get_stats()
    }

    /// Get gossip statistics
    pub fn get_gossip_stats(&self) -> &GossipStats {
        self.discovery.gossip().get_stats()
    }

    /// Check if peer is connected
    pub fn is_connected(&self, peer_id: &PeerId) -> bool {
        self.selector.is_connected(peer_id)
    }

    /// Get peer information
    pub fn get_peer(&self, peer_id: &PeerId) -> Option<&PeerInfo> {
        self.selector.get_peer(peer_id)
    }

    /// Get all connected peers
    pub fn get_connected_peers(&self) -> Vec<PeerId> {
        self.selector
            .get_connected_peers()
            .iter()
            .cloned()
            .collect()
    }

    /// Get total number of known peers
    pub fn peer_count(&self) -> usize {
        self.selector.peer_count()
    }

    /// Get number of connected peers
    pub fn connection_count(&self) -> usize {
        self.selector.connection_count()
    }

    /// Update selection configuration
    pub fn update_selection_config(&mut self, config: SelectionConfig) {
        self.selector.update_config(config);
    }

    /// Clear all peer data (for testing)
    pub fn clear(&mut self) {
        self.selector.clear();
    }
}

/// Builder for creating P2P diffusion instances with custom configurations
pub struct P2PDiffusionBuilder {
    selection_config: SelectionConfig,
    gossip_config: GossipConfig,
    local_peer_id: Option<PeerId>,
    dns_seeds: Vec<String>,
}

impl Default for P2PDiffusionBuilder {
    fn default() -> Self {
        Self {
            selection_config: SelectionConfig::default(),
            gossip_config: GossipConfig::default(),
            local_peer_id: None,
            dns_seeds: vec![
                "mainnet.cardano.org:3001".to_string(),
                "seed.cardano.org:3001".to_string(),
            ],
        }
    }
}

impl P2PDiffusionBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Set peer selection configuration
    pub fn with_selection_config(mut self, config: SelectionConfig) -> Self {
        self.selection_config = config;
        self
    }

    /// Set gossip configuration
    pub fn with_gossip_config(mut self, config: GossipConfig) -> Self {
        self.gossip_config = config;
        self
    }

    /// Set local peer ID
    pub fn with_local_peer_id(mut self, peer_id: PeerId) -> Self {
        self.local_peer_id = Some(peer_id);
        self
    }

    /// Set DNS seed addresses
    pub fn with_dns_seeds(mut self, seeds: Vec<String>) -> Self {
        self.dns_seeds = seeds;
        self
    }

    /// Add a DNS seed address
    pub fn add_dns_seed(mut self, seed: String) -> Self {
        self.dns_seeds.push(seed);
        self
    }

    /// Set connection limits
    pub fn with_connection_limits(mut self, max: usize, target: usize) -> Self {
        self.selection_config.max_connections = max;
        self.selection_config.target_connections = target;
        self
    }

    /// Set diversity weights
    pub fn with_diversity_weights(mut self, geographic: f64, stake_pool: f64) -> Self {
        self.selection_config.geographic_diversity_weight = geographic;
        self.selection_config.stake_pool_diversity_weight = stake_pool;
        self
    }

    /// Set reputation threshold
    pub fn with_reputation_threshold(mut self, threshold: i32) -> Self {
        self.selection_config.min_reputation_threshold = threshold;
        self
    }

    /// Build the P2P diffusion instance
    pub fn build(self) -> Result<P2PDiffusion, Box<dyn std::error::Error>> {
        let local_peer_id = self.local_peer_id.ok_or("Local peer ID must be set")?;

        Ok(P2PDiffusion::new(
            self.selection_config,
            self.gossip_config,
            local_peer_id,
            self.dns_seeds,
        ))
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    #[test]
    fn test_p2p_diffusion_creation() {
        let diffusion = P2PDiffusionBuilder::new()
            .with_local_peer_id(PeerId::random(1))
            .with_connection_limits(50, 25)
            .build()
            .unwrap();

        assert_eq!(diffusion.peer_count(), 0);
        assert_eq!(diffusion.connection_count(), 0);
    }

    #[test]
    fn test_p2p_diffusion_peer_lifecycle() {
        let mut diffusion = P2PDiffusionBuilder::new()
            .with_local_peer_id(PeerId::random(1))
            .build()
            .unwrap();

        // Add a peer
        let peer = PeerInfo::new(
            PeerId::random(2),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 3000),
        );
        let peer_id = peer.peer_id;

        diffusion.selector_mut().add_peer(peer).unwrap();
        assert_eq!(diffusion.peer_count(), 1);

        // Select for connection
        let candidates = diffusion.select_connection_candidates(1);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0], peer_id);

        // Handle connection success
        diffusion.handle_connection_success(&peer_id).unwrap();
        assert!(diffusion.is_connected(&peer_id));
        assert_eq!(diffusion.connection_count(), 1);

        // Handle disconnection
        diffusion.handle_disconnection(&peer_id);
        assert!(!diffusion.is_connected(&peer_id));
        assert_eq!(diffusion.connection_count(), 0);
    }

    #[test]
    fn test_p2p_diffusion_advertisement() {
        let mut diffusion = P2PDiffusionBuilder::new()
            .with_local_peer_id(PeerId::random(1))
            .build()
            .unwrap();

        // Add and connect some peers
        for i in 2..=4 {
            let peer = PeerInfo::new(
                PeerId::random(i),
                SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, i)), 3000),
            );
            let peer_id = peer.peer_id;

            diffusion.selector_mut().add_peer(peer).unwrap();
            diffusion.handle_connection_success(&peer_id).unwrap();
        }

        // Create advertisement
        let advertisement = diffusion.create_peer_advertisement();
        assert!(advertisement.is_some());

        let ad = advertisement.unwrap();
        assert!(!ad.peers.is_empty());
        assert!(ad.ttl > 0);
    }

    #[test]
    fn test_p2p_diffusion_misbehavior() {
        let mut diffusion = P2PDiffusionBuilder::new()
            .with_local_peer_id(PeerId::random(1))
            .build()
            .unwrap();

        let peer = PeerInfo::new(
            PeerId::random(2),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 3000),
        );
        let peer_id = peer.peer_id;

        diffusion.selector_mut().add_peer(peer).unwrap();
        diffusion.handle_connection_success(&peer_id).unwrap();

        // Record misbehavior
        diffusion
            .record_misbehavior(&peer_id, MisbehaviorSeverity::Critical)
            .unwrap();

        let peer = diffusion.get_peer(&peer_id).unwrap();
        assert!(peer.reputation.value() < ReputationScore::INITIAL);
    }

    #[test]
    fn test_p2p_diffusion_statistics() {
        let mut diffusion = P2PDiffusionBuilder::new()
            .with_local_peer_id(PeerId::random(1))
            .build()
            .unwrap();

        // Add some peers
        for i in 2..=5 {
            let peer = PeerInfo::new(
                PeerId::random(i),
                SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, i)), 3000),
            );
            diffusion.selector_mut().add_peer(peer).unwrap();
        }

        // Connect to some
        let candidates = diffusion.select_connection_candidates(2);
        for peer_id in &candidates {
            diffusion.handle_connection_success(peer_id).unwrap();
        }

        let stats = diffusion.get_network_stats();
        assert_eq!(stats.total_peers, 4);
        assert_eq!(stats.connected_peers, 2);
        assert_eq!(stats.available_peers, 2);

        let gossip_stats = diffusion.get_gossip_stats();
        assert_eq!(gossip_stats.advertisements_sent, 0); // No advertisements created yet
    }

    #[tokio::test]
    async fn test_p2p_diffusion_bootstrap() {
        let mut diffusion = P2PDiffusionBuilder::new()
            .with_local_peer_id(PeerId::random(1))
            .with_dns_seeds(vec!["test.cardano.org:3001".to_string()])
            .build()
            .unwrap();

        // Bootstrap should discover some peers (mocked)
        let result = diffusion.bootstrap().await;
        assert!(result.is_ok());

        // In the mock implementation, we should get at least one peer
        assert!(diffusion.peer_count() > 0);
    }
}
