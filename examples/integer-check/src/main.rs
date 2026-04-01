use std::time::Instant;

pub fn main() {
    tracing_subscriber::fmt::init();

    let input = std::env::args()
        .nth(1)
        .expect("Usage: integer-check <string>");

    let target_dir = "/tmp/jolt-guest-targets";

    let mut program = guest::compile_integer_check(target_dir);

    let shared_preprocessing =
        guest::preprocess_shared_integer_check(&mut program).unwrap();
    let prover_preprocessing =
        guest::preprocess_prover_integer_check(shared_preprocessing.clone());
    let verifier_preprocessing = guest::preprocess_verifier_integer_check(
        shared_preprocessing,
        prover_preprocessing.generators.to_verifier_setup(),
        Some(prover_preprocessing.blindfold_setup()),
    );

    let prove_integer_check =
        guest::build_prover_integer_check(program, prover_preprocessing);
    let verify_integer_check =
        guest::build_verifier_integer_check(verifier_preprocessing);

    let input_bytes = input.as_bytes();

    let summary = guest::analyze_integer_check(input_bytes);
    println!("Trace cycles: {}", summary.trace.len());

    let now = Instant::now();
    let (output, proof, io_device) = prove_integer_check(input_bytes);
    let prove_elapsed = now.elapsed();

    let (result, hash) = output;

    let now = Instant::now();
    let is_valid = verify_integer_check(input_bytes, output, io_device.panic, proof);
    let verify_elapsed = now.elapsed();

    println!("--- Result ---");
    println!("Input: \"{input}\"");
    println!("Result: {result}");
    println!("Blake3 hash: {}", hex::encode(hash));
    println!("Prove time: {:.3}s", prove_elapsed.as_secs_f64());
    println!("Verify time: {:.3}s", verify_elapsed.as_secs_f64());
    println!("Proof valid: {is_valid}");
    if result == 1 {
        println!("Proven: the input is a valid integer > 700");
    } else {
        println!("Proven: the input is NOT a valid integer > 700");
    }
}
