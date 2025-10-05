#![no_main]

use libfuzzer_sys::fuzz_target;
use cardano_network::connection::handshake::{HandshakeMessage, HandshakeCodec};
use tokio_util::codec::Decoder;
use bytes::BytesMut;

fuzz_target!(|data: &[u8]| {
    // Fuzz handshake message CBOR decoding
    // This tests for crashes, panics, or unexpected behavior when parsing malformed input
    
    let mut codec = HandshakeCodec;
    let mut buf = BytesMut::from(data);
    
    // Try to decode - should never panic, only return Ok or Err
    let _ = codec.decode(&mut buf);
});

