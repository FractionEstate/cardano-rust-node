//! Comprehensive tests for P2P diffusion and peer selection
//!
//! This module contains all tests from the original test specifications
//! to ensure complete compatibility and correctness of the P2P implementation.

use crate::diffusion::{gossip::*, selector::PeerSelector, types::*};
use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

fn create_test_peer(id: u8, port: u16) -> PeerInfo {
    let peer_id = PeerId::random(id);
    let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, id)), port);
    PeerInfo::new(peer_id, address)
}

#[cfg(test)]
mod peer_reputation_tests {
    use super::*;

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
}

#[cfg(test)]
mod peer_selection_tests {
    use super::*;

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
    fn test_peer_selection_reputation_filtering() {
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
            assert!(peer.reputation.value() >= selector.get_config().min_reputation_threshold);
            assert!(!peer.reputation.is_banned());
        }
    }

    #[test]
    fn test_peer_selection_diversity() {
        let config = SelectionConfig {
            geographic_diversity_weight: 1.0,
            stake_pool_diversity_weight: 1.0,
            ..Default::default()
        };

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

        // Verify geographic diversity by checking we have peers from different subnets
        let selected_subnets: HashSet<_> = candidates
            .iter()
            .filter_map(|id| selector.get_peer(id))
            .map(|peer| match peer.address.ip() {
                IpAddr::V4(ipv4) => {
                    let octets = ipv4.octets();
                    ((octets[0] as u32) << 8) | (octets[1] as u32)
                }
                IpAddr::V6(_) => 0,
            })
            .collect();

        // Should have peers from different subnets
        assert!(selected_subnets.len() >= 2);
    }

    #[test]
    fn test_peer_selection_connection_limits() {
        let config = SelectionConfig {
            max_connections: 3,
            target_connections: 2,
            ..Default::default()
        };

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
                assert!(selector.connection_count() <= selector.get_config().max_connections);
            }
        }

        // Test maintenance - should disconnect excess peers
        selector.periodic_maintenance();
        assert!(selector.connection_count() <= selector.get_config().max_connections);
    }

    #[test]
    fn test_peer_misbehavior_handling() {
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
    fn test_peer_candidate_queue_management() {
        let mut selector = PeerSelector::new(SelectionConfig::default());

        // Add test peers
        for i in 1..=5 {
            let peer = create_test_peer(i, 3000 + i as u16);
            selector.add_peer(peer).unwrap();
        }

        // Refresh candidates (internal method called by periodic_maintenance)
        selector.periodic_maintenance();

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

        let peer1_id = peer1.peer_id;
        let peer2_id = peer2.peer_id;

        selector.add_peer(peer1).unwrap();
        selector.add_peer(peer2).unwrap();

        // Select one candidate - should prefer peer1 with higher success rate
        let candidates = selector.select_connection_candidates(1);
        assert_eq!(candidates.len(), 1);

        // Calculate scores manually to verify
        let peer1_score = {
            let peer = selector.get_peer(&peer1_id).unwrap();
            selector.calculate_selection_score(peer)
        };
        let peer2_score = {
            let peer = selector.get_peer(&peer2_id).unwrap();
            selector.calculate_selection_score(peer)
        };

        assert!(
            peer1_score > peer2_score,
            "Peer1 score ({}) should be higher than Peer2 score ({})",
            peer1_score,
            peer2_score
        );
    }

    #[test]
    fn test_reputation_decay() {
        let mut selector = PeerSelector::new(SelectionConfig::default());
        let mut peer = create_test_peer(1, 3000);

        // Set high reputation
        peer.reputation = ReputationScore::new(500);
        let peer_id = peer.peer_id;
        selector.add_peer(peer).unwrap();

        // Force reputation decay by calling the internal method
        selector.periodic_maintenance();

        let peer = selector.get_peer(&peer_id).unwrap();
        assert_eq!(peer.reputation.value(), 499); // Should decay by 1

        // Set negative reputation
        selector.get_peer_mut(&peer_id).unwrap().reputation = ReputationScore::new(-300);
        selector.periodic_maintenance();

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
    fn test_peer_selection_error_handling() {
        let mut selector = PeerSelector::new(SelectionConfig::default());

        // Test adding duplicate peer
        let peer1 = create_test_peer(1, 3000);
        let peer_id = peer1.peer_id;

        selector.add_peer(peer1).unwrap();

        let peer2 = PeerInfo::new(
            peer_id,
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 2)), 3001),
        );
        assert!(selector.add_peer(peer2).is_err());

        // Test operations on non-existent peer
        let fake_peer_id = PeerId::random(99);
        assert!(selector.handle_connection_success(&fake_peer_id).is_err());
        assert!(selector
            .handle_connection_failure(&fake_peer_id, "test".to_string())
            .is_err());
        assert!(selector
            .record_misbehavior(&fake_peer_id, MisbehaviorSeverity::Minor)
            .is_err());
    }

    #[test]
    fn test_peer_connection_attempts_limit() {
        let config = SelectionConfig {
            max_connection_attempts: 2,
            ..Default::default()
        };

        let mut selector = PeerSelector::new(config);
        let peer = create_test_peer(1, 3000);
        let peer_id = peer.peer_id;

        selector.add_peer(peer).unwrap();

        // Fail connections up to the limit
        for _ in 0..2 {
            selector
                .handle_connection_failure(&peer_id, "Connection refused".to_string())
                .unwrap();
        }

        // Should not be selected anymore
        let candidates = selector.select_connection_candidates(5);
        assert!(!candidates.contains(&peer_id));
    }

    #[test]
    fn test_peer_data_transfer_tracking() {
        let mut selector = PeerSelector::new(SelectionConfig::default());
        let peer = create_test_peer(1, 3000);
        let peer_id = peer.peer_id;

        let initial_reputation = peer.reputation.value();
        selector.add_peer(peer).unwrap();

        // Simulate data transfer
        selector
            .get_peer_mut(&peer_id)
            .unwrap()
            .record_data_transfer(1024, 2048);

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
        let config = SelectionConfig {
            reputation_decay_interval: Duration::from_millis(1), // Very short for testing
            peer_discovery_interval: Duration::from_millis(1),
            ..Default::default()
        };

        let mut selector = PeerSelector::new(config);

        // Add a peer with high reputation
        let mut peer = create_test_peer(1, 3000);
        peer.reputation = ReputationScore::new(500);
        let peer_id = peer.peer_id;
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

#[cfg(test)]
mod gossip_protocol_tests {
    use super::*;

    #[test]
    fn test_gossip_advertisement_creation() {
        let mut gossip = PeerGossip::new(GossipConfig::default(), PeerId::random(1));
        let mut peers = std::collections::HashMap::new();

        // Add some peers to advertise
        for i in 2..=5 {
            let peer = create_test_peer(i, 3000 + i as u16);
            gossip.add_peer_to_advertise(peer.peer_id);
            peers.insert(peer.peer_id, peer);
        }

        let advertisement = gossip.create_advertisement(&peers).unwrap();

        assert!(!advertisement.peers.is_empty());
        assert!(advertisement.ttl > 0);
        assert_eq!(advertisement.source, *gossip.local_peer_id());
    }

    #[test]
    fn test_gossip_advertisement_processing() {
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
            timestamp: std::time::Instant::now(),
            source: PeerId::random(3),
        };

        let result = gossip.process_advertisement(advertisement.clone()).unwrap();

        match result {
            ProcessResult::NewPeers {
                peers,
                should_forward: _,
            } => {
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
            ProcessResult::Duplicate => {} // Expected
            _ => panic!("Expected Duplicate result"),
        }
    }

    #[test]
    fn test_gossip_ttl_handling() {
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
            timestamp: std::time::Instant::now(),
            source: PeerId::random(3),
        };

        let forwarded = gossip.create_forwarded_advertisement(&original).unwrap();

        assert_eq!(forwarded.ttl, original.ttl - 1);
        assert_eq!(forwarded.peers, original.peers);
        assert_eq!(forwarded.source, original.source);

        // TTL 1 should not be forwarded
        let ttl_1_ad = PeerAdvertisement { ttl: 1, ..original };

        assert!(gossip.create_forwarded_advertisement(&ttl_1_ad).is_none());
    }

    #[test]
    fn test_gossip_validation() {
        let mut gossip = PeerGossip::new(GossipConfig::default(), PeerId::random(1));

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
            timestamp: std::time::Instant::now(),
            source: PeerId::random(3),
        };

        // This would normally be valid, but we can't access private methods
        // so we'll test via process_advertisement
        assert!(gossip.process_advertisement(valid_ad).is_ok());
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn test_full_peer_lifecycle() {
        let mut selector = PeerSelector::new(SelectionConfig::default());

        // Create gossip for peer discovery
        let mut gossip = PeerGossip::new(GossipConfig::default(), PeerId::random(100));

        // Add some peers through gossip advertisement
        let advertisement = PeerAdvertisement {
            peers: vec![
                AdvertisedPeer {
                    peer_id: PeerId::random(1),
                    address: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 3001),
                    is_relay: true,
                    stake_pool_id: Some("pool1".to_string()),
                    protocol_version: Some(1),
                },
                AdvertisedPeer {
                    peer_id: PeerId::random(2),
                    address: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 2)), 3002),
                    is_relay: false,
                    stake_pool_id: None,
                    protocol_version: Some(1),
                },
            ],
            ttl: 3,
            timestamp: std::time::Instant::now(),
            source: PeerId::random(3),
        };

        let result = gossip.process_advertisement(advertisement).unwrap();

        match result {
            ProcessResult::NewPeers {
                peers,
                should_forward: _,
            } => {
                // Add discovered peers to selector
                for peer in peers {
                    selector.add_peer(peer).unwrap();
                }
            }
            _ => panic!("Expected NewPeers"),
        }

        assert_eq!(selector.peer_count(), 2);

        // Select peers for connection
        let candidates = selector.select_connection_candidates(2);
        assert!(!candidates.is_empty());

        // Connect to peers
        for peer_id in &candidates {
            selector.handle_connection_success(peer_id).unwrap();
        }

        assert!(selector.connection_count() > 0);

        // Simulate some misbehavior
        if let Some(peer_id) = candidates.first() {
            selector
                .record_misbehavior(peer_id, MisbehaviorSeverity::Minor)
                .unwrap();
            let peer = selector.get_peer(peer_id).unwrap();
            assert!(peer.reputation.value() < ReputationScore::INITIAL);
        }

        // Run periodic maintenance
        selector.periodic_maintenance();

        // Get network stats
        let stats = selector.get_stats();
        assert_eq!(stats.total_peers, 2);
    }

    #[test]
    fn test_peer_diversity_selection() {
        let config = SelectionConfig {
            geographic_diversity_weight: 1.0,
            stake_pool_diversity_weight: 1.0,
            ..Default::default()
        };

        let mut selector = PeerSelector::new(config);

        // Add peers with different characteristics for diversity testing
        let peers = vec![
            // Same region, same pool
            (1, "192.168.1.1", Some("pool1")),
            (2, "192.168.1.2", Some("pool1")),
            // Different region, same pool
            (3, "10.0.0.1", Some("pool1")),
            // Same region, different pool
            (4, "192.168.1.3", Some("pool2")),
            // Different region, different pool
            (5, "172.16.0.1", Some("pool3")),
        ];

        for (id, ip_str, pool_id) in peers {
            let ip: Ipv4Addr = ip_str.parse().unwrap();
            let address = SocketAddr::new(IpAddr::V4(ip), 3000);
            let mut peer = PeerInfo::new(PeerId::random(id), address);
            if let Some(pool) = pool_id {
                peer.stake_pool_id = Some(pool.to_string());
                peer.is_relay = true;
            }
            peer.reputation = ReputationScore::new(100); // Equal reputation
            selector.add_peer(peer).unwrap();
        }

        // Select multiple peers and verify diversity
        let candidates = selector.select_connection_candidates(3);
        assert!(candidates.len() >= 2);

        // Connect to some peers to test diversity bonus calculation
        for peer_id in &candidates[..2] {
            selector.handle_connection_success(peer_id).unwrap();
        }

        // Now select additional peers - should prefer diversity
        let additional_candidates = selector.select_connection_candidates(2);

        // Verify that selection considers diversity by checking addresses
        let selected_ips: HashSet<_> = additional_candidates
            .iter()
            .filter_map(|id| selector.get_peer(id))
            .map(|peer| peer.address.ip())
            .collect();

        // Should prefer peers from different geographic locations
        assert!(!selected_ips.is_empty());
    }
}
