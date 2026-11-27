// Building blocks for PDAs and Anchor discriminators (given, not an exercise).
//
// TOY CURVE: real Solana rejects a candidate PDA if it is a valid ed25519
// point. Doing the point decompression here would add a crypto dependency for
// no teaching value, so `is_on_curve` is a stand-in that, like the real test,
// rejects roughly half of all hashes. Everything else (hash input order,
// bump search from 255 down, seed limits) matches `solana_program`.

use sha2::{Digest, Sha256};

pub const MAX_SEED_LEN: usize = 32;
pub const MAX_SEEDS: usize = 16;
pub const PDA_MARKER: &[u8; 21] = b"ProgramDerivedAddress";

/// Toy stand-in for "is this 32-byte string a valid ed25519 point?".
pub fn is_on_curve(bytes: &[u8; 32]) -> bool {
    bytes[31] & 1 == 1
}

/// SHA-256 over the concatenation of `parts`.
pub fn sha256(parts: &[&[u8]]) -> [u8; 32] {
    let mut h = Sha256::new();
    for p in parts {
        h.update(p);
    }
    h.finalize().into()
}
