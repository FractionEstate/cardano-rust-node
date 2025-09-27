use crate::diffusion::types::*;
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::time::{Duration, Instant};


/// Peer advertisement message for peer discovery
#[derive(Debug, Clone, PartialEq)]
pub struct PeerAdvertisement {
    /// Peers being advertised
    pub peers: Vec<AdvertisedPeer>,
    /// Time-to-live for this advertisement
    pub ttl: u32,
    /// Timestamp when advertisement was created
    pub timestamp: Instant,
    /// Source peer ID who created the advertisement
    pub source: PeerId,
}

/// Information about a peer being advertised
#[derive(Debug, Clone, PartialEq)]
pub struct AdvertisedPeer {
    /// Peer ID
    pub peer_id: PeerId,
    /// Network address
    pub address: SocketAddr,
    /// Whether this is a relay node
    pub is_relay: bool,
    /// Associated stake pool ID (if any)
    pub stake_pool_id: Option<String>,
    /// Advertised protocol version
    pub protocol_version: Option<u32>,
}

impl From<&PeerInfo> for AdvertisedPeer {
    fn from(peer: &PeerInfo) -> Self {
        Self {
            peer_id: peer.peer_id.clone(),
            address: peer.address,
            is_relay: peer.is_relay,
            stake_pool_id: peer.stake_pool_id.clone(),
            protocol_version: peer.protocol_version,
        }
    }
}

/// Configuration for peer gossip protocol
#[derive(Debug, Clone)]
pub struct GossipConfig {
    /// Maximum number of peers to advertise at once
    pub max_peers_per_advertisement: usize,
    /// How often to send peer advertisements
    pub advertisement_interval: Duration,
    /// Maximum TTL for advertisements
    pub max_ttl: u32,
    /// How long to remember seen advertisements (to prevent loops)
    pub advertisement_cache_duration: Duration,
    /// Maximum number of advertisements to cache
    pub max_cached_advertisements: usize,
    /// Minimum reputation required to advertise a peer
    pub min_advertisement_reputation: i32,
    /// Probability of forwarding an advertisement (0.0-1.0)
    pub forward_probability: f64,
}

impl Default for GossipConfig {
    fn default() -> Self {
        Self {
            max_peers_per_advertisement: 10,
            advertisement_interval: Duration::from_secs(60), // 1 minute
            max_ttl: 5,
            advertisement_cache_duration: Duration::from_secs(300), // 5 minutes
            max_cached_advertisements: 1000,
            min_advertisement_reputation: 0,
            forward_probability: 0.8,
        }
    }
}

/// Peer gossip and discovery manager
#[derive(Debug)]
pub struct PeerGossip {
    /// Configuration
    config: GossipConfig,
    /// Our peer ID
    local_peer_id: PeerId,
    /// Cache of seen advertisements to prevent loops
    advertisement_cache: HashMap<AdvertisementId, CachedAdvertisement>,
    /// Last time we sent an advertisement
    last_advertisement: Instant,
    /// Peers we want to advertise to others
    peers_to_advertise: HashSet<PeerId>,
    /// Statistics
    stats: GossipStats,
}

/// Unique identifier for an advertisement
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct AdvertisementId {
    source: PeerId,
    timestamp_nanos: u64,
}

/// Cached advertisement information
#[derive(Debug, Clone)]
struct CachedAdvertisement {
    advertisement: PeerAdvertisement,
    received_at: Instant,
    forwarded_count: usize,
}

/// Gossip protocol statistics
#[derive(Debug, Clone, Default)]
pub struct GossipStats {
    /// Total advertisements sent
    pub advertisements_sent: u64,
    /// Total advertisements received
    pub advertisements_received: u64,
    /// Total advertisements forwarded
    pub advertisements_forwarded: u64,
    /// Total peers discovered through gossip
    pub peers_discovered: u64,
    /// Number of duplicate advertisements received
    pub duplicate_advertisements: u64,
    /// Number of invalid advertisements received
    pub invalid_advertisements: u64,
}

impl PeerGossip {
    /// Create a new peer gossip manager
    pub fn new(config: GossipConfig, local_peer_id: PeerId) -> Self {
        Self {
            config,
            local_peer_id,
            advertisement_cache: HashMap::new(),
            last_advertisement: Instant::now(),
            peers_to_advertise: HashSet::new(),
            stats: GossipStats::default(),
        }
    }

    /// Add a peer to be advertised to others
    pub fn add_peer_to_advertise(&mut self, peer_id: PeerId) {
        self.peers_to_advertise.insert(peer_id);
    }

    /// Remove a peer from advertisement list
    pub fn remove_peer_from_advertise(&mut self, peer_id: &PeerId) {
        self.peers_to_advertise.remove(peer_id);
    }

    /// Create an advertisement for our known peers
    pub fn create_advertisement(&mut self, peers: &HashMap<PeerId, PeerInfo>) -> Option<PeerAdvertisement> {
        let now = Instant::now();

        // Check if it's time to send an advertisement
        if now.duration_since(self.last_advertisement) < self.config.advertisement_interval {
            return None;
        }

        // Select peers to advertise
        let advertised_peers: Vec<AdvertisedPeer> = self.peers_to_advertise.iter()
            .filter_map(|peer_id| peers.get(peer_id))
            .filter(|peer| {
                // Only advertise peers with good reputation
                peer.reputation.value() >= self.config.min_advertisement_reputation
                && peer.is_available()
                && peer.peer_id != self.local_peer_id // Don't advertise ourselves
            })
            .take(self.config.max_peers_per_advertisement)
            .map(AdvertisedPeer::from)
            .collect();

        if advertised_peers.is_empty() {
            return None;
        }

        let advertisement = PeerAdvertisement {
            peers: advertised_peers,
            ttl: self.config.max_ttl,
            timestamp: now,
            source: self.local_peer_id.clone(),
        };

        self.last_advertisement = now;
        self.stats.advertisements_sent += 1;

        // Cache our own advertisement
        let ad_id = self.get_advertisement_id(&advertisement);
        self.advertisement_cache.insert(ad_id, CachedAdvertisement {
            advertisement: advertisement.clone(),
            received_at: now,
            forwarded_count: 0,
        });

        Some(advertisement)
    }

    /// Process a received peer advertisement
    pub fn process_advertisement(
        &mut self,
        advertisement: PeerAdvertisement,
    ) -> Result<ProcessResult, GossipError> {
        let now = Instant::now();
        self.stats.advertisements_received += 1;

        // Validate advertisement
        self.validate_advertisement(&advertisement)?;

        // Check if we've seen this advertisement before
        let ad_id = self.get_advertisement_id(&advertisement);
        if let Some(_cached) = self.advertisement_cache.get(&ad_id) {
            self.stats.duplicate_advertisements += 1;
            return Ok(ProcessResult::Duplicate);
        }

        // Check TTL
        if advertisement.ttl == 0 {
            return Ok(ProcessResult::Expired);
        }

        // Extract new peers
        let mut new_peers = Vec::new();
        for advertised_peer in &advertisement.peers {
            // Don't add ourselves
            if advertised_peer.peer_id == self.local_peer_id {
                continue;
            }

            // Convert to PeerInfo
            let peer_info = PeerInfo {
                peer_id: advertised_peer.peer_id.clone(),
                address: advertised_peer.address,
                connection_state: ConnectionState::Disconnected,
                reputation: ReputationScore::default(),
                is_relay: advertised_peer.is_relay,
                stake_pool_id: advertised_peer.stake_pool_id.clone(),
                connection_attempts: 0,
                successful_connections: 0,
                discovered_at: now,
                last_connection_attempt: None,
                last_successful_connection: None,
                bytes_sent: 0,
                bytes_received: 0,
                protocol_version: advertised_peer.protocol_version,
                metadata: HashMap::new(),
            };
            new_peers.push(peer_info);
        }

        // Cache the advertisement
        self.advertisement_cache.insert(ad_id, CachedAdvertisement {
            advertisement: advertisement.clone(),
            received_at: now,
            forwarded_count: 0,
        });

        // Determine if we should forward this advertisement
        let should_forward = advertisement.ttl > 1
            && self.should_forward_advertisement(&advertisement);

        let result = ProcessResult::NewPeers {
            peers: new_peers,
            should_forward,
        };

        self.stats.peers_discovered += advertisement.peers.len() as u64;

        Ok(result)
    }

    /// Create a forwarded advertisement (decremented TTL)
    pub fn create_forwarded_advertisement(&mut self, original: &PeerAdvertisement) -> Option<PeerAdvertisement> {
        if original.ttl <= 1 {
            return None;
        }

        let ad_id = self.get_advertisement_id(original);

        // Update forward count in cache
        if let Some(cached) = self.advertisement_cache.get_mut(&ad_id) {
            cached.forwarded_count += 1;
        }

        self.stats.advertisements_forwarded += 1;

        Some(PeerAdvertisement {
            peers: original.peers.clone(),
            ttl: original.ttl - 1,
            timestamp: original.timestamp,
            source: original.source.clone(),
        })
    }

    /// Periodic maintenance - clean up old cached advertisements
    pub fn periodic_maintenance(&mut self) {
        let now = Instant::now();
        let cutoff = now - self.config.advertisement_cache_duration;

        // Remove old cached advertisements
        self.advertisement_cache.retain(|_, cached| {
            cached.received_at > cutoff
        });

        // Limit cache size
        if self.advertisement_cache.len() > self.config.max_cached_advertisements {
            // Remove oldest entries
            let mut entries: Vec<_> = self.advertisement_cache.iter()
                .map(|(id, cached)| (id.clone(), cached.received_at))
                .collect();

            entries.sort_by_key(|(_, time)| *time);

            let to_remove = entries.len() - self.config.max_cached_advertisements;
            for (id, _) in entries.into_iter().take(to_remove) {
                self.advertisement_cache.remove(&id);
            }
        }
    }

    /// Get gossip statistics
    pub fn get_stats(&self) -> &GossipStats {
        &self.stats
    }

    /// Reset statistics
    pub fn reset_stats(&mut self) {
        self.stats = GossipStats::default();
    }

    /// Get local peer ID
    pub fn local_peer_id(&self) -> &PeerId {
        &self.local_peer_id
    }

    // Private helper methods

    fn validate_advertisement(&self, advertisement: &PeerAdvertisement) -> Result<(), GossipError> {
        // Check TTL bounds
        if advertisement.ttl > self.config.max_ttl {
            return Err(GossipError::InvalidTtl(advertisement.ttl));
        }

        // Check peer count
        if advertisement.peers.len() > self.config.max_peers_per_advertisement {
            return Err(GossipError::TooManyPeers(advertisement.peers.len()));
        }

        // Check for empty advertisements
        if advertisement.peers.is_empty() {
            return Err(GossipError::EmptyAdvertisement);
        }

        // Validate peer addresses
        for peer in &advertisement.peers {
            if peer.address.ip().is_loopback() || peer.address.ip().is_unspecified() {
                return Err(GossipError::InvalidAddress(peer.address));
            }
        }

        Ok(())
    }

    fn get_advertisement_id(&self, advertisement: &PeerAdvertisement) -> AdvertisementId {
        AdvertisementId {
            source: advertisement.source.clone(),
            timestamp_nanos: advertisement.timestamp.elapsed().as_nanos() as u64,
        }
    }

    fn should_forward_advertisement(&self, _advertisement: &PeerAdvertisement) -> bool {
        // Simple probability-based forwarding
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        self.local_peer_id.hash(&mut hasher);
        let hash = hasher.finish();

        let random_value = (hash % 1000) as f64 / 1000.0;
        random_value < self.config.forward_probability
    }
}

/// Result of processing a peer advertisement
#[derive(Debug)]
pub enum ProcessResult {
    /// Advertisement contained new peers
    NewPeers {
        peers: Vec<PeerInfo>,
        should_forward: bool,
    },
    /// Advertisement was a duplicate
    Duplicate,
    /// Advertisement had expired (TTL = 0)
    Expired,
}

/// Errors that can occur in the gossip protocol
#[derive(Debug)]
pub enum GossipError {
    /// Invalid TTL value
    InvalidTtl(u32),
    /// Too many peers in advertisement
    TooManyPeers(usize),
    /// Advertisement contains no peers
    EmptyAdvertisement,
    /// Invalid peer address
    InvalidAddress(SocketAddr),
    /// Advertisement validation failed
    ValidationError(String),
}

impl std::fmt::Display for GossipError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GossipError::InvalidTtl(ttl) => write!(f, "Invalid TTL: {}", ttl),
            GossipError::TooManyPeers(count) => write!(f, "Too many peers in advertisement: {}", count),
            GossipError::EmptyAdvertisement => write!(f, "Advertisement contains no peers"),
            GossipError::InvalidAddress(addr) => write!(f, "Invalid peer address: {}", addr),
            GossipError::ValidationError(msg) => write!(f, "Validation error: {}", msg),
        }
    }
}

impl std::error::Error for GossipError {}

/// Peer discovery service that combines gossip with other discovery methods
#[derive(Debug)]
pub struct PeerDiscovery {
    /// Gossip manager
    gossip: PeerGossip,
    /// DNS seed addresses for bootstrap
    dns_seeds: Vec<String>,
    /// Last DNS bootstrap attempt
    last_dns_bootstrap: Option<Instant>,
    /// Bootstrap interval
    bootstrap_interval: Duration,
}

impl PeerDiscovery {
    /// Create a new peer discovery service
    pub fn new(
        gossip_config: GossipConfig,
        local_peer_id: PeerId,
        dns_seeds: Vec<String>,
    ) -> Self {
        Self {
            gossip: PeerGossip::new(gossip_config, local_peer_id),
            dns_seeds,
            last_dns_bootstrap: None,
            bootstrap_interval: Duration::from_secs(300), // 5 minutes
        }
    }

    /// Perform DNS bootstrap to discover initial peers
    pub async fn dns_bootstrap(&mut self) -> Result<Vec<PeerInfo>, Box<dyn std::error::Error>> {
        let now = Instant::now();

        // Check if we need to bootstrap
        if let Some(last_bootstrap) = self.last_dns_bootstrap {
            if now.duration_since(last_bootstrap) < self.bootstrap_interval {
                return Ok(Vec::new());
            }
        }

        let mut discovered_peers = Vec::new();

        for seed in &self.dns_seeds {
            match self.resolve_dns_seed(seed).await {
                Ok(mut peers) => {
                    discovered_peers.append(&mut peers);
                }
                Err(e) => {
                    eprintln!("Failed to resolve DNS seed {}: {}", seed, e);
                }
            }
        }

        self.last_dns_bootstrap = Some(now);

        Ok(discovered_peers)
    }

    async fn resolve_dns_seed(&self, seed: &str) -> Result<Vec<PeerInfo>, Box<dyn std::error::Error>> {
        // In a real implementation, this would perform DNS queries
        // For testing, we'll return empty or mock some peers

        // Parse seed as hostname:port
        let parts: Vec<&str> = seed.split(':').collect();
        if parts.len() != 2 {
            return Err("Invalid DNS seed format".into());
        }

        let _hostname = parts[0];
        let port: u16 = parts[1].parse()?;

        // Mock DNS resolution - in practice would use tokio::net::lookup_host
        let mock_peers = vec![
            PeerInfo::new(
                PeerId::new([100; 32]),
                std::net::SocketAddr::new(
                    std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)),
                    port
                )
            )
        ];

        Ok(mock_peers)
    }

    /// Get mutable reference to gossip manager
    pub fn gossip_mut(&mut self) -> &mut PeerGossip {
        &mut self.gossip
    }

    /// Get reference to gossip manager
    pub fn gossip(&self) -> &PeerGossip {
        &self.gossip
    }

    /// Periodic maintenance
    pub fn periodic_maintenance(&mut self) {
        self.gossip.periodic_maintenance();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, IpAddr};

    fn create_test_peer_info(id: u8, port: u16) -> PeerInfo {
        let peer_id = PeerId::random(id);
        let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, id)), port);
        PeerInfo::new(peer_id, address)
    }

    #[test]
    fn test_gossip_creation() {
        let config = GossipConfig::default();
        let local_peer_id = PeerId::random(1);
        let gossip = PeerGossip::new(config, local_peer_id);

        assert_eq!(gossip.stats.advertisements_sent, 0);
        assert_eq!(gossip.advertisement_cache.len(), 0);
    }

    #[test]
    fn test_peer_advertisement_creation() {
        let mut gossip = PeerGossip::new(GossipConfig::default(), PeerId::random(1));
        let mut peers = HashMap::new();

        // Add some peers
        for i in 2..=5 {
            let peer = create_test_peer_info(i, 3000 + i as u16);
            gossip.add_peer_to_advertise(peer.peer_id.clone());
            peers.insert(peer.peer_id.clone(), peer);
        }

        let advertisement = gossip.create_advertisement(&peers).unwrap();

        assert!(!advertisement.peers.is_empty());
        assert_eq!(advertisement.ttl, gossip.config.max_ttl);
        assert_eq!(advertisement.source, gossip.local_peer_id);
    }

    #[test]
    fn test_advertisement_validation() {
        let gossip = PeerGossip::new(GossipConfig::default(), PeerId::random(1));

        // Valid advertisement
        let valid_ad = PeerAdvertisement {
            peers: vec![AdvertisedPeer {
                peer_id: PeerId::random(2),
                address: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 2)), 3000),
                is_relay: false,
                stake_pool_id: None,
                protocol_version: Some(1),
            }],
            ttl: 3,
            timestamp: Instant::now(),
            source: PeerId::random(3),
        };

        assert!(gossip.validate_advertisement(&valid_ad).is_ok());

        // Invalid TTL
        let invalid_ttl_ad = PeerAdvertisement {
            peers: valid_ad.peers.clone(),
            ttl: gossip.config.max_ttl + 1,
            timestamp: Instant::now(),
            source: PeerId::random(3),
        };

        assert!(gossip.validate_advertisement(&invalid_ttl_ad).is_err());

        // Empty advertisement
        let empty_ad = PeerAdvertisement {
            peers: vec![],
            ttl: 3,
            timestamp: Instant::now(),
            source: PeerId::random(3),
        };

        assert!(gossip.validate_advertisement(&empty_ad).is_err());
    }

    #[test]
    fn test_advertisement_processing() {
        let mut gossip = PeerGossip::new(GossipConfig::default(), PeerId::random(1));

        let advertisement = PeerAdvertisement {
            peers: vec![AdvertisedPeer {
                peer_id: PeerId::random(2),
                address: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 2)), 3000),
                is_relay: true,
                stake_pool_id: Some("pool123".to_string()),
                protocol_version: Some(1),
            }],
            ttl: 3,
            timestamp: Instant::now(),
            source: PeerId::random(3),
        };

        let result = gossip.process_advertisement(advertisement.clone()).unwrap();

        match result {
            ProcessResult::NewPeers { peers, should_forward: _ } => {
                assert_eq!(peers.len(), 1);
                assert_eq!(peers[0].peer_id, PeerId::random(2));
                assert_eq!(peers[0].stake_pool_id, Some("pool123".to_string()));
                assert!(peers[0].is_relay);
            }
            _ => panic!("Expected NewPeers result"),
        }

        // Process same advertisement again - should be duplicate
        let result2 = gossip.process_advertisement(advertisement).unwrap();
        match result2 {
            ProcessResult::Duplicate => {}, // Expected
            _ => panic!("Expected Duplicate result"),
        }
    }

    #[test]
    fn test_advertisement_forwarding() {
        let mut gossip = PeerGossip::new(GossipConfig::default(), PeerId::random(1));

        let original = PeerAdvertisement {
            peers: vec![AdvertisedPeer {
                peer_id: PeerId::random(2),
                address: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 2)), 3000),
                is_relay: false,
                stake_pool_id: None,
                protocol_version: Some(1),
            }],
            ttl: 3,
            timestamp: Instant::now(),
            source: PeerId::random(3),
        };

        let forwarded = gossip.create_forwarded_advertisement(&original).unwrap();

        assert_eq!(forwarded.ttl, original.ttl - 1);
        assert_eq!(forwarded.peers, original.peers);
        assert_eq!(forwarded.source, original.source);

        // TTL 1 should not be forwarded
        let ttl_1_ad = PeerAdvertisement {
            ttl: 1,
            ..original
        };

        assert!(gossip.create_forwarded_advertisement(&ttl_1_ad).is_none());
    }

    #[test]
    fn test_gossip_statistics() {
        let mut gossip = PeerGossip::new(GossipConfig::default(), PeerId::random(1));
        let mut peers = HashMap::new();

        // Add peer to advertise
        let peer = create_test_peer_info(2, 3000);
        gossip.add_peer_to_advertise(peer.peer_id.clone());
        peers.insert(peer.peer_id.clone(), peer);

        // Create advertisement
        gossip.create_advertisement(&peers);

        let stats = gossip.get_stats();
        assert_eq!(stats.advertisements_sent, 1);

        // Process advertisement
        let advertisement = PeerAdvertisement {
            peers: vec![AdvertisedPeer {
                peer_id: PeerId::random(3),
                address: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 3)), 3000),
                is_relay: false,
                stake_pool_id: None,
                protocol_version: Some(1),
            }],
            ttl: 2,
            timestamp: Instant::now(),
            source: PeerId::random(4),
        };

        gossip.process_advertisement(advertisement).unwrap();

        let stats = gossip.get_stats();
        assert_eq!(stats.advertisements_received, 1);
        assert_eq!(stats.peers_discovered, 1);
    }

    #[tokio::test]
    async fn test_peer_discovery() {
        let local_peer_id = PeerId::random(1);
        let dns_seeds = vec!["seed1.cardano.org:3001".to_string()];

        let mut discovery = PeerDiscovery::new(
            GossipConfig::default(),
            local_peer_id,
            dns_seeds,
        );

        // This would normally perform real DNS lookups
        // For now, it should return empty or mock results
        let peers = discovery.dns_bootstrap().await.unwrap();

        // In the mock implementation, we should get some test peers
        assert!(!peers.is_empty());
    }
}
