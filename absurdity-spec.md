# Absurdity Protocol — Intueina Deployment Spec

> Applying the 11-domain equilibrium framework to fix Intueina's 4 critical wounds and harden the cognitive memory server.

---

## Domain -1: Parsimony Gate

| Criterion | Target | Verification |
|-----------|--------|--------------|
| Test coverage per module | >80% | `pytest --cov=intueina` |
| IRR improvement per fix | >5% | Compare info retention across boot cycles |
| Latency overhead per fix | ≤20% | `time` benchmark on recall/memorize |
| Removability | Each fix toggleable via config flag | Config field per domain |

---

## Domain 0: Nature — SimultaneousConstraint

All 4 fixes + 3 enhancements must satisfy ALL forces simultaneously before entering codebase:

| Force | Test |
|-------|------|
| No breaking changes to MCP tool interface | Existing tools unchanged |
| No new dependencies | stdlib + current deps only |
| Backward-compatible config | Old configs parse without error |
| No data loss on upgrade | Existing DBs remain readable |

---

## Domain 1: Biology — Mycelial Cortex Distribution

**Wound:** Recall is single-threaded — cortex queries one node at a time.

**Fix:** Distribute `Cortex.recall()` across specialty nodes in parallel.

| Step | Artifact |
|------|----------|
| **File** | `intueina/cortex.py` |
| **Class** | `MycelialRecall` (new) |
| **Interface** | `MycelialRecall.distribute(query: str, limit: int) -> list[dict]` |
| **Behavioral change** | Fan out recall to `MAX_WORKERS=4` parallel token matchers. Merge results by score. |
| **Call site** | `Cortex.recall()` at `cortex.py:71` — replaces serial loop |
| **Tests** | 1. Parallel recall returns same top-5 as serial. 2. Graceful degradation when worker thread fails. |

---

## Domain 2: Physics — Maxwell Proxy Dispatch

**Wound (Critical):** `_handle_proxy_chat` triple-dispatches through ChatFlow → Agent → Engine → back to Agent with inconsistent signature propagation. The `respond` call creates an information funnel that drops parameters at each hop.

**Root cause:** `server.py:472` — `_handle_proxy_chat` calls `context_injector.inject()` which calls `provider_router.resolve()` which calls `proxy.py` chain, but context enrichment parameters are not propagated back.

**Fix:** Replace triple dispatch with MaxwellCompiler-sorted direct dispatch.

| Step | Artifact |
|------|----------|
| **File** | `intueina/server.py` |
| **Class** | `MaxwellProxyDispatcher` (new) |
| **Interface** | `dispatch(body: dict) -> dict` |
| **Behavioral change** | Instead of `_handle_proxy_chat → context_injector.inject → provider_router.resolve`, directly sort incoming messages into `[CONTEXT]`, `[MEMORY]`, `[QUERY]` dimensions and dispatch to the resolved provider in a single call. |
| **Call site** | `_handle_proxy_chat` at `server.py:472` |
| **Tests** | 1. Message with explicit context → context preserved. 2. Message without context → no context injected. 3. All original parameters propagate to provider. |

---

## Domain 3: Chemistry — Ribosome Pagination

**Wound (Critical):** `Scheduler.get_due()` at `scheduler.py:30` reads entire `reviews` table. `Observer.correct()` at `observer.py:188` iterates all `_query_history` items. `Observer.verify_chain()` at `observer.py:244` iterates entire chain.

**Root cause:** No LIMIT clauses on production queries.

**Fix:** Replace unbounded scans with paginated, limited queries.

| Step | Artifact |
|------|----------|
| **File** | `intueina/scheduler.py`, `intueina/observer.py` |
| **Class** | (methods only) |
| **Interface** | `get_due(limit=10, offset=0)` — existing signature, add `offset` param |
| **Behavioral change** | Every SQL SELECT gets a LIMIT clause. `correct()` only processes recent entries. `verify_chain()` only checks last N blocks. |
| **Call site** | `scheduler.py:30`, `observer.py:188`, `observer.py:244` |
| **Tests** | 1. `get_due` with 1000 reviews returns only `limit` results. 2. `correct()` processes ≤100 entries. 3. `verify_chain()` checks ≤1000 blocks. |

---

## Domain 4: Biomechanics — Creative Joints in Server Pipeline

**Wound:** Pipeline stages (recall → detect → correct → respond) are rigid levers — no graceful degradation on failure.

**Fix:** Wrap each stage transition in a `CreativeJoint` that flexes under tension.

| Step | Artifact |
|------|----------|
| **File** | `intueina/server.py` |
| **Class** | `PipelineJoint` (new) |
| **Interface** | `transmit(signal: dict, stage: str) -> dict` |
| **Behavioral change** | When `recall` errors → returns degraded results (empty list) instead of crashing. When `detect` times out → skips. When `correct` fails → logs and continues. |
| **Call site** | Every tool handler in `server.py:_handle_*` |
| **Tests** | 1. Recall storage error → returns empty results. 2. Observer failure → continues without observations. |

---

## Domain 5: Philosophy — Death of the Director (Context Injector)

**Wound:** `ContextInjector` produces a single interpretation of the message context.

**Fix:** Generate 3 alternative context interpretations; let the provider choose.

| Step | Artifact |
|------|----------|
| **File** | `intueina/proxy.py` |
| **Class** | `InterpretationSpace` (new) |
| **Interface** | `InterpretationSpace.generate(messages: list) -> list[list]` |
| **Behavioral change** | Instead of one enriched message set, produce 3: conservative, balanced, aggressive context injection. The provider model picks which to use (via first-choice heuristic). |
| **Call site** | `ContextInjector.inject()` at `proxy.py` (current) |
| **Tests** | 1. Returns exactly 3 interpretations. 2. All preserve original messages. 3. Conservative has minimal injection. |

---

## Domain 6: Psychology — WorldSchema Persistence

**Wound:** Memories "vanish" between boot cycles — no explicit persistence schema.

**Fix:** `WorldSchema` in `storage.py` tracks every entity with `continues_to_exist` flag and generates `[PERSISTS]` clauses on boot.

| Step | Artifact |
|------|----------|
| **File** | `intueina/storage.py` |
| **Class** | `WorldSchema` (new) |
| **Interface** | `WorldSchema.persist(boot_timestamp: float) -> str` |
| **Behavioral change** | On `boot()`, inject `[PERSISTS: ...]` clauses for all entities that existed in previous session. Prevents agent from assuming memories were created fresh. |
| **Call site** | `IntueinaServer._handle_boot()` at `server.py:400` |
| **Tests** | 1. 3 entities from prior session → all persisted. 2. No prior session → empty persist clause. |

---

## Domain 7: Medicine — ContinuityHomeostat (Proactive Correction)

**Wound (Critical):** `Observer.correct()` at `observer.py:185-197` is passive — it only runs when manually called via `correct` tool, never triggers when users actually make mistakes during recall/memorize.

**Root cause:** `_apply_correction()` at `observer.py:199` only logs corrections with no active feedback. No set points for memory integrity.

**Fix:** Replace passive logging with `ContinuityHomeostat` that proactively measures drift and corrects on every `ingest()`.

| Step | Artifact |
|------|----------|
| **File** | `intueina/observer.py` |
| **Class** | `ContinuityHomeostat` (new) |
| **Interface** | `measure_and_correct(observation: Observation) -> dict` — injects `[CORRECTION]` into memory state |
| **Behavioral change** | On every `ingest()`, if drift confidence > 0.5, proactively attach `[CORRECTION]` to the observation detail. Replace passive `_apply_correction` (log-only) with `measure_and_correct` (stateful fix + log). |
| **Call site** | `Observer.ingest()` at `observer.py:123` |
| **Tests** | 1. Ingest with high-confidence drift → `[CORRECTION]` in observation. 2. Ingest with low confidence → no correction. 3. Multiple corrections → drift state updated. |

---

## Domain 8: Economics — VCG Model Auction

**Wound:** `ProviderRouter.resolve()` at `proxy.py` uses hard-coded model priority.

**Fix:** Add `TaskAuction` that models bid on proxy requests.

| Step | Artifact |
|------|----------|
| **File** | `intueina/proxy.py` |
| **Class** | `TaskAuction` (new) |
| **Interface** | `allocate(task_type: str, available: list[str]) -> tuple[str, float]` |
| **Behavioral change** | Replace priority-based model selection with VCG auction. Providers bid (speed × reliability / cost). Winner pays second price. |
| **Call site** | `ProviderRouter.resolve()` at `proxy.py` |
| **Tests** | 1. Phi-4-mini wins on fast_analysis tasks. 2. Gemma-12B wins on reasoning tasks. 3. Loading penalty correctly increases bid. |

---

## Domain 9: Cryptography — MerkleContinuity

**Wound (Critical):** `Observer` at `observer.py:73-78` has a chain of blocks linked by `previous_hash`, but no global Merkle root. `_last_hash()` at `observer.py:303` returns the last block's hash but there's no `root_hash()` method. No `provenance()` tracking per field.

**Root cause:** The chain is just an ordered list — without a tree structure the entire chain is the only proof of integrity. No single hash can verify arbitrary subsets.

**Fix:** Add Merkle root computation and provenance tracking.

| Step | Artifact |
|------|----------|
| **File** | `intueina/observer.py` |
| **Class** | `MerkleContinuity` (new mixin to Observer) |
| **Interface** | `root_hash() -> str`, `provenance(field: str) -> list[tuple[int, Any]]` |
| **Behavioral change** | Add `root_hash()` that builds a Merkle tree from all block hashes and returns the root. Add `provenance()` that tracks when specific fields (query, source, confidence) changed across blocks. |
| **Call site** | `Observer.summary()` at `observer.py:252` — root_hash in output. `Observer.ingest()` at `observer.py:123` — provenance updated. |
| **Tests** | 1. Empty chain → root_hash = "EMPTY". 2. 3 blocks → deterministic hex root. 3. provenance("source") returns change points. 4. Chain tamper → root_hash changes. |

---

## Domain 10: Metallurgy — SubstrateConstraint

**Wound:** No substrate-level resource budgeting. `ResourceBudget` at `resources.py` is in-memory only, no persistence.

**Fix:** Add substrate constraints: max queries/sec, max memory size, max chain length. Persist to SQLite.

| Step | Artifact |
|------|----------|
| **File** | `intueina/resources.py` |
| **Class** | `SubstrateConstraint` (new) |
| **Interface** | `check(resource: str) -> bool`, `budget_remaining() -> dict` |
| **Behavioral change** | On each tool call, check substrate budget. If exceeded, return rate_limit error. Budget persisted in SQLite. |
| **Call site** | Every `_handle_*` method in `server.py` — called after `_check_budget()` |
| **Tests** | 1. Within budget → returns True. 2. Exceeded → returns False. 3. Persisted across restarts. |

---

## Deployment Order

| Priority | Domain | Wound | Effort | Impact |
|----------|--------|-------|--------|--------|
| P0 | D2 Physics | Triple dispatch | Low | Critical |
| P0 | D3 Chemistry | Full table scan | Low | Critical |
| P0 | D7 Medicine | Passive corrections | Medium | Critical |
| P0 | D9 Cryptography | No global root | Medium | Critical |
| P1 | D6 Psychology | No persistence | Low | High |
| P1 | D1 Biology | Single-threaded recall | Medium | High |
| P1 | D4 Biomechanics | Rigid pipeline | Low | Medium |
| P1 | D8 Economics | Hard-coded priority | Medium | Medium |
| P2 | D10 Metallurgy | No substrate budget | Low | Medium |
| P2 | D5 Philosophy | Single interpretation | Low | Low |

---

## Performance Budget

| Component | Budget (ms) | IRR Impact |
|-----------|-------------|------------|
| MaxwellProxyDispatcher | 2 | Eliminates parameter loss |
| Ribosome pagination | 1 | Prevents OOM |
| ContinuityHomeostat | 15 | Active drift correction |
| MerkleContinuity | 5 | Verifiable root |
| MycelialRecall | 10 | 4x parallel speedup |
| WorldSchema persist | 2 | Cross-boot persistence |
| **Total overhead** | **35** | **IRR target: ≥0.95** |

---

## Verification

```
pytest tests/ -v --tb=short  # All existing + new tests pass
python -m intueina serve --transport stdio  # Server boots
# Verify all MCP tools respond
echo '{"method":"tools/list","id":1}' | python -m intueina serve --transport stdio
```
