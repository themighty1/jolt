use serde::Serialize;
use std::time::Instant;

#[derive(Serialize)]
struct Record {
    id: u32,
    amount: u64,
    valid: bool,
    label: String,
}

fn build_json(count: u32) -> Vec<u8> {
    let records: Vec<Record> = (0..count)
        .map(|i| Record {
            id: i,
            amount: (i as u64 + 1) * 100,
            valid: i % 3 != 0,
            label: format!("item_{:04}", i),
        })
        .collect();

    let wrapper = serde_json::json!({ "records": records });
    serde_json::to_vec(&wrapper).unwrap()
}

pub fn main() {
    tracing_subscriber::fmt::init();

    let size_kb: u32 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);

    let count = match size_kb {
        1 => 18,
        2 => 36,
        4 => 72,
        _ => {
            eprintln!("Usage: json-parse <1|2|4> [query]");
            eprintln!("  Size in KB: 1, 2, or 4");
            std::process::exit(1);
        }
    };

    let query = std::env::args()
        .nth(2)
        .unwrap_or_else(|| format!("records.{}.amount", count - 1));

    let json_bytes = build_json(count);
    println!("Size: ~{}KB ({} records, {} bytes)", size_kb, count, json_bytes.len());
    println!("Query: {query}");

    let summary = guest::analyze_json_query(
        query.as_bytes(),
        jolt_sdk::UntrustedAdvice::new(json_bytes.clone()),
    );
    println!("Trace cycles: {}", summary.trace.len());

    let target_dir = "/tmp/jolt-guest-targets";
    let mut program = guest::compile_json_query(target_dir);

    let shared_preprocessing =
        guest::preprocess_shared_json_query(&mut program).unwrap();
    let prover_preprocessing =
        guest::preprocess_prover_json_query(shared_preprocessing.clone());

    let verifier_preprocessing = guest::preprocess_verifier_json_query(
        shared_preprocessing,
        prover_preprocessing.generators.to_verifier_setup(),
        Some(prover_preprocessing.blindfold_setup()),
    );

    let prove_json_query =
        guest::build_prover_json_query(program, prover_preprocessing);
    let verify_json_query =
        guest::build_verifier_json_query(verifier_preprocessing);

    let query_bytes = query.as_bytes();

    let now = Instant::now();
    let (output, proof, io_device) =
        prove_json_query(query_bytes, jolt_sdk::PrivateInput::new(json_bytes));
    let prove_elapsed = now.elapsed();

    let (value, hash_lo, hash_hi) = output;

    let now = Instant::now();
    let is_valid = verify_json_query(query_bytes, output, io_device.panic, proof);
    let verify_elapsed = now.elapsed();

    println!("--- Result ---");
    println!("Value at \"{query}\": {value}");
    println!("Blake2b hash of JSON: {}{}", hex::encode(hash_lo), hex::encode(hash_hi));
    println!("Prove time: {:.3}s", prove_elapsed.as_secs_f64());
    println!("Verify time: {:.3}s", verify_elapsed.as_secs_f64());
    println!("Proof valid: {is_valid}");
}
