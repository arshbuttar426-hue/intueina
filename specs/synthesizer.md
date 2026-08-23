# Intueina Synthesizer — Orion Domain Reinforcement

| Field         | Value                                                                                                                                                                                                                                                               |
| ------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Orion Domains | D4 Chemistry                                                                                                                                                                                                                                                        |
| File          | intueina/synthesizer.py                                                                                                                                                                                                                                             |
| Function      | Synthesizer                                                                                                                                                                                                                                                         |
| Inputs        | storage: Storage                                                                                                                                                                                                                                                    |
| Outputs       | Merged memory ID with cross-references                                                                                                                                                                                                                              |
| Test Cases    | 1. Merge two memories produces distilled insight, 2. Dedup prevents near-duplicate merge, 3. Cross-references link source memories                                                                                                                                  |
| Call Site     | `server.py` — new `synthesize` MCP tool                                                                                                                                                                                                                             |
| Design        | Takes 2+ memory IDs, extracts key claims from each, finds common ground + contradictions, produces a single distilled insight. Stores with memory_type="synthesis" and tags containing source memory IDs. Deduplication via cosine similarity > 0.85 skips storage. |

## Test Plan

1. **Two-memory merge**: Input memories about "ChromaDB latency" and "SQLite optimization" → output synthesis covering both topics
2. **Dedup skip**: Merge same two memories twice → second call returns existing synthesis ID
3. **Cross-references**: Synthesized memory has source_ids in tags, sources can be traversed backward
