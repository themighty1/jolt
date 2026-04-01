#!/usr/bin/env bash
# Shared WASM build script for all Jolt WASM examples.
# Usage: ./wasm-build.sh [crate-name] [output-dir]
#   If no crate-name is given, builds all *-wasm examples.
# Example: ./wasm-build.sh integer-check-wasm ./examples/integer-check-wasm/pkg

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${REPO_ROOT}"

GUEST_TARGET_DIR="/tmp/jolt-guest-targets"

# Rebuild guest.elf if the guest source is newer than the existing ELF.
# The native host binary compiles the guest via the SDK with the correct
# linker script and memory layout — manual `jolt build` does not match.
ensure_guest_elf() {
    local CRATE="$1"
    # foo-wasm -> foo
    local NATIVE_CRATE="${CRATE%-wasm}"
    local GUEST_DIR="./examples/${NATIVE_CRATE}/guest"
    local GUEST_ELF="./examples/${CRATE}/guest.elf"

    if [ ! -d "${GUEST_DIR}" ]; then
        return
    fi

    # Check if guest.elf is missing or older than any guest source file
    local stale=false
    if [ ! -f "${GUEST_ELF}" ]; then
        stale=true
    elif [ -n "$(find "${GUEST_DIR}" \( -name '*.rs' -o -name 'Cargo.toml' \) -newer "${GUEST_ELF}" 2>/dev/null)" ]; then
        stale=true
    fi

    if [ "${stale}" = "false" ]; then
        echo "=== guest.elf is up to date for ${CRATE} ==="
        return
    fi

    echo "=== guest.elf is stale for ${CRATE}, rebuilding via native host ==="

    # Build the native host binary (doesn't run it yet)
    cargo build -p "${NATIVE_CRATE}" --release -q

    # Run with minimal input to trigger guest compilation.
    # The SDK's compile_* builds the guest ELF with the correct memory layout.
    # We need to actually run the binary since guest compilation happens at runtime.
    # Use timeout to kill after guest is compiled (prove step is unnecessary).
    # The binary will compile the guest, then we kill it during preprocessing/proving.
    echo "=== Running native host to compile guest ELF ==="
    timeout 120 cargo run -p "${NATIVE_CRATE}" --release -q -- "1" 2>/dev/null || true

    # Find the freshly built guest ELF
    local GUEST_CRATE
    GUEST_CRATE=$(grep -oP 'package\s*=\s*"\K[^"]+' "./examples/${NATIVE_CRATE}/Cargo.toml" | head -1)
    if [ -z "${GUEST_CRATE}" ]; then
        # Fallback: derive from native crate name
        GUEST_CRATE="${NATIVE_CRATE}-guest"
    fi
    local GUEST_BIN="${GUEST_CRATE}"

    local FRESH_ELF
    FRESH_ELF=$(find "${GUEST_TARGET_DIR}" -name "${GUEST_BIN}" -type f -path "*/release/*" -newer "./examples/${NATIVE_CRATE}/Cargo.toml" 2>/dev/null | head -1)

    if [ -z "${FRESH_ELF}" ]; then
        echo "ERROR: Could not find freshly built guest ELF for ${GUEST_BIN}"
        echo "Try running: cargo run -p ${NATIVE_CRATE} --release -- <args>"
        exit 1
    fi

    cp "${FRESH_ELF}" "${GUEST_ELF}"
    echo "=== Copied fresh guest.elf ($(wc -c < "${GUEST_ELF}") bytes) ==="
}

build_one() {
    local CRATE="$1"
    local OUTDIR="${2:-./examples/${CRATE}/pkg}"
    if [ -z "${OUTDIR}" ]; then
        OUTDIR="./examples/${CRATE}/pkg"
    fi
    # Derive the wasm filename from crate name (hyphens -> underscores)
    local WASM_NAME="${CRATE//-/_}"

    ensure_guest_elf "${CRATE}"

    echo "=== Building ${CRATE} for wasm32 ==="
    cargo +nightly build \
        -p "${CRATE}" \
        --target wasm32-unknown-unknown \
        --profile wasm \
        -Z build-std=std,panic_abort

    local WASM_PATH="./target/wasm32-unknown-unknown/wasm/${WASM_NAME}.wasm"

    echo "=== Running wasm-bindgen ==="
    rm -rf "${OUTDIR}"
    mkdir -p "${OUTDIR}"
    wasm-bindgen \
        --target web \
        --out-dir "${OUTDIR}" \
        "${WASM_PATH}"

    echo "=== Running wasm-opt ==="
    wasm-opt -O3 \
        --converge \
        --enable-simd \
        --enable-threads \
        --enable-bulk-memory \
        --enable-nontrapping-float-to-int \
        --enable-sign-ext \
        --enable-mutable-globals \
        --strip-debug \
        "${OUTDIR}/${WASM_NAME}_bg.wasm" \
        -o "${OUTDIR}/${WASM_NAME}_bg.wasm"

    # Fix wasm-bindgen-rayon worker import path
    local WORKER_JS
    WORKER_JS=$(find "${OUTDIR}/snippets" -name "workerHelpers.js" 2>/dev/null || true)
    if [ -n "${WORKER_JS}" ]; then
        echo "=== Fixing worker import path ==="
        sed -i "s|await import('../../..')|await import('../../../${WASM_NAME}.js')|" "${WORKER_JS}"
    fi

    echo "=== Done: ${CRATE} ==="
    ls -lh "${OUTDIR}/${WASM_NAME}_bg.wasm"
}

if [ $# -ge 1 ]; then
    build_one "$1" "${2:-}"
else
    echo "No crate specified — building all WASM examples"
    for dir in ./examples/*-wasm/; do
        crate="$(basename "${dir}")"
        build_one "${crate}"
    done
fi
