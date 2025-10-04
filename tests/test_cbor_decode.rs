// Quick test to decode the handshake CBOR
use bytes::Bytes;

fn main() {
    let hex_data = "8200a20e841a4170cb17f400f40f841a4170cb17f400f4";
    let data = hex::decode(hex_data).expect("Invalid hex");

    println!("Raw hex: {}", hex_data);
    println!("Length: {} bytes", data.len());
    println!("\nBytes breakdown:");
    for (i, byte) in data.iter().enumerate() {
        print!("{:02x} ", byte);
        if (i + 1) % 8 == 0 {
            println!();
        }
    }
    println!("\n");

    // Manual CBOR decode
    println!("CBOR Analysis:");
    println!("0x82 = array(2) - MsgProposeVersions wrapper");
    println!("0x00 = uint(0) - message tag 0");
    println!("0xa2 = map(2) - version table with 2 entries");
    println!("  Entry 1:");
    println!("    0x0e = uint(14) - Version V14");
    println!("    0x84 = array(4) - version data");
    println!("      0x1a 0x4170cb17 = uint(1097911063) - network magic (PREVIEW)");
    println!("      0xf4 = false - diffusion mode");
    println!("      0x00 = uint(0) - peer sharing");
    println!("      0xf4 = false - query mode");
    println!("  Entry 2:");
    println!("    0x0f = uint(15) - Version V15");
    println!("    0x84 = array(4) - version data");
    println!("      0x1a 0x4170cb17 = uint(1097911063) - network magic (PREVIEW)");
    println!("      0xf4 = false - diffusion mode");
    println!("      0x00 = uint(0) - peer sharing");
    println!("      0xf4 = false - query mode");
}
