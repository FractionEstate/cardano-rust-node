use cardano_crypto::vrf::{
    VrfOutput,
    VrfPrivateKey,
    VrfProof,
    VrfPublicKey,
    VRF_OUTPUT_LENGTH,
    VRF_PRIVATE_KEY_LENGTH,
    VRF_PROOF_LENGTH,
    VRF_PUBLIC_KEY_LENGTH,
    VRF_SEED_LENGTH,
};

#[derive(Debug, Clone, Copy)]
pub struct PraosVrfTestVector {
    pub name: &'static str,
    pub sk_seed_hex: &'static str,
    pub pk_hex: &'static str,
    pub alpha_hex: Option<&'static str>,
    pub proof_hex: &'static str,
    pub output_hex: &'static str,
}

pub const PRAOS_VRF_TEST_VECTORS: &[PraosVrfTestVector] = &[
    PraosVrfTestVector {
        name: "vrf_ver03_generated_1",
        sk_seed_hex: "0000000000000000000000000000000000000000000000000000000000000000",
        pk_hex: "3b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29",
        alpha_hex: Some("00"),
        proof_hex: "000f006e64c91f84212919fe0899970cd341206fc081fe599339c8492e2cea3299ae9de4b6ce21cda0a975f65f45b70f82b3952ba6d0dbe11a06716e67aca233c0d78f115a655aa1952ada9f3d692a0a",
        output_hex: "9930b5dddc0938f01cf6f9746eded569ee676bd6ff3b4f19233d74b903ec53a45c5728116088b7c622b6d6c354f7125c7d09870b56ec6f1e4bf4970f607e04b2",
    },
    PraosVrfTestVector {
        name: "vrf_ver03_generated_2",
        sk_seed_hex: "0000000000000000000000000000000000000000000000000000000000000000",
        pk_hex: "3b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29",
        alpha_hex: Some("00010203040506070809"),
        proof_hex: "0031f929352875995e3d55c4abdac7bfb92e706beb182999dd7d78f61e1bdc3f83b746a9ae6caee317a7c47597ece1801799c06ca2180cdb5392677cd8815353c1d0d5691956b3be52b322be049fc20c",
        output_hex: "ca4171883d173a3f03bdb87c45ce349f0bb168ca8171d64f9b9aeaf20d0869bab9f74e819ccdc6754656468ccc2aa85e5f903a31375a39be84464fa515b51512",
    },
    PraosVrfTestVector {
        name: "vrf_ver03_generated_3",
        sk_seed_hex: "a70b8f607568df8ae26cf438b1057d8d0a94b7f3ac44cd984577fc43c2da55b7",
        pk_hex: "f1eb347d5c59e24f9f5f33c80cfd866e79fd72e0c370da3c011b1c9f045e23f1",
        alpha_hex: Some("00"),
        proof_hex: "aa349327d919c8c96de316855de6fe5fa841ef25af913cfb9b33d6b663c425bd024456ca193f10da319a2205c67222e8a62da87101904f453de0beb79568902cedeea891f3db8202690f51c8e7d3210b",
        output_hex: "d4b4deef941fc3ece4e86f837c784951b4a0cbc4accd79cdcbc882123befeb17c63b329730c59bbe9253294496f730428d588b9221832cb336bfd9d67754030f",
    },
    PraosVrfTestVector {
        name: "vrf_ver03_generated_4",
        sk_seed_hex: "a70b8f607568df8ae26cf438b1057d8d0a94b7f3ac44cd984577fc43c2da55b7",
        pk_hex: "f1eb347d5c59e24f9f5f33c80cfd866e79fd72e0c370da3c011b1c9f045e23f1",
        alpha_hex: Some("00010203040506070809"),
        proof_hex: "989c0c477b4a0c07e0dabd7b73cdb42beb4b4e09471377e6d0b75e8ffd5d091704394c5ea4e2be5d5244b02c03cf85984adfa12c61280bc8c6e46f02035ee57d6cd18b96695ea04ff5ec541869ea890a",
        output_hex: "933f886e8648796a968dccc71a3ce09a8026b28fdf5ffcc50be4b97431f3e3904375870b0bd196509dc33606846bb14820acdf36170e1667dbe9d3a940717bbd",
    },
    PraosVrfTestVector {
        name: "vrf_ver03_standard_10",
        sk_seed_hex: "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60",
        pk_hex: "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a",
        alpha_hex: None,
        proof_hex: "b6b4699f87d56126c9117a7da55bd0085246f4c56dbc95d20172612e9d38e8d7ca65e573a126ed88d4e30a46f80a666854d675cf3ba81de0de043c3774f061560f55edc256a787afe701677c0f602900",
        output_hex: "5b49b554d05c0cd5a5325376b3387de59d924fd1e13ded44648ab33c21349a603f25b84ec5ed887995b33da5e3bfcb87cd2f64521c4c62cf825cffabbe5d31cc",
    },
    PraosVrfTestVector {
        name: "vrf_ver03_standard_11",
        sk_seed_hex: "4ccd089b28ff96da9db6c346ec114e0f5b8a319f35aba624da8cf6ed4fb8a6fb",
        pk_hex: "3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c",
        alpha_hex: Some("72"),
        proof_hex: "ae5b66bdf04b4c010bfe32b2fc126ead2107b697634f6f7337b9bff8785ee111200095ece87dde4dbe87343f6df3b107d91798c8a7eb1245d3bb9c5aafb093358c13e6ae1111a55717e895fd15f99f07",
        output_hex: "94f4487e1b2fec954309ef1289ecb2e15043a2461ecc7b2ae7d4470607ef82eb1cfa97d84991fe4a7bfdfd715606bc27e2967a6c557cfb5875879b671740b7d8",
    },
    PraosVrfTestVector {
        name: "vrf_ver03_standard_12",
        sk_seed_hex: "c5aa8df43f9f837bedb7442f31dcb7b166d38535076f094b85ce3a2e0b4458f7",
        pk_hex: "fc51cd8e6218a1a38da47ed00230f0580816ed13ba3303ac5deb911548908025",
        alpha_hex: Some("af82"),
        proof_hex: "dfa2cba34b611cc8c833a6ea83b8eb1bb5e2ef2dd1b0c481bc42ff36ae7847f6ab52b976cfd5def172fa412defde270c8b8bdfbaae1c7ece17d9833b1bcf31064fff78ef493f820055b561ece45e1009",
        output_hex: "2031837f582cd17a9af9e0c7ef5a6540e3453ed894b62c293686ca3c1e319dde9d0aa489a4b59a9594fc2328bc3deff3c8a0929a369a72b1180a596e016b5ded",
    },
];

fn first_vector() -> &'static PraosVrfTestVector {
    PRAOS_VRF_TEST_VECTORS
        .first()
        .expect("at least one VRF test vector is available")
}

pub fn vrf_seed_bytes() -> Vec<u8> {
    hex::decode(first_vector().sk_seed_hex).expect("seed hex string is valid")
}

pub fn vrf_public_key_bytes() -> Vec<u8> {
    hex::decode(first_vector().pk_hex).expect("public key hex string is valid")
}

pub fn vrf_proof_bytes() -> Vec<u8> {
    hex::decode(first_vector().proof_hex).expect("proof hex string is valid")
}

pub fn vrf_output_bytes() -> Vec<u8> {
    hex::decode(first_vector().output_hex).expect("output hex string is valid")
}

pub fn vrf_private_key_bytes() -> Vec<u8> {
    let mut bytes = vrf_seed_bytes();
    bytes.extend_from_slice(&vrf_public_key_bytes());
    bytes
}

pub fn vrf_private_key() -> VrfPrivateKey {
    VrfPrivateKey::from_bytes(&vrf_private_key_bytes())
        .expect("golden vector secret key is valid")
}

pub fn vrf_public_key() -> VrfPublicKey {
    VrfPublicKey::from_bytes(&vrf_public_key_bytes())
        .expect("golden vector public key is valid")
}

pub fn vrf_proof() -> VrfProof {
    VrfProof::from_bytes(vrf_proof_bytes()).expect("golden vector VRF proof is valid")
}

pub fn vrf_output() -> VrfOutput {
    VrfOutput::from_bytes(vrf_output_bytes()).expect("golden vector VRF output is valid")
}

pub fn vrf_fixture(tag: &str) -> (VrfOutput, VrfProof) {
    let message = format!("cardano_vrf_fixture::{tag}");
    vrf_private_key()
        .try_prove(message.as_bytes())
        .expect("fixture VRF proof generation succeeds")
}

pub fn vrf_fixture_output(tag: &str) -> VrfOutput {
    vrf_fixture(tag).0
}

pub fn vrf_fixture_proof(tag: &str) -> VrfProof {
    vrf_fixture(tag).1
}

pub fn vrf_prove_message(message: &[u8]) -> (VrfOutput, VrfProof) {
    vrf_private_key()
        .try_prove(message)
        .expect("VRF proof generation succeeds")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_length_matches_constant() {
        assert_eq!(vrf_seed_bytes().len(), VRF_SEED_LENGTH);
    }

    #[test]
    fn public_key_length_matches_constant() {
        assert_eq!(vrf_public_key_bytes().len(), VRF_PUBLIC_KEY_LENGTH);
    }

    #[test]
    fn proof_length_matches_constant() {
        assert_eq!(vrf_proof_bytes().len(), VRF_PROOF_LENGTH);
    }

    #[test]
    fn output_length_matches_constant() {
        assert_eq!(vrf_output_bytes().len(), VRF_OUTPUT_LENGTH);
    }

    #[test]
    fn private_key_length_matches_constant() {
        let mut bytes = vrf_seed_bytes();
        bytes.extend_from_slice(&vrf_public_key_bytes());
        assert_eq!(bytes.len(), VRF_PRIVATE_KEY_LENGTH);
    }

    #[test]
    fn typed_accessors_roundtrip() {
        let sk = vrf_private_key();
        let pk = vrf_public_key();
        let proof = vrf_proof();
        let output = vrf_output();

        assert_eq!(sk.to_bytes().len(), VRF_PRIVATE_KEY_LENGTH);
        assert_eq!(pk.to_bytes().len(), VRF_PUBLIC_KEY_LENGTH);
        assert_eq!(proof.to_bytes().len(), VRF_PROOF_LENGTH);
        assert_eq!(output.to_bytes().len(), VRF_OUTPUT_LENGTH);
    }

    #[test]
    fn fixture_helpers_generate_valid_pairs() {
        let (output, proof) = vrf_fixture("test");
        assert_eq!(output.to_bytes().len(), VRF_OUTPUT_LENGTH);
        assert_eq!(proof.to_bytes().len(), VRF_PROOF_LENGTH);

        let message = b"fixture_message";
        let (direct_output, direct_proof) = vrf_prove_message(message);
        assert_eq!(direct_output.to_bytes().len(), VRF_OUTPUT_LENGTH);
        assert_eq!(direct_proof.to_bytes().len(), VRF_PROOF_LENGTH);
    }
}
