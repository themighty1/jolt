#![cfg_attr(feature = "guest", no_std)]

extern crate alloc;
use alloc::vec::Vec;

use serde::Deserialize;

#[derive(Deserialize)]
#[allow(dead_code)]
struct Record {
    id: u32,
    amount: u64,
    valid: bool,
    label: heapless::String<16>,
}

#[derive(Deserialize)]
struct Data {
    records: heapless::Vec<Record, 128>,
}

fn lookup(data: &Data, query: &[u8]) -> u64 {
    // query format: "records.<index>.<field>"
    let mut i = 0;
    while i < query.len() && query[i] != b'.' {
        i += 1;
    }
    i += 1;

    let mut idx: usize = 0;
    while i < query.len() && query[i] != b'.' {
        idx = idx * 10 + (query[i] - b'0') as usize;
        i += 1;
    }
    i += 1;

    let record = &data.records[idx];
    let field = &query[i..];

    match field {
        b"id" => record.id as u64,
        b"amount" => record.amount,
        b"valid" => {
            if record.valid {
                1
            } else {
                0
            }
        }
        _ => panic!("unknown field"),
    }
}

#[jolt::provable(
    max_input_size = 4096,
    max_untrusted_advice_size = 8192,
    stack_size = 131072,
    max_trace_length = 1048576
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

    let (data, _) = serde_json_core::from_slice::<Data>(json_bytes).expect("invalid JSON");
    (lookup(&data, query), hash_lo, hash_hi)
}
