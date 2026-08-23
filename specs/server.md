# Intueina Server — Orion Domain Reinforcement

| Field                    | Value                                                                                                                                  |
| ------------------------ | -------------------------------------------------------------------------------------------------------------------------------------- |
| Orion Domains            | D1 Metallurgy, D5 Biomechanics, D6 Philosophy                                                                                          |
| File                     | intueina/server.py                                                                                                                     |
| Function                 | IntueinaServer, MCPStdioServer                                                                                                         |
| Inputs                   | IntueinaConfig                                                                                                                         |
| Outputs                  | JSON-RPC 2.0 responses over stdio                                                                                                      |
| Test Cases               | 1. Rate limiter blocks flood, 2. Health check returns ChromaDB liveness, 3. Crossref toggles off, 4. Degraded mode on ChromaDB failure |
| Call Site                | `cli.py:31` — `IntueinaServer(cfg)`                                                                                                    |
| D1: Rate Limiter         | Token-bucket rate limiter on MCP tool dispatch, configurable tokens/sec, 429 response on exhaustion                                    |
| D5: Health Tool          | New `health` MCP tool returning ChromaDB status, observer status, memory count, uptime                                                 |
| D5: Graceful Degradation | ChromaDB init wrapped in try/except — falls back to text-only mode (SQL search without vectors)                                        |
| D6: Optional Crossref    | `_crossref` respects a `crossref` argument in the `recall` tool schema, defaults to false                                              |

## Test Plan

1. **Rate limit blocks flood**: Send 100 requests in 1s with rate=10/s → 90 should return rate-limited responses
2. **Health check**: Call `health` tool → returns `{"chromadb": "healthy", "memories": N, "observer": "active"}`
3. **Crossref off by default**: Call `recall` without crossref=True → response has no crossref data
4. **Degraded mode**: If ChromaDB init fails → server starts, `health` reports `chromadb: "degraded"`, recall falls back to SQL LIKE search
