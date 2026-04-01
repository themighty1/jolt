#![cfg_attr(feature = "guest", no_std)]

extern crate alloc;
use alloc::vec::Vec;

#[jolt::provable(
    max_input_size = 4096,
    max_untrusted_advice_size = 65536,
    stack_size = 65536,
    max_trace_length = 4194304
)]
fn json_query(query: &[u8], json_data: jolt::PrivateInput<Vec<u8>>) -> (u64, [u8; 32], [u8; 32]) {
    let json_bytes = &*json_data;

    let full_hash = jolt_inlines_blake2::Blake2b::digest(json_bytes);
    let mut hash_lo = [0u8; 32];
    let mut hash_hi = [0u8; 32];
    let mut i = 0;
    while i < 32 {
        hash_lo[i] = full_hash[i];
        hash_hi[i] = full_hash[i + 32];
        i += 1;
    }

    let json_str = core::str::from_utf8(json_bytes).expect("invalid UTF-8");
    let query_str = core::str::from_utf8(query).expect("invalid query");
    let value = gjson::get(json_str, query_str);
    (value.u64(), hash_lo, hash_hi)
}
