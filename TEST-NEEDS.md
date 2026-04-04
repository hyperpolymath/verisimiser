# TEST-NEEDS.md — verisimiser

## CRG Grade: C — ACHIEVED 2026-04-04

## Current Test State

| Category | Count | Notes |
|----------|-------|-------|
| Integration tests (Rust) | 2 | Dual compiled binaries (debug + release) |
| Verification tests | Unit-level | `verification/tests/` directory present |
| FFI tests | Present | `src/interface/ffi/test/` |

## What's Covered

- [x] Dual integration test builds
- [x] FFI verification layer
- [x] Cargo test harness

## Still Missing (for CRG B+)

- [ ] Property-based verification testing
- [ ] Fuzzing for edge cases
- [ ] Benchmarking suite
- [ ] Multi-target platform tests

## Run Tests

```bash
cd /var/mnt/eclipse/repos/verisimiser && cargo test
```
