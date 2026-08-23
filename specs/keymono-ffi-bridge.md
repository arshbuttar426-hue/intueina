# Keymono FFI Bridge — Module 12 Spec

napi-rs native addon. In-process context injection for Node.js agents.
Zero HTTP overhead. Single `.node` file. Load via `require('./keymono.node')`.

## Exported Functions

### `inject(messages: Array<Message>) → Array<Message>`
Accepts OpenAI-format messages array. Runs full injection pipeline:
1. Extract last user message
2. Triviality check
3. Router lookup (profile, grounding, suppress?)
4. Cortex recall via CortexActor channel
5. Format [CONTEXT FROM MEMORY] tag
6. Return enriched messages array

### `health() → Object`
Returns cortex stats (fast_stats), profile router stats, feedback stats, uptime.
Same data as GET /health HTTP endpoint.

### `stats() → Object`
Detailed stats including node counts, bond counts, merkle root.
Same as cortex.stats() but via channel.

## Architecture

```
Node.js (opencode plugin)
    │
    │  require('./keymono.node')
    │  const { inject } = require('keymono-ffi');
    ▼
FFI Boundary (napi-rs)
    │
    │  fn inject(env, messages: Array) → Array
    ▼
CortexActor (dedicated thread, spawned once on module init)
    │
    │  mpsc channel → Recall, Stats, Health
    ▼
Cortex (rusqlite + cortex.db)
Router (profile tree)
FeedbackTracker (signal store)
```

## Files

| File | Purpose |
|---|---|
| `cli-rust/ffi/Cargo.toml` | napi-rs crate config |
| `cli-rust/ffi/build.rs` | napi-build setup |
| `cli-rust/ffi/src/lib.rs` | Exported functions + module init |
| `cli-rust/ffi/package.json` | npm package metadata |
| `cli-rust/ffi/index.js` | JS wrapper (optional) |
| `cli-rust/ffi/index.d.ts` | TypeScript types |

## Build & Test

```bash
cd cli-rust/ffi
npm install @napi-rs/cli
npx napi build --release
node -e "const k = require('./keymono.node'); console.log(k.health())"
```

## Integration with opencode plugin

Replace HTTP proxy with FFI call:

```typescript
// plugin.ts — before
const PROXY_URL = "http://localhost:9124/v1";
output.options = { ...output.options, baseURL: PROXY_URL };

// plugin.ts — after
const keymono = require('./keymono.node');
// inject context, return to opencode
// opencode handles the LLM call with enriched messages
```

## Falsification Conditions

1. napi build fails on macOS aarch64
2. inject() returns same message count as input (no enrichment)
3. health() returns error when cortex.db is locked
4. Memory leak: each inject() call leaks the CortexActor thread
5. Node.js crash on module unload (SIGABRT on cleanup)
