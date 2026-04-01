#!/usr/bin/env bash
# Shared WASM build script for all Jolt WASM examples.
# Usage: ./wasm-build.sh [crate-name] [output-dir]
#   If no crate-name is given, builds all *-wasm examples.
# Example: ./wasm-build.sh integer-check-wasm ./examples/integer-check-wasm/pkg

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${REPO_ROOT}"

build_one() {
    local CRATE="$1"
    local OUTDIR="${2:-./examples/${CRATE}/pkg}"
    if [ -z "${OUTDIR}" ]; then
        OUTDIR="./examples/${CRATE}/pkg"
    fi
    # Derive the wasm filename from crate name (hyphens -> underscores)
    local WASM_NAME="${CRATE//-/_}"

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
