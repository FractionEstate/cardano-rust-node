//! Peer Discovery Service
//!
//! This module provides peer discovery functionality including:
//! - DNS resolution for domain-based peer addresses
//! - Topology-based peer enumeration
//! - Peer prioritization (local trusted vs public relays)

use crate::topology::{AccessPoint, ResolvedPeer, TopologyConfig, TopologyError};
use std::collections::HashSet;
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::time::Duration;
use thiserror::Error;
use tokio::time::timeout;
use tracing::{debug, info, warn};

/// Peer discovery service
pub struct PeerDiscovery {
    /// Topology configuration
    topology: TopologyConfig,
    /// DNS resolution timeout
    dns_timeout: Duration,
}

impl PeerDiscovery {
    /// Create a new peer discovery service
    pub fn new(topology: TopologyConfig) -> Self {
        Self {
            topology,
            dns_timeout: Duration::from_secs(10),
        }
    }

    /// Set DNS resolution timeout
    pub fn with_dns_timeout(mut self, timeout_duration: Duration) -> Self {
        self.dns_timeout = timeout_duration;
        self
    }

    /// Discover all peers from topology configuration
    ///
    /// Resolves DNS names and returns a list of resolved peers.
    pub async fn discover_all(&self) -> Result<Vec<ResolvedPeer>, DiscoveryError> {
        let mut resolved_peers = Vec::new();
        let mut errors = Vec::new();

        // Resolve local roots (trusted peers)
        for root in &self.topology.local_roots {
            for access_point in &root.access_points {
                match self.resolve_access_point(access_point).await {
                    Ok(mut peers) => {
                        // Mark local roots as trustable
                        for peer in &mut peers {
                            peer.trustable = root.trustable;
                        }
                        info!(
                            "Resolved local peer {}: {} addresses",
                            access_point.to_string(),
                            peers.len()
                        );
                        resolved_peers.extend(peers);
                    }
                    Err(e) => {
                        warn!(
                            "Failed to resolve local peer {}: {}",
                            access_point.to_string(),
                            e
                        );
                        errors.push((access_point.clone(), e));
                    }
                }
            }
        }

        // Resolve public roots (relay nodes)
        for root in &self.topology.public_roots {
            for access_point in &root.access_points {
                match self.resolve_access_point(access_point).await {
                    Ok(peers) => {
                        info!(
                            "Resolved public relay {}: {} addresses",
                            access_point.to_string(),
                            peers.len()
                        );
                        resolved_peers.extend(peers);
                    }
                    Err(e) => {
                        warn!(
                            "Failed to resolve public relay {}: {}",
                            access_point.to_string(),
                            e
                        );
                        errors.push((access_point.clone(), e));
                    }
                }
            }
        }

        // Resolve bootstrap peers
        for access_point in &self.topology.bootstrap_peers {
            match self.resolve_access_point(access_point).await {
                Ok(peers) => {
                    info!(
                        "Resolved bootstrap peer {}: {} addresses",
                        access_point.to_string(),
                        peers.len()
                    );
                    resolved_peers.extend(peers);
                }
                Err(e) => {
                    warn!(
                        "Failed to resolve bootstrap peer {}: {}",
                        access_point.to_string(),
                        e
                    );
                    errors.push((access_point.clone(), e));
                }
            }
        }

        if resolved_peers.is_empty() {
            return Err(DiscoveryError::NoResolved {
                errors: errors.len(),
            });
        }

        info!(
            "Peer discovery complete: {} peers resolved, {} failures",
            resolved_peers.len(),
            errors.len()
        );

        Ok(resolved_peers)
    }

    /// Resolve a single access point to socket addresses
    async fn resolve_access_point(
        &self,
        access_point: &AccessPoint,
    ) -> Result<Vec<ResolvedPeer>, DiscoveryError> {
        let addr_string = format!("{}:{}", access_point.address, access_point.port);

        debug!("Resolving DNS for {}", addr_string);

        // Perform DNS resolution with timeout
        let resolve_future = tokio::task::spawn_blocking({
            let addr_string = addr_string.clone();
            move || {
                addr_string
                    .to_socket_addrs()
                    .map(|iter| iter.collect::<Vec<_>>())
            }
        });

        let socket_addrs = timeout(self.dns_timeout, resolve_future)
            .await
            .map_err(|_| DiscoveryError::DnsTimeout(addr_string.clone()))?
            .map_err(|_| DiscoveryError::TaskPanic(addr_string.clone()))?
            .map_err(|e| DiscoveryError::DnsResolution {
                address: addr_string.clone(),
                error: e.to_string(),
            })?;

        if socket_addrs.is_empty() {
            return Err(DiscoveryError::NoAddresses(addr_string));
        }

        debug!(
            "Resolved {} to {} addresses: {:?}",
            addr_string,
            socket_addrs.len(),
            socket_addrs
        );

        // Convert to ResolvedPeer instances (default trustable = false)
        let peers = socket_addrs
            .into_iter()
            .map(|socket_addr| {
                ResolvedPeer::new(access_point.clone(), socket_addr, false)
            })
            .collect();

        Ok(peers)
    }

    /// Discover only local trusted peers
    pub async fn discover_local(&self) -> Result<Vec<ResolvedPeer>, DiscoveryError> {
        let mut resolved_peers = Vec::new();

        for root in &self.topology.local_roots {
            for access_point in &root.access_points {
                match self.resolve_access_point(access_point).await {
                    Ok(mut peers) => {
                        for peer in &mut peers {
                            peer.trustable = root.trustable;
                        }
                        resolved_peers.extend(peers);
                    }
                    Err(e) => {
                        warn!(
                            "Failed to resolve local peer {}: {}",
                            access_point.to_string(),
                            e
                        );
                    }
                }
            }
        }

        Ok(resolved_peers)
    }

    /// Discover only public relay peers
    pub async fn discover_public(&self) -> Result<Vec<ResolvedPeer>, DiscoveryError> {
        let mut resolved_peers = Vec::new();

        for root in &self.topology.public_roots {
            for access_point in &root.access_points {
                match self.resolve_access_point(access_point).await {
                    Ok(peers) => {
                        resolved_peers.extend(peers);
                    }
                    Err(e) => {
                        warn!(
                            "Failed to resolve public relay {}: {}",
                            access_point.to_string(),
                            e
                        );
                    }
                }
            }
        }

        Ok(resolved_peers)
    }

    /// Get unique socket addresses from resolved peers
    pub fn unique_addresses(peers: &[ResolvedPeer]) -> Vec<SocketAddr> {
        let mut seen = HashSet::new();
        peers
            .iter()
            .filter_map(|peer| {
                if seen.insert(peer.socket_addr) {
                    Some(peer.socket_addr)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Separate peers by IP version (IPv4 vs IPv6)
    pub fn separate_by_ip_version(peers: &[ResolvedPeer]) -> (Vec<ResolvedPeer>, Vec<ResolvedPeer>) {
        let mut ipv4_peers = Vec::new();
        let mut ipv6_peers = Vec::new();

        for peer in peers {
            match peer.socket_addr.ip() {
                IpAddr::V4(_) => ipv4_peers.push(peer.clone()),
                IpAddr::V6(_) => ipv6_peers.push(peer.clone()),
            }
        }

        (ipv4_peers, ipv6_peers)
    }
}

/// Peer discovery errors
#[derive(Debug, Error)]
pub enum DiscoveryError {
    #[error("DNS resolution timeout for {0}")]
    DnsTimeout(String),

    #[error("DNS resolution failed for {address}: {error}")]
    DnsResolution { address: String, error: String },

    #[error("No addresses resolved for {0}")]
    NoAddresses(String),

    #[error("Tokio task panicked for {0}")]
    TaskPanic(String),

    #[error("No peers resolved ({errors} errors occurred)")]
    NoResolved { errors: usize },

    #[error("Topology error: {0}")]
    Topology(#[from] TopologyError),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_topology() -> TopologyConfig {
        let json = r#"{
            "bootstrapPeers": [],
            "localRoots": [
                {
                    "accessPoints": [
                        {
                            "address": "127.0.0.1",
                            "port": 3001
                        }
                    ],
                    "advertise": false,
                    "trustable": true,
                    "valency": 1
                }
            ],
            "publicRoots": [],
            "useLedgerAfterSlot": 0
        }"#;

        TopologyConfig::from_json(json).unwrap()
    }

    #[tokio::test]
    async fn test_resolve_localhost() {
        let topology = create_test_topology();
        let discovery = PeerDiscovery::new(topology);

        let peers = discovery.discover_all().await.unwrap();
        assert!(!peers.is_empty());

        // Should have resolved localhost
        assert!(peers.iter().any(|p| p.socket_addr.ip().is_loopback()));

        // Should be marked as trustable
        assert!(peers.iter().any(|p| p.trustable));
    }

    #[test]
    fn test_unique_addresses() {
        let access_point = AccessPoint::new("example.com".to_string(), 3001);
        let addr1 = "192.168.1.1:3001".parse().unwrap();
        let addr2 = "192.168.1.2:3001".parse().unwrap();

        let peers = vec![
            ResolvedPeer::new(access_point.clone(), addr1, false),
            ResolvedPeer::new(access_point.clone(), addr1, false), // Duplicate
            ResolvedPeer::new(access_point.clone(), addr2, false),
        ];

        let unique = PeerDiscovery::unique_addresses(&peers);
        assert_eq!(unique.len(), 2); // Should deduplicate
    }

    #[test]
    fn test_separate_by_ip_version() {
        let access_point = AccessPoint::new("example.com".to_string(), 3001);
        let ipv4_addr: SocketAddr = "192.168.1.1:3001".parse().unwrap();
        let ipv6_addr: SocketAddr = "[::1]:3001".parse().unwrap();

        let peers = vec![
            ResolvedPeer::new(access_point.clone(), ipv4_addr, false),
            ResolvedPeer::new(access_point.clone(), ipv6_addr, false),
        ];

        let (ipv4, ipv6) = PeerDiscovery::separate_by_ip_version(&peers);
        assert_eq!(ipv4.len(), 1);
        assert_eq!(ipv6.len(), 1);
    }
}
