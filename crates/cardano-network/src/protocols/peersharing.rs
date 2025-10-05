//! PeerSharing Protocol (node-to-node, protocol #10)
//!
//! Enables decentralized peer discovery and topology management in the P2P network.
//!
//! ## Protocol Overview
//!
//! The PeerSharing protocol allows nodes to:
//! - Request peer addresses from other nodes
//! - Share their known peer list
//! - Build and maintain decentralized network topology
//! - Bootstrap new nodes without relying on central registries
//!
//! ## State Machine
//!
//! ```text
//! ┌────────┐
//! │  Idle  │◀────────────────────┐
//! └────┬───┘                     │
//!      │ MsgSharePeers          │
//!      ▼                         │
//! ┌──────────┐  MsgShareResponse │
//! │   Busy   ├───────────────────┘
//! └──────────┘
//!
//! Flow:
//! 1. Client sends MsgSharePeers requesting N peer addresses
//! 2. Server responds with MsgSharePeersResponse containing available peers
//! 3. Client evaluates received peers and potentially connects to them
//! 4. Process repeats as needed to maintain healthy peer connections
//! ```
//!
//! ## Peer Selection
//!
//! Nodes select which peers to share based on:
//! - **Stake**: Higher stake nodes preferred (for SPOs)
//! - **Uptime**: More reliable nodes prioritized
//! - **Diversity**: Geographic and network diversity
//! - **Privacy**: Avoid revealing full peer set
//!
//! ## Use Cases
//!
//! 1. **Network bootstrap**: New nodes discover initial peers
//! 2. **Topology optimization**: Nodes improve connection quality
//! 3. **Resilience**: Recover from network partitions
//! 4. **Decentralization**: Reduce reliance on hardcoded relays
//!
//! ## Reference
//!
//! Based on IntersectMBO/ouroboros-network PeerSharing module

use crate::{NetworkError, Result};
use bytes::Bytes;
use minicbor::{Decode, Encode};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

/// Protocol number for PeerSharing (node-to-node)
pub const PROTOCOL_NUM: u16 = 10;

/// Maximum number of peers to request in a single message
pub const MAX_PEERS_PER_REQUEST: u16 = 100;

/// Maximum number of peers to share in a single response
pub const MAX_PEERS_PER_RESPONSE: u16 = 100;

/// PeerSharing protocol states
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum State {
    /// Idle, ready to send/receive requests
    Idle,
    /// Waiting for response to peer request
    Busy,
    /// Protocol terminated
    Done,
}

/// PeerSharing protocol messages
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Encode, Decode)]
pub enum Message {
    /// Request peer addresses from remote node
    #[n(0)]
    MsgSharePeers {
        /// Maximum number of peer addresses requested
        #[n(0)]
        amount: u16,
    },

    /// Response with peer addresses
    #[n(1)]
    MsgSharePeersResponse {
        /// List of peer addresses
        #[n(0)]
        peers: Vec<PeerAddress>,
    },

    /// Either party terminates the protocol
    #[n(2)]
    MsgDone,
}

/// Peer address information
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Encode, Decode)]
pub struct PeerAddress {
    /// IP address (v4 or v6)
    #[n(0)]
    pub ip: IpAddress,

    /// Port number
    #[n(1)]
    pub port: u16,

    /// Optional peer metadata
    #[n(2)]
    pub metadata: Option<PeerMetadata>,
}

/// IP address representation (v4 or v6)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Encode, Decode)]
pub enum IpAddress {
    /// IPv4 address (4 bytes)
    #[n(0)]
    V4 {
        #[n(0)]
        #[cbor(with = "minicbor::bytes")]
        octets: Vec<u8>,
    },

    /// IPv6 address (16 bytes)
    #[n(1)]
    V6 {
        #[n(0)]
        #[cbor(with = "minicbor::bytes")]
        octets: Vec<u8>,
    },
}

impl IpAddress {
    /// Create from standard library IpAddr
    pub fn from_ip_addr(addr: IpAddr) -> Self {
        match addr {
            IpAddr::V4(v4) => Self::V4 {
                octets: v4.octets().to_vec(),
            },
            IpAddr::V6(v6) => Self::V6 {
                octets: v6.octets().to_vec(),
            },
        }
    }

    /// Convert to standard library IpAddr
    pub fn to_ip_addr(&self) -> Option<IpAddr> {
        match self {
            IpAddress::V4 { octets } if octets.len() == 4 => Some(IpAddr::V4(Ipv4Addr::new(
                octets[0], octets[1], octets[2], octets[3],
            ))),
            IpAddress::V6 { octets } if octets.len() == 16 => {
                let mut segments = [0u16; 8];
                for (i, chunk) in octets.chunks(2).enumerate() {
                    segments[i] = u16::from_be_bytes([chunk[0], chunk[1]]);
                }
                Some(IpAddr::V6(Ipv6Addr::from(segments)))
            }
            _ => None,
        }
    }
}

impl PeerAddress {
    /// Create a new peer address
    pub fn new(ip: IpAddr, port: u16) -> Self {
        Self {
            ip: IpAddress::from_ip_addr(ip),
            port,
            metadata: None,
        }
    }

    /// Create from SocketAddr
    pub fn from_socket_addr(addr: SocketAddr) -> Self {
        Self::new(addr.ip(), addr.port())
    }

    /// Convert to SocketAddr
    pub fn to_socket_addr(&self) -> Option<SocketAddr> {
        self.ip
            .to_ip_addr()
            .map(|ip| SocketAddr::new(ip, self.port))
    }

    /// Set peer metadata
    pub fn with_metadata(mut self, metadata: PeerMetadata) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

/// Optional peer metadata for selection/ranking
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Encode, Decode)]
pub struct PeerMetadata {
    /// Estimated stake pool size (if known)
    #[n(0)]
    pub stake: Option<u64>,

    /// Peer reliability score (0-100)
    #[n(1)]
    pub reliability: Option<u8>,

    /// Geographic region hint (ISO 3166-1 alpha-2 code)
    #[n(2)]
    pub region: Option<String>,
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            State::Idle => write!(f, "Idle"),
            State::Busy => write!(f, "Busy"),
            State::Done => write!(f, "Done"),
        }
    }
}

impl fmt::Display for PeerAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(socket_addr) = self.to_socket_addr() {
            write!(f, "{}", socket_addr)
        } else {
            write!(f, "Invalid peer address")
        }
    }
}

/// PeerSharing protocol handler
pub struct PeerSharing {
    state: State,
    known_peers: Vec<PeerAddress>,
}

impl PeerSharing {
    /// Create a new PeerSharing protocol instance
    pub fn new() -> Self {
        Self {
            state: State::Idle,
            known_peers: Vec::new(),
        }
    }

    /// Get the current protocol state
    pub fn state(&self) -> State {
        self.state
    }

    /// Add a known peer to the local peer list
    pub fn add_peer(&mut self, peer: PeerAddress) {
        if !self.known_peers.contains(&peer) {
            self.known_peers.push(peer);
        }
    }

    /// Get all known peers
    pub fn get_peers(&self) -> &[PeerAddress] {
        &self.known_peers
    }

    /// Request peers from remote node
    pub fn request_peers(&mut self, amount: u16) -> Result<Message> {
        match self.state {
            State::Idle => {
                if amount == 0 || amount > MAX_PEERS_PER_REQUEST {
                    return Err(NetworkError::ProtocolError(format!(
                        "Invalid peer request amount: {} (must be 1-{})",
                        amount, MAX_PEERS_PER_REQUEST
                    )));
                }

                self.state = State::Busy;
                Ok(Message::MsgSharePeers { amount })
            }
            _ => Err(NetworkError::ProtocolError(format!(
                "Cannot request peers in state {:?}",
                self.state
            ))),
        }
    }

    /// Handle an incoming message
    pub fn handle_message(&mut self, msg: Message) -> Result<Option<Message>> {
        match (&self.state, &msg) {
            // Server receives peer request
            (State::Idle, Message::MsgSharePeers { amount }) => self.handle_share_request(*amount),

            // Client receives peer response
            (State::Busy, Message::MsgSharePeersResponse { peers }) => {
                self.handle_share_response(peers.clone())?;
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

    /// Handle a request to share peers
    fn handle_share_request(&self, amount: u16) -> Result<Option<Message>> {
        if amount == 0 || amount > MAX_PEERS_PER_REQUEST {
            return Err(NetworkError::ProtocolError(format!(
                "Invalid peer request amount: {}",
                amount
            )));
        }

        // Select peers to share (limited by amount and max response size)
        let num_to_share = (amount as usize)
            .min(self.known_peers.len())
            .min(MAX_PEERS_PER_RESPONSE as usize);

        let peers_to_share = self.known_peers[..num_to_share].to_vec();

        Ok(Some(Message::MsgSharePeersResponse {
            peers: peers_to_share,
        }))
    }

    /// Handle a response with shared peers
    fn handle_share_response(&mut self, peers: Vec<PeerAddress>) -> Result<()> {
        if peers.len() > MAX_PEERS_PER_RESPONSE as usize {
            return Err(NetworkError::ProtocolError(format!(
                "Received too many peers: {} (max {})",
                peers.len(),
                MAX_PEERS_PER_RESPONSE
            )));
        }

        // Add new peers to known peer list
        for peer in peers {
            self.add_peer(peer);
        }

        Ok(())
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

impl Default for PeerSharing {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_peersharing_creation() {
        let ps = PeerSharing::new();
        assert_eq!(ps.state(), State::Idle);
        assert_eq!(ps.get_peers().len(), 0);
    }

    #[test]
    fn test_add_peer() {
        let mut ps = PeerSharing::new();
        let peer = PeerAddress::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 3001);

        ps.add_peer(peer.clone());
        assert_eq!(ps.get_peers().len(), 1);

        // Adding same peer again shouldn't duplicate
        ps.add_peer(peer);
        assert_eq!(ps.get_peers().len(), 1);
    }

    #[test]
    fn test_request_peers() {
        let mut ps = PeerSharing::new();

        let msg = ps.request_peers(10).unwrap();
        assert!(matches!(msg, Message::MsgSharePeers { amount: 10 }));
        assert_eq!(ps.state(), State::Busy);
    }

    #[test]
    fn test_invalid_request_amount() {
        let mut ps = PeerSharing::new();

        // Zero amount
        let result = ps.request_peers(0);
        assert!(result.is_err());

        // Too large amount
        let result = ps.request_peers(MAX_PEERS_PER_REQUEST + 1);
        assert!(result.is_err());
    }

    #[test]
    fn test_share_peers_flow() {
        let mut server = PeerSharing::new();
        let mut client = PeerSharing::new();

        // Server has some peers
        for i in 0..5 {
            let peer = PeerAddress::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, i)), 3001);
            server.add_peer(peer);
        }

        // Client requests peers
        let request = client.request_peers(3).unwrap();

        // Server handles request
        let response = server.handle_message(request).unwrap();
        assert!(matches!(
            response,
            Some(Message::MsgSharePeersResponse { .. })
        ));

        // Client receives response
        if let Some(Message::MsgSharePeersResponse { peers }) = response {
            assert_eq!(peers.len(), 3);
            client
                .handle_message(Message::MsgSharePeersResponse { peers })
                .unwrap();
            assert_eq!(client.get_peers().len(), 3);
            assert_eq!(client.state(), State::Idle);
        }
    }

    #[test]
    fn test_ipv4_address() {
        let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        let ip_address = IpAddress::from_ip_addr(ip);

        let converted = ip_address.to_ip_addr().unwrap();
        assert_eq!(converted, ip);
    }

    #[test]
    fn test_ipv6_address() {
        let ip = IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1));
        let ip_address = IpAddress::from_ip_addr(ip);

        let converted = ip_address.to_ip_addr().unwrap();
        assert_eq!(converted, ip);
    }

    #[test]
    fn test_peer_address_from_socket_addr() {
        let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)), 8080);
        let peer = PeerAddress::from_socket_addr(socket);

        let converted = peer.to_socket_addr().unwrap();
        assert_eq!(converted, socket);
    }

    #[test]
    fn test_peer_with_metadata() {
        let peer = PeerAddress::new(IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4)), 3001).with_metadata(
            PeerMetadata {
                stake: Some(1_000_000),
                reliability: Some(95),
                region: Some("US".to_string()),
            },
        );

        assert!(peer.metadata.is_some());
        let metadata = peer.metadata.unwrap();
        assert_eq!(metadata.stake, Some(1_000_000));
        assert_eq!(metadata.reliability, Some(95));
        assert_eq!(metadata.region, Some("US".to_string()));
    }

    #[test]
    fn test_max_peers_enforcement() {
        let mut ps = PeerSharing::new();

        // Add more peers than max response size
        for i in 0..150 {
            let peer = PeerAddress::new(
                IpAddr::V4(Ipv4Addr::new(10, (i / 256) as u8, (i % 256) as u8, 1)),
                3001,
            );
            ps.add_peer(peer);
        }

        // Request valid amount, but response should be limited to MAX_PEERS_PER_RESPONSE
        let request = Message::MsgSharePeers {
            amount: MAX_PEERS_PER_REQUEST,
        };
        let response = ps.handle_message(request).unwrap();

        if let Some(Message::MsgSharePeersResponse { peers }) = response {
            assert!(peers.len() <= MAX_PEERS_PER_RESPONSE as usize);
            assert_eq!(peers.len(), MAX_PEERS_PER_RESPONSE as usize);
        }
    }

    #[test]
    fn test_encode_decode_roundtrip() {
        let peers = vec![
            PeerAddress::new(IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4)), 3001),
            PeerAddress::new(
                IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1)),
                3002,
            ),
        ];

        let msg = Message::MsgSharePeersResponse { peers };

        let encoded = PeerSharing::encode_message(&msg).unwrap();
        let decoded = PeerSharing::decode_message(&encoded).unwrap();

        assert_eq!(msg, decoded);
    }

    #[test]
    fn test_done_message() {
        let mut ps = PeerSharing::new();

        ps.handle_message(Message::MsgDone).unwrap();
        assert_eq!(ps.state(), State::Done);
    }

    #[test]
    fn test_peer_address_display() {
        let peer = PeerAddress::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 3001);
        let display = peer.to_string();
        assert!(display.contains("192.168.1.1"));
        assert!(display.contains("3001"));
    }
}
