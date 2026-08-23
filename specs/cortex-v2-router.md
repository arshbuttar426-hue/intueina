# Cortex v2 — Router/Cortex Separation + Profile-Gated Neural Chains

| Field | Value |
|---|---|
| Project | Intueina (cortex subsystem) |
| Problem | Current cortex is a flat word-graph tag injector. 183 nodes generate 8.5M bonds (637MB, quadratic growth). Recall quality is word-match deep, not meaning-deep. No feedback loop exists — bonds accumulate uniformly with no notion of which ones ever helped produce a good response. |
| Solution | Separate Router (deterministic gating) from Cortex (meaning-finding). Router uses 1,440 pre-built static profiles (one per minute, multi-dimensional keyed) to gate cortex activation. Feedback loop tracks which injected context survived into the LLM response — used to evolve profiles and nuke dead bonds. Cortex remains a meaning-finder; router decides density, tag composition, and domain weighting per invocation. |
| Test Cases | See §Test Plan |

## Architecture

```
User Query
    │
    ▼
Router — deterministic profile lookup (no runtime AI)
    │
    ├─ 1. Resolve profile for current minute
    │       Key: time + IP region + device + session depth
    │       Returns: {composition, density, domain_weights, grounding_anchor}
    │
    ├─ 2. Grounding (Layer 0)
    │       "What is undeniably true about this query?"
    │       Static facts: time, device, user, session context
    │       Meta-grounding: is the scenario itself plausible?
    │
    ├─ 3. Dissection (Layer 1)
    │       Domain classification, urgency, constraints, knowledge gap
    │
    ├─ 4. Gating
    │       Load profile → {tag_composition, density, bias_weights}
    │       Pass instruction + query to cortex
    │
    ▼
Cortex — meaning-finder (unchanged core)
    │
    ├─ Receives: gated query + profile instruction
    ├─ Executes: keyword-tokenize → bond traversal → recall → format_tags
    ├─ Returns: persona-formatted context line
    │
    ▼
ContextInjector — prepends [CONTEXT FROM MEMORY] to messages
    │
    ▼
LLM (DeepSeek via proxy)
    │
    ▼
Feedback Loop — AutoLearner (rewritten)
    │
    ├─ Did injected context appear in LLM response? (true/false)
    ├─ Did user accept response? (no correction = positive signal)
    ├─ Signal → strengthens profile branch OR branches child profile
    ├─ Silent bonds → flagged for nuke on cleanup cycle
    └─ Stores: decisions only, not full transcripts
```

## Router Module (new)

### Profile Structure

```python
Profile = {
    "id": "00:11-IST-Monday-home-macbookair",
    "time_minute": 11,          # minute of day (0-1439)
    "timezone": "IST",
    "day": "Monday",
    "ip_region": "home",
    "device": "macbookair",
    "session_depth": "deep",    # shallow(<15min) / moderate / deep(>60min)
    "composition": {
        "query_weight": 0.7,    # how much specific terms matter vs domain
        "think_in": ["architecture", "systems-design", "memory-engineering"],
        "output_needed": ["pattern-recognition", "design-decision"],
        "density": "minimal",   # minimal / balanced / dense
        "recall_bias": "recent", # recent-session / balanced / full-archive
    },
    "grounding_anchor": "building Intueina exocortex",
    "parent_profile": None,     # None = root profile
}
```

### Multi-Dimensional Profile Key

Profiles are keyed by `(minute, timezone, day_type, ip_region, device, session_depth)` — not just clock time. A single minute slot can have multiple profiles for different contexts (home vs mobile, deep work vs quick check).

### Profile Evolution (Branching)

Profiles never mutate. They branch:

```
parent: {day:Monday, depth:deep}
   └ child: {day:Monday, depth:deep, domain_override:ops}
         └ grandchild: {day:Monday, depth:deep, domain_override:ops, density:minimal}
```

When feedback says a profile consistently produced wrong context, a child profile is created with a delta override. The router walks the tree: newest matching branch wins. Parents remain for traceability.

### Maximum Pool

1,440 base minutes × ~3 contexts (home/work/mobile) × ~2 depths (shallow/deep) = ~8,640 profiles maximum. Upper bound. Real-world: far fewer until branching creates more.

## Cortex Module (enhanced, mostly unchanged)

### What stays the same

- Tokenize → filter stopwords → bond traversal → recall → `format_tags()`
- SQLite-backed word graph
- Bond fire/decay/gyrate homeostasis mechanics
- Word co-occurrence bond table
- Merkle root for integrity

### What changes

| Change | Detail |
|---|---|
| Bond creation gate | Only create bonds when co-occurrence count ≥ N (TBD by feedback data). Cut quadratic explosion by rejecting one-off pairings. |
| Recall limit | Accept `density` and `composition` params from router. `minimal` = 3 nodes, `balanced` = 5, `dense` = 10. |
| Tag formatting | Accept `think_in` and `output_needed` overrides from router. If router says `think_in:[ops]` but metadata says `engineering`, use router. |
| Cleanup cycle | `nuke()` method: remove bonds with strength < 0.10 AND last_fired older than 7 days AND never appeared in a positive-feedback recall. |

### Bond Cleanup Criteria (Deferred)

Cannot run cleanup until feedback loop produces data. After 30 days of feedback:

1. Mark every bond that contributed to a recall with positive feedback
2. Mark every bond that appeared in a recall where LLM ignored context
3. Nuke unmarked bonds below strength threshold
4. Expected reduction: 85-95% of current bonds (from 8.5M to ~500K-1M)

## Feedback Loop (new)

### Signal Types

| Signal | Detection | Effect |
|---|---|---|
| Context used | Injected tags appeared in LLM response terms | +strength on bond chain, +score on profile branch |
| Context ignored | LLM response has no overlap with injected tags | -score on profile branch, bonds flagged for review |
| User corrected | User's next message contradicts or re-asks | -strength on entire injected chain |
| User accepted | No correction, conversation continues naturally | Default positive for active profile |

### Decision-Only AutoLearner (rewrite)

Replace current AutoLearner which stores full Q&A transcript (500+ chars).

New format:
```
[decision] cortex_architecture: router+cortex split, 1440 static profiles, profile=deterministic, feedback=context-survival
[constraint] bond_explosion: 637MB at 183 nodes, quadratic growth
[open] profile_branching vs profile_override?
```

Max 200 chars per entry. Structured. Searchable by domain tag. Never stores conversational tone or full responses.

## Grounding Layer (new)

### Layer 0 — Static Truth

Per query, extract without AI:

1. **Time truth** — what hour, what day, what session length so far
2. **Device truth** — IP region, device type, uptime
3. **Session truth** — accumulated topic, last decision pending
4. **Plausibility truth** — does the scenario described match the user's actual state? (parachute gate)

Grounding failures → no cortex activation. "I'm falling from a plane help" at a typing desk → query is hypothetical, inject differently or not at all.

### Layer 1 — Dissection

Given grounded facts, classify:

- What domain(s) does this query touch?
- What's the urgency?
- What constraints are in play (resources, prior decisions, dependencies)?
- What does the user already know? (check cortex with depth=0)
- What's the actual knowledge gap?

Dissection output drives profile selection and density.

## Immediate Actions (Ordered)

1. **Rewrite AutoLearner** — store decisions, not transcripts. Stop recursive bloat.
2. **Add feedback signal tracking** — context_used, context_ignored, user_corrected, user_accepted. Store against bond chains. No nuke yet, just collect data.
3. **Tighten bond creation** — add co-occurrence threshold. Don't create bonds on first meeting of two words. Wait for evidence.
4. **Build Router prototype** — 1 profile static, no branching. One config: `{composition, density, domain_weights}`. Prove the separation works.
5. **Build profile branching** — parent → child evolution based on feedback data.
6. **Build grounding layer** — Layer 0 (static truth) first, Layer 1 (dissection) after.
7. **Nuke cycle** — run when 30 days of feedback data exists. Prune dead bonds.

## Falsification Conditions

The design is falsified if:

1. Router profile lookup adds >10ms to any request
2. Router + cortex combined produces worse recall quality than cortex alone (A/B against baseline)
3. Profile branching creates >50 child profiles for any single parent (runaway evolution)
4. Nuke cycle removes bonds that would have contributed to a positive match within the next 7 days
5. Grounding Layer 0 produces false positives on plausibility (rejects "what if" queries as real scenarios)
6. Feedback loop fails to distinguish between "context helped" and "LLM is good at faking it"
7. Bond explosion continues at quadratic rate after threshold gate is applied

## Interesting Ideas Discovered (Looping Session)

### 1. Profile Anomaly Detection
If the profile at 03:00 is typically "deep architecture" and the user suddenly asks about dinner recipes, grounding detects the anomaly. The profile doesn't switch — but the density drops to minimal. System recognizes it doesn't belong in this session, injects nothing.

### 2. Idle vs Active Profiles
Profiles not used for 30+ days become "cold." On re-entry, the system boots a fresh child profile instead of reactivating the stale parent. Prevents "you used to do X at this hour" from anchoring wrong.

### 3. Competitive Forgetting as Feature
The nuke isn't just cleanup — it's a forgetting mechanism that actively improves recall. Real memory discards information that contradicts new learning. The nuke cycle should do the same: if a bond chain was correct in the past but feedback now says it's wrong, nuke it harder than the silent bonds.

### 4. Decision Archaeology via Profile Tree
The profile branching tree IS an audit trail. You can trace every router decision back to its origin: "why did the system inject ops-domain context at 09:47?" Because a branch was created when user corrected a deployment-related mis-injection on March 3rd. Visible, traceable, debuggable.

### 5. Grounding as Intrusion Detection
If the IP region suddenly shifts from Punjab to Lagos at 00:11 — grounding rejects the profile change and flags the session. Not a security feature per se, but the deterministic truth layer catches impossible state transitions automatically.

### 6. Bond Archeology vs Nuke
Before nuking, export the bond graph as a snapshot. Bond graphs at different points in time ARE the evolution of your thinking. "This cluster of bonds formed in January when I was deep in cinema-engine; this cluster formed in March during the security audit." The graph is a fossil record even when nuked. Keep historical snapshots.

## Test Plan

| # | Test | Expected |
|---|---|---|
| 1 | Router lookup at minute 00:11 with known profile | Profile returned in <5ms, composition correct |
| 2 | Router lookup with no matching profile | Falls back to nearest parent profile |
| 3 | Cortex recall with router-reported density=minimal | Returns ≤3 nodes, max 200 chars per snippet |
| 4 | Cortex recall with router-reported density=dense | Returns ≤10 nodes, max 800 chars per snippet |
| 5 | Feedback loop: context_used → bond strength increase | Bonds in active chain strengthen by ≥FIRE_INCREMENT |
| 6 | Feedback loop: context_ignored → profile score decrease | Profile branch score drops; after 3 ignores, child profile created |
| 7 | Grounding Layer 0: query="I'm skydiving, chute failed" at desk | Grounding rejects literal interpretation, tags query as hypothetical |
| 8 | Grounding Layer 0: query="deploy to production tonight" at 22:00 | Grounding accepts, urgency=high, domain=deployment |
| 9 | AutoLearner: store decision, not transcript | New memory ≤200 chars, structured format, no conversational filler |
| 10 | AutoLearner: blob of 500 char conversation → decision extraction | Reduces to one-line decision, drops tone/pleasantries |
| 11 | Bond creation gate: words paired once, co-occurrence=1 | Bond not created (below threshold) |
| 12 | Bond creation gate: words paired 5+ times across different nodes | Bond created with initial strength 0.30 |
| 13 | Nuke cycle: bond with strength <0.10, last_fired >7d, zero positive feedback | Bond deleted |
| 14 | Nuke cycle: bond with strength <0.10 but marked positive feedback | Bond preserved |
| 15 | Profile branching: parent used 10 times, 3 ignores | Child profile created with delta override |
| 16 | Profile branching: 50+ children from one parent | Falsification condition triggered — branching is runaway |
| 17 | DB file size after nuke + bond creation gate | Target: <50MB at 183 nodes (from 637MB baseline) |
| 18 | A/B test: router+cortex vs cortex alone on 20 test queries | Router version produces higher relevance score in ≥14/20 |

## Files Changed

| File | Change |
|---|---|
| `cli/intueina/router.py` | **NEW** — Profile lookup, grounding, dissection, gating |
| `cli/intueina/cortex.py` | Accept `composition`, `density`, `domain_weights` params. Add `nuke()` method. Add bond creation threshold constant. |
| `cli/intueina/proxy.py` | `ContextInjector.inject()` calls router before cortex. Pass router output to cortex. Updated `AutoLearner` to store decisions only. Add feedback signal tracking. |
| `cli/intueina/storage.py` | Profile storage table. Feedback signal storage. |
| `cli/intueina/server.py` | Wire router into server constructor. Pass to ContextInjector. |
