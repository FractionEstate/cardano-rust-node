use minicbor::{Decoder};

fn main() {
    let hex = "8200a20e841a4170cb17f400f40f841a4170cb17f400f4";
    let bytes = hex::decode(hex).unwrap();

    println!("Hex bytes: {:?}", bytes);
    println!("Length: {}", bytes.len());

    let mut decoder = Decoder::new(&bytes);

    // Outer array
    if let Ok(Some(array_len)) = decoder.array() {
        println!("\nOuter array length: {}", array_len);

        // Element 0: message tag
        if let Ok(tag) = decoder.u32() {
            println!("  [0] message tag: {} (MsgProposeVersions)", tag);
        }

        // Element 1: version table (map)
        if let Ok(Some(map_len)) = decoder.map() {
            println!("  [1] version table map length: {}", map_len);

            for i in 0..map_len {
                // Version number (key)
                if let Ok(version) = decoder.i64() {
                    println!("    Version {}: {}", i, version);

                    // Version data (value) - array of 4 elements
                    if let Ok(Some(vdata_len)) = decoder.array() {
                        println!("      Version data array length: {}", vdata_len);

                        // Network magic
                        if let Ok(magic) = decoder.u32() {
                            println!("        [0] network_magic: {} (0x{:x})", magic, magic);
                        }

                        // Diffusion mode
                        if let Ok(diffusion) = decoder.bool() {
                            println!("        [1] diffusion_mode: {}", diffusion);
                        }

                        // Peer sharing
                        if let Ok(peer_sharing) = decoder.i64() {
                            println!("        [2] peer_sharing: {}", peer_sharing);
                        }

                        // Query
                        if let Ok(query) = decoder.bool() {
                            println!("        [3] query: {}", query);
                        }
                    }
                }
            }
        }
    }
}
