use blake2::{Blake2s256, Digest};
use hex_literal::hex;

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
}
