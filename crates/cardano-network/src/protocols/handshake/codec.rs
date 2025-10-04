//! Handshake Protocol CBOR Codec
//!
//! Implements CBOR encoding and decoding for handshake messages according to
//! the Ouroboros network protocol specification.
//!
//! ## Message Encoding Format
//!
//! All messages are encoded as CBOR arrays with a tag identifying the message type:
//!
//! ```text
//! MsgProposeVersions:  [0, {version_int: version_data_term, ...}]
//! MsgReplyVersions:    [0, {version_int: version_data_term, ...}]
//! MsgAcceptVersion:    [1, version_int, version_data_term]
//! MsgRefuse:           [2, refuse_reason]
//! MsgQueryReply:       [3, {version_int: version_data_term, ...}]
//! ```
//!
//! Version data is encoded as:
//! ```text
//! [network_magic: uint, diffusion_mode: bool, peer_sharing: int, query: bool]
//! ```

use super::messages::{HandshakeMessage, RefuseReason};
use super::types::{
    DiffusionMode, NetworkMagic, NodeToNodeVersion, NodeToNodeVersionData, PeerSharing,
    VersionTable,
};
use super::HandshakeError;
use bytes::Bytes;
use minicbor::{Decoder, Encoder};
use std::collections::HashMap;

/// Encode a handshake message to CBOR bytes
pub fn encode_message(msg: &HandshakeMessage) -> Result<Bytes, HandshakeError> {
    let mut buf = Vec::with_capacity(512);
    {
        let mut encoder = Encoder::new(&mut buf);

        match msg {
            HandshakeMessage::MsgProposeVersions { versions } => {
                // [0, version_table]
                encoder
                    .array(2)
                    .map_err(|e| HandshakeError::EncodingError(e.to_string()))?
                    .u32(0)
                    .map_err(|e| HandshakeError::EncodingError(e.to_string()))?;
                encode_version_table(&mut encoder, versions)?;
            }

            HandshakeMessage::MsgReplyVersions { versions } => {
                // [0, version_table] (same as MsgProposeVersions)
                encoder
                    .array(2)
                    .map_err(|e| HandshakeError::EncodingError(e.to_string()))?
                    .u32(0)
                    .map_err(|e| HandshakeError::EncodingError(e.to_string()))?;
                encode_version_table(&mut encoder, versions)?;
            }

            HandshakeMessage::MsgAcceptVersion {
                version,
                version_data,
            } => {
                // [1, version_int, version_data]
                encoder
                    .array(3)
                    .map_err(|e| HandshakeError::EncodingError(e.to_string()))?
                    .u32(1)
                    .map_err(|e| HandshakeError::EncodingError(e.to_string()))?
                    .i64(version.to_tag())
                    .map_err(|e| HandshakeError::EncodingError(e.to_string()))?;
                encode_version_data(&mut encoder, version_data)?;
            }

            HandshakeMessage::MsgRefuse { reason } => {
                // [2, refuse_reason]
                encoder
                    .array(2)
                    .map_err(|e| HandshakeError::EncodingError(e.to_string()))?
                    .u32(2)
                    .map_err(|e| HandshakeError::EncodingError(e.to_string()))?;
                encode_refuse_reason(&mut encoder, reason)?;
            }

            HandshakeMessage::MsgQueryReply { versions } => {
                // [3, version_table]
                encoder
                    .array(2)
                    .map_err(|e| HandshakeError::EncodingError(e.to_string()))?
                    .u32(3)
                    .map_err(|e| HandshakeError::EncodingError(e.to_string()))?;
                encode_version_table(&mut encoder, versions)?;
            }
        }
    }

    Ok(Bytes::from(buf))
}

/// Decode a handshake message from CBOR bytes
pub fn decode_message(bytes: &[u8]) -> Result<HandshakeMessage, HandshakeError> {
    let mut decoder = Decoder::new(bytes);

    // Read array length
    let array_len = decoder
        .array()
        .map_err(|e| HandshakeError::DecodingError(e.to_string()))?
        .ok_or_else(|| HandshakeError::DecodingError("Expected definite array".to_string()))?;

    if array_len < 2 {
        return Err(HandshakeError::DecodingError(format!(
            "Array too short: expected at least 2 elements, got {}",
            array_len
        )));
    }

    // Read message tag
    let tag = decoder
        .u32()
        .map_err(|e| HandshakeError::DecodingError(e.to_string()))?;

    match tag {
        0 => {
            // MsgProposeVersions or MsgReplyVersions (indistinguishable in wire format)
            let versions = decode_version_table(&mut decoder)?;
            // We treat this as MsgProposeVersions when received as client
            // The protocol state machine will handle the distinction
            Ok(HandshakeMessage::MsgProposeVersions { versions })
        }

        1 => {
            // MsgAcceptVersion
            if array_len != 3 {
                return Err(HandshakeError::DecodingError(format!(
                    "MsgAcceptVersion expects 3 elements, got {}",
                    array_len
                )));
            }

            let version_tag = decoder
                .i64()
                .map_err(|e| HandshakeError::DecodingError(e.to_string()))?;
            let version = NodeToNodeVersion::from_tag(version_tag).ok_or_else(|| {
                HandshakeError::DecodingError(format!("Unknown version tag: {}", version_tag))
            })?;

            let version_data = decode_version_data(&mut decoder)?;

            Ok(HandshakeMessage::MsgAcceptVersion {
                version,
                version_data,
            })
        }

        2 => {
            // MsgRefuse
            let reason = decode_refuse_reason(&mut decoder)?;
            Ok(HandshakeMessage::MsgRefuse { reason })
        }

        3 => {
            // MsgQueryReply
            let versions = decode_version_table(&mut decoder)?;
            Ok(HandshakeMessage::MsgQueryReply { versions })
        }

        _ => Err(HandshakeError::DecodingError(format!(
            "Unknown message tag: {}",
            tag
        ))),
    }
}

/// Encode version table as CBOR map
fn encode_version_table(
    encoder: &mut Encoder<&mut Vec<u8>>,
    versions: &VersionTable,
) -> Result<(), HandshakeError> {
    // Encode as map: {version_int: version_data, ...}
    encoder
        .map(versions.len() as u64)
        .map_err(|e| HandshakeError::EncodingError(e.to_string()))?;

    // Sort versions for deterministic encoding (protocol requirement)
    let mut sorted_versions: Vec<_> = versions.iter().collect();
    sorted_versions.sort_by_key(|(v, _)| *v);

    for (version, version_data) in sorted_versions {
        encoder
            .i64(version.to_tag())
            .map_err(|e| HandshakeError::EncodingError(e.to_string()))?;
        encode_version_data(encoder, version_data)?;
    }

    Ok(())
}

/// Decode version table from CBOR map
fn decode_version_table(decoder: &mut Decoder) -> Result<VersionTable, HandshakeError> {
    let map_len = decoder
        .map()
        .map_err(|e| HandshakeError::DecodingError(e.to_string()))?
        .ok_or_else(|| HandshakeError::DecodingError("Expected definite map".to_string()))?;

    let mut versions = HashMap::new();

    for _ in 0..map_len {
        let version_tag = decoder
            .i64()
            .map_err(|e| HandshakeError::DecodingError(e.to_string()))?;

        // Try to decode version, but don't fail on unknown versions
        // (we'll report them in VersionMismatch)
        if let Some(version) = NodeToNodeVersion::from_tag(version_tag) {
            let version_data = decode_version_data(decoder)?;
            versions.insert(version, version_data);
        } else {
            // Skip unknown version data (consume the value)
            decoder
                .skip()
                .map_err(|e| HandshakeError::DecodingError(e.to_string()))?;
        }
    }

    Ok(versions)
}

/// Encode version data as CBOR array
fn encode_version_data(
    encoder: &mut Encoder<&mut Vec<u8>>,
    data: &NodeToNodeVersionData,
) -> Result<(), HandshakeError> {
    // [network_magic, diffusion_mode, peer_sharing, query]
    tracing::debug!(
        network_magic = data.network_magic.value(),
        diffusion_mode = data.diffusion_mode.to_cbor_bool(),
        peer_sharing = data.peer_sharing.to_cbor_int(),
        query = data.query,
        "Encoding version data"
    );

    encoder
        .array(4)
        .map_err(|e| HandshakeError::EncodingError(e.to_string()))?
        .u32(data.network_magic.value())
        .map_err(|e| HandshakeError::EncodingError(e.to_string()))?
        .bool(data.diffusion_mode.to_cbor_bool())
        .map_err(|e| HandshakeError::EncodingError(e.to_string()))?
        .i64(data.peer_sharing.to_cbor_int())
        .map_err(|e| HandshakeError::EncodingError(e.to_string()))?
        .bool(data.query)
        .map_err(|e| HandshakeError::EncodingError(e.to_string()))?;

    Ok(())
}

/// Decode version data from CBOR array
fn decode_version_data(decoder: &mut Decoder) -> Result<NodeToNodeVersionData, HandshakeError> {
    let array_len = decoder
        .array()
        .map_err(|e| HandshakeError::DecodingError(e.to_string()))?
        .ok_or_else(|| HandshakeError::DecodingError("Expected definite array".to_string()))?;

    if array_len != 4 {
        return Err(HandshakeError::DecodingError(format!(
            "Version data expects 4 elements, got {}",
            array_len
        )));
    }

    let network_magic = NetworkMagic::new(
        decoder
            .u32()
            .map_err(|e| HandshakeError::DecodingError(e.to_string()))?,
    );

    let diffusion_mode = DiffusionMode::from_cbor_bool(
        decoder
            .bool()
            .map_err(|e| HandshakeError::DecodingError(e.to_string()))?,
    );

    let peer_sharing_int = decoder
        .i64()
        .map_err(|e| HandshakeError::DecodingError(e.to_string()))?;
    let peer_sharing = PeerSharing::from_cbor_int(peer_sharing_int).ok_or_else(|| {
        HandshakeError::DecodingError(format!("Invalid peer sharing value: {}", peer_sharing_int))
    })?;

    let query = decoder
        .bool()
        .map_err(|e| HandshakeError::DecodingError(e.to_string()))?;

    Ok(NodeToNodeVersionData {
        network_magic,
        diffusion_mode,
        peer_sharing,
        query,
    })
}

/// Encode refuse reason as CBOR array
fn encode_refuse_reason(
    encoder: &mut Encoder<&mut Vec<u8>>,
    reason: &RefuseReason,
) -> Result<(), HandshakeError> {
    match reason {
        RefuseReason::VersionMismatch {
            proposed,
            unknown_tags,
        } => {
            // [0, [proposed_versions...], [unknown_tags...]]
            encoder
                .array(3)
                .map_err(|e| HandshakeError::EncodingError(e.to_string()))?
                .u32(0)
                .map_err(|e| HandshakeError::EncodingError(e.to_string()))?;

            // Encode proposed versions
            encoder
                .array(proposed.len() as u64)
                .map_err(|e| HandshakeError::EncodingError(e.to_string()))?;
            for version in proposed {
                encoder
                    .i64(version.to_tag())
                    .map_err(|e| HandshakeError::EncodingError(e.to_string()))?;
            }

            // Encode unknown tags
            encoder
                .array(unknown_tags.len() as u64)
                .map_err(|e| HandshakeError::EncodingError(e.to_string()))?;
            for tag in unknown_tags {
                encoder
                    .i64(*tag)
                    .map_err(|e| HandshakeError::EncodingError(e.to_string()))?;
            }
        }

        RefuseReason::HandshakeDecodeError { version, error } => {
            // [1, version_int, error_string]
            encoder
                .array(3)
                .map_err(|e| HandshakeError::EncodingError(e.to_string()))?
                .u32(1)
                .map_err(|e| HandshakeError::EncodingError(e.to_string()))?
                .i64(version.to_tag())
                .map_err(|e| HandshakeError::EncodingError(e.to_string()))?
                .str(error)
                .map_err(|e| HandshakeError::EncodingError(e.to_string()))?;
        }

        RefuseReason::Refused { version, reason } => {
            // [2, version_int, reason_string]
            encoder
                .array(3)
                .map_err(|e| HandshakeError::EncodingError(e.to_string()))?
                .u32(2)
                .map_err(|e| HandshakeError::EncodingError(e.to_string()))?
                .i64(version.to_tag())
                .map_err(|e| HandshakeError::EncodingError(e.to_string()))?
                .str(reason)
                .map_err(|e| HandshakeError::EncodingError(e.to_string()))?;
        }
    }

    Ok(())
}

/// Decode refuse reason from CBOR array
fn decode_refuse_reason(decoder: &mut Decoder) -> Result<RefuseReason, HandshakeError> {
    let array_len = decoder
        .array()
        .map_err(|e| HandshakeError::DecodingError(e.to_string()))?
        .ok_or_else(|| HandshakeError::DecodingError("Expected definite array".to_string()))?;

    if array_len < 1 {
        return Err(HandshakeError::DecodingError(
            "Refuse reason array too short".to_string(),
        ));
    }

    let tag = decoder
        .u32()
        .map_err(|e| HandshakeError::DecodingError(e.to_string()))?;

    match tag {
        0 => {
            // VersionMismatch
            let proposed_len = decoder
                .array()
                .map_err(|e| HandshakeError::DecodingError(e.to_string()))?
                .ok_or_else(|| {
                    HandshakeError::DecodingError("Expected definite array".to_string())
                })?;

            let mut proposed = Vec::new();
            for _ in 0..proposed_len {
                let version_tag = decoder
                    .i64()
                    .map_err(|e| HandshakeError::DecodingError(e.to_string()))?;
                if let Some(version) = NodeToNodeVersion::from_tag(version_tag) {
                    proposed.push(version);
                }
            }

            let unknown_len = decoder
                .array()
                .map_err(|e| HandshakeError::DecodingError(e.to_string()))?
                .ok_or_else(|| {
                    HandshakeError::DecodingError("Expected definite array".to_string())
                })?;

            let mut unknown_tags = Vec::new();
            for _ in 0..unknown_len {
                unknown_tags.push(
                    decoder
                        .i64()
                        .map_err(|e| HandshakeError::DecodingError(e.to_string()))?,
                );
            }

            Ok(RefuseReason::VersionMismatch {
                proposed,
                unknown_tags,
            })
        }

        1 => {
            // HandshakeDecodeError
            let version_tag = decoder
                .i64()
                .map_err(|e| HandshakeError::DecodingError(e.to_string()))?;
            let version = NodeToNodeVersion::from_tag(version_tag).ok_or_else(|| {
                HandshakeError::DecodingError(format!("Unknown version tag: {}", version_tag))
            })?;

            let error = decoder
                .str()
                .map_err(|e| HandshakeError::DecodingError(e.to_string()))?
                .to_string();

            Ok(RefuseReason::HandshakeDecodeError { version, error })
        }

        2 => {
            // Refused
            let version_tag = decoder
                .i64()
                .map_err(|e| HandshakeError::DecodingError(e.to_string()))?;
            let version = NodeToNodeVersion::from_tag(version_tag).ok_or_else(|| {
                HandshakeError::DecodingError(format!("Unknown version tag: {}", version_tag))
            })?;

            let reason = decoder
                .str()
                .map_err(|e| HandshakeError::DecodingError(e.to_string()))?
                .to_string();

            Ok(RefuseReason::Refused { version, reason })
        }

        _ => Err(HandshakeError::DecodingError(format!(
            "Unknown refuse reason tag: {}",
            tag
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_version_table() -> VersionTable {
        let mut versions = HashMap::new();
        versions.insert(
            NodeToNodeVersion::V14,
            NodeToNodeVersionData::preview_testnet(),
        );
        versions.insert(
            NodeToNodeVersion::V15,
            NodeToNodeVersionData::preview_testnet(),
        );
        versions
    }

    #[test]
    fn test_encode_decode_propose_versions() {
        let msg = HandshakeMessage::MsgProposeVersions {
            versions: create_test_version_table(),
        };

        let encoded = encode_message(&msg).unwrap();
        let decoded = decode_message(&encoded).unwrap();

        match decoded {
            HandshakeMessage::MsgProposeVersions { versions } => {
                assert_eq!(versions.len(), 2);
                assert!(versions.contains_key(&NodeToNodeVersion::V14));
                assert!(versions.contains_key(&NodeToNodeVersion::V15));
            }
            _ => panic!("Wrong message type decoded"),
        }
    }

    #[test]
    fn test_encode_decode_accept_version() {
        let msg = HandshakeMessage::MsgAcceptVersion {
            version: NodeToNodeVersion::V15,
            version_data: NodeToNodeVersionData::preview_testnet(),
        };

        let encoded = encode_message(&msg).unwrap();
        let decoded = decode_message(&encoded).unwrap();

        match decoded {
            HandshakeMessage::MsgAcceptVersion {
                version,
                version_data,
            } => {
                assert_eq!(version, NodeToNodeVersion::V15);
                assert_eq!(version_data.network_magic, NetworkMagic::PREVIEW_TESTNET);
            }
            _ => panic!("Wrong message type decoded"),
        }
    }

    #[test]
    fn test_encode_decode_refuse() {
        let msg = HandshakeMessage::MsgRefuse {
            reason: RefuseReason::VersionMismatch {
                proposed: vec![NodeToNodeVersion::V14],
                unknown_tags: vec![99, 100],
            },
        };

        let encoded = encode_message(&msg).unwrap();
        let decoded = decode_message(&encoded).unwrap();

        match decoded {
            HandshakeMessage::MsgRefuse { reason } => match reason {
                RefuseReason::VersionMismatch {
                    proposed,
                    unknown_tags,
                } => {
                    assert_eq!(proposed, vec![NodeToNodeVersion::V14]);
                    assert_eq!(unknown_tags, vec![99, 100]);
                }
                _ => panic!("Wrong refuse reason"),
            },
            _ => panic!("Wrong message type decoded"),
        }
    }
    #[test]
    fn test_version_data_roundtrip() {
        let data = NodeToNodeVersionData::new(
            NetworkMagic::PREVIEW_TESTNET,
            DiffusionMode::InitiatorAndResponder,
            PeerSharing::Enabled,
            true,
        );

        let mut buf = Vec::new();
        {
            let mut encoder = Encoder::new(&mut buf);
            encode_version_data(&mut encoder, &data).unwrap();
        }

        let mut decoder = Decoder::new(&buf);
        let decoded = decode_version_data(&mut decoder).unwrap();

        assert_eq!(decoded, data);
    }
}
