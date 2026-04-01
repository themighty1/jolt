use std::sync::OnceLock;
use wasm_bindgen::prelude::*;

extern crate jolt_inlines_blake3;

use common::jolt_device::{MemoryConfig, MemoryLayout};
use jolt_core::ark_bn254::Fr;
use jolt_core::curve::Bn254Curve;
use jolt_core::guest;
use jolt_core::poly::commitment::dory::DoryCommitmentScheme;
use jolt_core::transcripts::Blake2bTranscript;
use jolt_core::zkvm::prover::JoltProverPreprocessing;
use jolt_core::zkvm::verifier::JoltSharedPreprocessing;

pub use wasm_bindgen_rayon::init_thread_pool;

const GUEST_ELF: &[u8] = include_bytes!("../guest.elf");

fn base_memory_config() -> MemoryConfig {
    MemoryConfig {
        heap_size: 32768,
        stack_size: 4096,
        max_input_size: 4096,
        max_output_size: 4096,
        max_untrusted_advice_size: 4096,
        max_trusted_advice_size: 4096,
        program_size: None,
    }
}

struct CachedPreprocessing {
    program: guest::program::Program,
    preprocessing: JoltProverPreprocessing<Fr, Bn254Curve, DoryCommitmentScheme>,
}

static CACHED: OnceLock<CachedPreprocessing> = OnceLock::new();

fn get_cached() -> &'static CachedPreprocessing {
    CACHED.get_or_init(|| {
        let program = guest::program::Program::new(GUEST_ELF, &base_memory_config());
        let (bytecode, memory_init, program_size, e_entry) = program.decode();

        let mut mem_config = base_memory_config();
        mem_config.program_size = Some(program_size);
        let memory_layout = MemoryLayout::new(&mem_config);

        let shared = JoltSharedPreprocessing::new(
            bytecode,
            memory_layout,
            memory_init,
            65536,
            e_entry,
        )
        .expect("preprocessing failed");

        let preprocessing = JoltProverPreprocessing::new(shared);
        CachedPreprocessing { program, preprocessing }
    })
}

#[wasm_bindgen]
pub fn preprocess() {
    console_error_panic_hook::set_once();
    get_cached();
}

#[wasm_bindgen]
pub fn prove_integer_check(input: &str) -> String {
    let cached = get_cached();

    let input_bytes = postcard::to_stdvec(&input.as_bytes()).unwrap();
    let mut output_bytes = vec![0u8; 4096];

    let (_proof, io_device, _) = guest::prover::prove::<
        Fr,
        Bn254Curve,
        DoryCommitmentScheme,
        Blake2bTranscript,
    >(
        &cached.program,
        &input_bytes,
        &[],
        &[],
        None,
        None,
        &mut output_bytes,
        &cached.preprocessing,
    );

    let (result, hash): (u32, [u8; 32]) =
        postcard::from_bytes(&io_device.outputs).unwrap_or((0, [0; 32]));
    let valid = !io_device.panic;
    let hash_hex = hex::encode(hash);

    format!(
        r#"{{"output": {}, "valid": {}, "is_gt_700": {}, "blake3_hash": "{}"}}"#,
        result,
        valid,
        result == 1,
        hash_hex
    )
}
