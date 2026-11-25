//! A stable hash of byte strings.

/// Returns the 64-bit FNV-1a hash of `parts`, each part ended by its length.
///
/// The value depends only on the bytes, so it is the same in every build and on every machine.
pub fn fingerprint(parts: &[&[u8]]) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0100_0000_01b3;
    let mut hash = OFFSET;
    for part in parts {
        let length = (part.len() as u64).to_le_bytes();
        for &byte in part.iter().chain(&length) {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(PRIME);
        }
    }
    hash
}
