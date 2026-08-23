# Keymono Module Spec — Local Arterial Layer

Keymono is the Rust-powered local layer between AI agents and LLM providers.
Built one module at a time. Each module tested independently, pushed to production,
then used as context for the next module.

## Module Boundaries

| Layer | Owns | Does NOT own |
|---|---|---|
| **Keymono** | API keys, routing, context injection, cortex recall, FFI bridge | Memory storage schema, observer, Brainhaul, macOS UI |
| **Intueina** | Cortex schema, observer, profiles, marketplace, macOS app | API keys, credential storage, LLM routing |

## Module Queue (build order)

### 1. Credential Store (BUILT — keychain_manager.swift, keymono_view.swift)
- macOS keychain CRUD
- Provider detection by key format
- Multiple keys per provider
- Status: deployed in Intueina macOS app

### 2. Provider Router (BUILT — proxy.py, server.rs)
- Model → provider mapping
- Round-robin key rotation
- TaskAuction provider selection
- Status: deployed in Python proxy, ported to Rust

### 3. Cortex Engine (BUILT — cortex.py, cortex.rs)
- Word-graph memory store
- Bond traversal recall
- Fire/decay/gyrate homeostasis
- Bond creation gate (threshold=3)
- Nuke cycle (safe-set preservation)
- Fast health stats (no AVG/merkle)
- Status: Python 39/39 tests, Rust 18/18 tests. SQLite-backed.

### 4. Profile Router (BUILT — router.py, router.rs)
- 30-minute profile buckets (48 profiles/day)
- Multi-dimensional keying (time, IP region, device, session depth)
- Grounding Layer 0 (static truth extraction)
- Plausibility detection (crisis scenarios)
- Profile branching (parent → child evolution)
- Status: Python + Rust tests passing

### 5. Feedback Loop (BUILT — feedback.rs, proxy.py)
- Signal detection (used, ignored, corrected)
- Per-request tracking
- Stats accumulation
- Status: running in Rust server

### 6. Context Injector (BUILT)
- Router lookup → cortex recall → tag injection
- Enriches messages with [CONTEXT FROM MEMORY] block
- Status: running in Rust server on POST /v1/chat/completions

### 7. HTTP Proxy Server (BUILT — server.rs)
- GET /health (instant, fast_stats)
- POST /v1/chat/completions (inject → forward → track)
- CortexActor channel-based architecture
- Status: running on 9124, 18/18 tests

### 8. Auto-Learner (PARTIAL — proxy.py)
- Decision-only storage (≤200 chars)
- Type classification: decision/constraint/open/discovery
- NOT wired to feedback loop
- Python only, not ported to Rust

### 9. Nuke Cycle Scheduler (SPEC ONLY)
- 30-day feedback window before nuke
- Runs bond cleanup on schedule (daily)
- Safe-set preservation from feedback signals
- Requires: Feedback loop data (module 5)

### 10. Profile Evolution Engine (SPEC ONLY)
- Profile branches on consistent feedback signals
- Parent → child evolution with delta overrides
- Cold profile re-entry (30-day idle → fresh child)
- Requires: Feedback loop data (module 5)

### 11. Grounding Layer 0 — Rust (NOT BUILT)
- Static truth extraction in Rust (currently Python only)
- Domain classification
- Urgency detection
- Plausibility gate
- Requires: regex patterns ported from router.py → router.rs

### 12. FFI Bridge (SPEC ONLY)
- Rust compiled as `.dylib`
- Loaded by opencode plugin via ffi-napi
- Zero HTTP overhead — function calls in-process
- Exposes: inject(messages) → enriched_messages
- Exposes: get_stats() → health data
- Replaces HTTP proxy model

### 13. macOS Keychain Bridge — Rust (NOT BUILT)
- Native macOS keychain access from Rust
- Replaces Python subprocess calls to `security` CLI
- Reads opencode auth.json
- Env var fallback
- Requires: security framework FFI

## Production Status

| Module | Built | Tested | Deployed |
|---|---|---|---|
| Credential Store | ✅ | ✅ | ✅ |
| Provider Router | ✅ | ✅ | ✅ |
| Cortex Engine | ✅ | ✅ | ✅ |
| Profile Router | ✅ | ✅ | ✅ |
| Feedback Loop | ✅ | ✅ | ✅ |
| Context Injector | ✅ | ✅ | ✅ |
| HTTP Proxy Server | ✅ | ✅ | ✅ |
| Auto-Learner | ✅ (Python) | ⬜ | ⬜ |
| Nuke Cycle Scheduler | ⬜ | ⬜ | ⬜ |
| Profile Evolution | ⬜ | ⬜ | ⬜ |
| Grounding Layer 0 (Rust) | ⬜ | ⬜ | ⬜ |
| FFI Bridge | ⬜ | ⬜ | ⬜ |
| Keychain Bridge (Rust) | ⬜ | ⬜ | ⬜ |

## Integration Contract

Each module exposes a single interface. Modules don't import each other directly — they compose via the integration layer (server.rs currently, FFI bridge eventually).

```rust
// Contract
trait KeymonoModule {
    fn op(&mut self, cmd: &str, params: &OpParams) -> OpResult;
}
```

All state mutations return merkle root. All modules implement living absurdity homeostasis.
