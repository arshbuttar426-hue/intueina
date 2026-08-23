# Keymono Bridge — Observer → ContextInjector Drift-Aware Density Control

| Field | Value |
|---|---|
| Orion Domains | D0 Nature (simultaneous), D1 Metallurgy, D2 Biology, D3 Physics, D4 Chemistry, D5 Biomechanics, D6 Philosophy, D7 Psychology, D8 Medicine, D9 Economics, D10 Cryptography, D-1 Parsimony |
| Project | Keymono (intueina proxy + credential subsystem) |
| Problem | `ContextInjector.inject()` always fetches 5 memories at 400 chars each. `Observer` computes drift, anomaly, and pattern shift but nothing consumes these signals. Keymono's intended design is **drift-aware dynamic injection density** — the bridge between observer output and injector parameters is missing. |
| Solution | Add `Observer.drift_summary()` that exposes aggregate drift confidence; add `ContextInjector._drift_density()` that maps confidence → `(count, snippet_length, boost_priority)`; wire in server constructor. |
| Test Cases | See §Test Plan |

## Architecture

```
User message
     │
     ▼
ContextInjector.inject(messages)
     │
     ├─ 1. Extract keywords from last user message
     ├─ 2. Query observer.drift_summary()  ←── NEW
     │       └─ returns {confidence: 0.0-1.0, kind: str, ...}
     ├─ 3. Compute density params ←── NEW
     │       count = 3 + floor(confidence × 7)        → 3–10
     │       snippet_length = 200 + floor(confidence × 600) → 200–800
     │       boost = confidence > 0.5
     ├─ 4. server._handle_recall({query, limit: count})
     ├─ 5. Format context with dynamic snippet_length, boost prefix
     └─ 6. Prepend [CONTEXT FROM MEMORY] block to messages

Observer (updated concurrently)
     │
     ├─ ingest() → updates internal drift state ←── NEW
     │       reads recent correction_log entries
     │       computes max confidence, drift kinds
     │       stores in self._drift_state dict
     └─ drift_summary() → returns snapshot ←── NEW
```

## Files Changed

### 1. `intueina/observer.py`

| Change | Detail |
|---|---|
| New attribute | `self._drift_state: dict` — `{confidence: float, kind: str, updated_at: float}` |
| New method | `_update_drift_state()` — iterates `_correction_log[-20:]`, sets max confidence and union of drift kinds |
| Hook | `ingest()` calls `_update_drift_state()` at end (after any persist) |
| New method | `drift_summary() -> dict` — returns copy of `_drift_state` |

**`drift_summary()` interface:**
```python
def drift_summary(self) -> dict:
    """Aggregate drift confidence across all observer channels.
    
    Returns:
        confidence: float (0.0 = no drift, 1.0 = max drift)
        kind: str ("none" if no drift, "|"-joined kinds otherwise)
        updated_at: float (unix timestamp of last update)
    """
```

**`_update_drift_state()` behavior:**
- Scans `self._correction_log[-20:]` (most recent 20 corrections)
- `confidence` = max of all entry confidences
- `kind` = `"|".join(sorted(set of all entry kinds))` or `"none"`
- `updated_at` = `time.time()`

### 2. `intueina/proxy.py`

| Change | Detail |
|---|---|
| `ContextInjector.__init__` | Add optional `observer: Observer | None = None` param. Store as `self._observer`. |
| New method | `_drift_density() -> dict` — queries observer for drift, returns `{count, snippet_length, boost}` |
| `inject()` | Uses `_drift_density()` params instead of hardcoded `limit=5` and `snippet_length=400` |
| `_format_context()` | Add `snippet_length: int = 400` and `boost: bool = False` params |

**`_drift_density()` mapping:**

| Drift Confidence | Count | Snippet Length | Boost |
|---|---|---|---|
| 0.0–0.14 | 3 | 200 | False |
| 0.15–0.28 | 4 | 286 | False |
| 0.29–0.42 | 5 | 372 | False |
| 0.43–0.57 | 6 | 457 | True (if >0.5) |
| 0.58–0.71 | 7 | 543 | True |
| 0.72–0.85 | 8 | 629 | True |
| 0.86–1.0 | 10 | 800 | True |

Formula: `count = 3 + floor(confidence * 7)`, `snippet_length = 200 + floor(confidence * 600)`.

Boost adds `[BOOST]` prefix to each memory line in formatted output.

### 3. `intueina/server.py`

| Change | Detail |
|---|---|
| `IntueinaServer.__init__` | Pass `observer=self.observer` to `ContextInjector(self, observer=...)` |

## Domain Verification (Absurdity Protocol — All 11 Domains Simultaneously)

### D0: Nature — Simultaneous Constraint

All forces below are satisfied by the same change. The bridge is not optional — it is the only path code can enter the artifact pool.

### D1: Metallurgy — Substrate Constraint

`_drift_density()` is O(20) scan of in-memory list. No I/O. No new SQL queries. No API calls. Context token budget bounded: max 10 × 800 = 8000 chars ≈ 2000 tokens.

### D2: Biology — Mycelial Distribution

Observer and ContextInjector remain independent nodes. Observer broadcasts drift state; injector reads it. Neither blocks on the other. No central coordinator.

### D3: Physics — Thermodynamics of Creativity

The bridge is a Maxwell's Demon: when entropy (drift) rises, it does more work (denser injection) to maintain ordered context. Density factor = thermodynamic work.

### D4: Chemistry — Catalytic Templates

`[CONTEXT FROM MEMORY]` template is unchanged. The observer→injector interaction doesn't consume or modify observer state. Catalyst is preserved.

### D5: Biomechanics — Joints Over Levers

`_drift_density()` is a joint between Observer and ContextInjector with 3 degrees of freedom (count, snippet_length, boost). Under high drift (tension), it flexes (denser injection). No rigid coupling.

### D6: Philosophy — Death of the Director

No single interpretation of drift is privileged. Pattern, anomaly, regression, and contradiction all feed the same density computation. The aggregate confidence determines behavior, not any one reading.

### D7: Psychology — Object Permanence

When drift is detected, previously-relevant memories become more important. Density increase ensures older context persists across requests. Objects continue to exist off-screen.

### D8: Medicine — Homeostatic Continuity Regulation

Set point: drift confidence = 0.0. Error signal: current drift confidence. Corrective action: denser context injection proportional to error. P-controller: `density = K × drift_confidence` where K=1.0.

### D9: Economics — Resource Markets

Context tokens are scarce. Allocation rule: spend proportionally to need. No drift → 3 × 200 = 600 chars. High drift → 10 × 800 = 8000 chars. Token allocation matches signal uncertainty.

### D10: Cryptography — Per-Memory Integrity

`drift_summary()` is deterministic given observer state. No randomness in critical path. Same observer state → same density params.

### D-1: Parsimony — Occam's Razor

Three changes: one new method on Observer (`drift_summary` + `_update_drift_state` + attribute), density computation in ContextInjector (one new method + modified `inject`/`_format_context` + constructor param), one wiring change in server.py. No new classes. No new files.

## Falsification Conditions

The bridge is falsified if:
1. `drift_summary()` returns confidence > 0 when `_correction_log` is empty
2. Density params exceed bounds (count > 10, snippet_length > 800, count < 3, snippet_length < 200)
3. Observer is None but `inject()` raises (should degrade gracefully to defaults)
4. Boost flag toggles on but produces zero behavioral difference in output (test with same input, probe for "[BOOST]" prefix)

## Test Plan

| # | Test | Expected |
|---|---|---|
| 1 | Observer with empty correction_log → `drift_summary()` | `confidence=0.0, kind="none"` |
| 2 | Observer with drift correction at confidence 0.8 → `drift_summary()` | `confidence=0.8, kind="drift"` |
| 3 | Observer with 3 corrections (drift=0.6, anomaly=0.9, regression=0.4) → `drift_summary()` | `confidence=0.9, kind="anomaly|drift|regression"` |
| 4 | `ContextInjector` with observer → `_drift_density()` at confidence 0.0 | `{count: 3, snippet_length: 200, boost: False}` |
| 5 | `ContextInjector` with observer → `_drift_density()` at confidence 0.7 | `{count: 7, snippet_length: 620, boost: True}` |
| 6 | `ContextInjector` with observer → `_drift_density()` at confidence 1.0 | `{count: 10, snippet_length: 800, boost: True}` |
| 7 | `ContextInjector` with observer=None → `_drift_density()` | Falls back to `{count: 5, snippet_length: 400, boost: False}` |
| 8 | `inject()` with observer=None → produces same output as pre-change | Output unchanged from baseline |
| 9 | `inject()` with observer, confidence=0.8, drift kind="drift" → context contains `[BOOST]` | First memory line prefixed with `[BOOST]` |
| 10 | `_format_context(snippet_length=100)` → each snippet truncated to ≤100 chars | No snippet exceeds 100 characters |
