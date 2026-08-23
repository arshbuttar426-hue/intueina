# Intueina Storage — Orion Domain Reinforcement

| Field                  | Value                                                                                                                                                                                                                                       |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Orion Domains          | D3 Physics, D7 Psychology, D8 Medicine                                                                                                                                                                                                      |
| File                   | intueina/storage.py                                                                                                                                                                                                                         |
| Function               | Storage                                                                                                                                                                                                                                     |
| Inputs                 | db_path: str, chroma_path: str                                                                                                                                                                                                              |
| Outputs                | Memory records, thinkpad entries, session logs                                                                                                                                                                                              |
| Test Cases             | 1. Compaction archives old memories before TTL, 2. Semantic diff returns changes between timestamps, 3. ChromaDB self-heal retries on failure, 4. TTL purge removes expired memories                                                        |
| Call Site              | `server.py:31` — `Storage(cfg.db_path, cfg.chroma_path)`                                                                                                                                                                                    |
| D3: Compaction         | `compact(ttl_days=90)` method — moves memories older than TTL to `memories_archive` table, prunes ChromaDB collection, logs count                                                                                                           |
| D3: TTL Purge          | `purge_expired(ttl_days=365)` — deletes memories older than absolute TTL from both SQLite and ChromaDB                                                                                                                                      |
| D7: Semantic Diff      | `diff_since(timestamp: float) -> dict` — returns `{"added": N, "updated": N, "deleted": N}` of memories changed since timestamp                                                                                                             |
| D8: Self-Heal ChromaDB | `_init_chroma()` with retry loop (3 attempts, exponential backoff). On persistent failure, set `self.chroma = None` and `self._degraded = True`. `recall()` falls back to `search_sql()` when degraded. Notify via `self._degraded_reason`. |

## Test Plan

1. **Compaction**: Insert 100 memories with old timestamps, run compact(ttl_days=1) → 100 moved to archive, 0 in main table
2. **Semantic diff**: Insert 3 memories, record timestamp, insert 2 more → diff returns added=2, updated=0
3. **Self-heal**: Mock ChromaDB failure → Storage initializes in degraded mode, recall returns SQL results
4. **TTL purge**: Insert memory with timestamp 400 days ago, run purge_expired(365) → memory gone from both stores
