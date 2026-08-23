# Intueina Observer — Orion Domain Reinforcement

| Field                | Value                                                                                                                                                                                                                                           |
| -------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Orion Domains        | D8 Medicine, D10 Cryptography                                                                                                                                                                                                                   |
| File                 | intueina/observer.py                                                                                                                                                                                                                            |
| Function             | Observer                                                                                                                                                                                                                                        |
| Inputs               | db_path: str, storage: Storage, max_blocks: int                                                                                                                                                                                                 |
| Outputs              | Observations list, chain integrity report                                                                                                                                                                                                       |
| Test Cases           | 1. Auto-correct adds drift alert to observer history, 2. Per-memory hash verifies content integrity, 3. Chain repair detects and logs violation, 4. Self-heal triggers on drift threshold breach                                                |
| Call Site            | `server.py:33` — `Observer(cfg.db_path, ...)`                                                                                                                                                                                                   |
| D8: Auto-Corrective  | New `_apply_correction(obs: Observation) -> str` — when drift confidence > 0.7, logs correction to observer_history, triggers storage rebalance suggestion. New `correct()` MCP-callable method that runs detect + apply.                       |
| D10: Per-Memory Hash | New `hash_memory(content: str) -> str` — SHA-256 of content. New `verify_memory(memory_id: str, expected_hash: str) -> bool` — compares stored content hash. New `seal_memory(memory_id: str, content: str)` — stores hash in observer_history. |
| D10: Integrity Audit | New `audit_integrity(memory_ids: list[str]) -> list[dict]` — returns `[{memory_id, intact: bool, stored_hash, computed_hash}]` for each sealed memory.                                                                                          |

## Test Plan

1. **Auto-correct**: Inject drift observation with confidence 0.85 → `_apply_correction` returns correction log entry and stores to history
2. **Per-memory hash**: `hash_memory("hello")` != `hash_memory("hallo")` (avalanche property)
3. **Seal + verify**: `seal_memory(id, content)` → `verify_memory(id, hash)` returns True; after modifying content, returns False
4. **Integrity audit**: Seal 3 memories, corrupt 1 in DB, audit returns 2 intact, 1 tampered
