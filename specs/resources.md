# Intueina Resources — Orion Domain Reinforcement

| Field               | Value                                                                                                                                                                            |
| ------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Orion Domains       | D9 Economics                                                                                                                                                                     |
| File                | intueina/resources.py                                                                                                                                                            |
| Function            | ResourceBudget, PriorityRouter                                                                                                                                                   |
| Inputs              | Config budget limits, query cost metadata                                                                                                                                        |
| Outputs             | Allocated/rejected routing decisions                                                                                                                                             |
| Test Cases          | 1. Budget exhaustion blocks excess queries, 2. Priority routing sends high-priority queries first, 3. Cost tracking accumulates per-query stats                                  |
| Call Site           | `server.py` — `_handle_recall` wraps in budget check                                                                                                                             |
| D9: Resource Budget | `ResourceBudget(max_queries_per_minute=60, max_chroma_results=50)` — tracks consumption, returns `allow(estimated_cost: int) -> bool`. Resets every 60 seconds.                  |
| D9: Priority Router | `PriorityRouter` — wraps queries with priority tags (high/medium/low). High-priority bypasses budget exhaustion (reserve pool). Tracks `cost_history: list[dict]` for analytics. |
| D9: Cost Tracking   | `cost_history` stores `{query, results, chroma_cost, sql_cost, timestamp}`. Exposed via `health` tool metrics.                                                                   |

## Test Plan

1. **Budget exhaustion**: Set budget to 5/min, send 10 queries → first 5 pass, next 5 rejected with "budget exceeded"
2. **Priority bypass**: Budget exhausted but high-priority query arrives → accepted from reserve pool (up to 10% of budget)
3. **Cost tracking**: Run 3 queries → `cost_history` has 3 entries with non-zero costs
