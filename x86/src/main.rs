use blake2::{Blake2s256, Digest};
use hex_literal::hex;
use sha2::{Digest as Sha2Digest, Sha256};

fn main() {
    let mut hasher = Blake2s256::new();
    hasher.update(b"hello world");
    let res = hasher.finalize();
    assert_eq!(
        res[..],
        hex!(
            "
    9aec6806794561107e594b1f6a8a6b0c92a0cba9acf5e5e93cca06f781813b0b
"
        )[..]
    );

    let hash = Sha256::digest(b"hello world");
    assert_eq!(
        hash,
        hex!("b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9")
    );
}
