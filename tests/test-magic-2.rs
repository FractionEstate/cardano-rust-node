use cardano_network::protocols::handshake::types::NetworkMagic;

fn main() {
    let magic = NetworkMagic(2);
    println!("Testing with magic: {}", magic.value());
    println!("Network: {}", magic.network_name());
}
