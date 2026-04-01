Some corrections to Jolt benchmarks to fill the gaps which I overlooked the first time around:

| # | Change | Detail |
|---|---|---|
| 1 | Lazy query only | Only json-query is benchmarked — zkTLS apps need lazy path queries, not full parsing. Uses a patched gjson (no no\_std lazy JSON crate exists off the shelf; upstream Jolt std guest support is WIP). |
| 2 | Blake commitment cost | Bridging MPC-TLS data to Jolt requires committing in MPC and opening in Jolt, whereas VOLE-zkVM operates directly on MPC-TLS authenticated data. |
| 3 | Upstream rebase | Rebased on latest Jolt for recent perf improvements. |

| Benchmark | Input | Native | Browser |
|---|---|---|---|
| Integer check (x > 700) | "701" | 1.5s | 4.2s |
| JSON query | 1 KB | 4.7s | 31.0s |
| JSON query | 2 KB | 6.7s | 43.9s |
| JSON query | 4 KB | 9.9s | 60.6s |
