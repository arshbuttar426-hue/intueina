# Intueina — MCP Cognitive Memory Server

intueina (from intuition) is a backend-agnostic cognitive memory server for AI agents.
MCP-native. Zero install friction. One config line mounts a full cognitive stack: recall,
memorize, think, review, learn — plus a drift observer powered by the DREAMSWAP pipeline.

## Specification

| Field             | Value                            |
| ----------------- | -------------------------------- |
| Project name      | intueina                         |
| Version           | 0.1.0                            |
| Install           | pip install intueina             |
| CLI command       | intueina serve --transport stdio |
| Python version    | >=3.10                           |
| Dependencies      | mcp, chromadb, sqlite3 (stdlib)  |
| Standard lib only | no (requires chromadb, mcp)      |

### Layer 1 — Storage

| Field                | Value                                                                                    |
| -------------------- | ---------------------------------------------------------------------------------------- |
| Storage: recall      | Full-text + semantic search across stored memories. Returns top-N matches with metadata. |
| Storage: memorize    | Store a memory with title, content, type, domain, tags. Auto-embed for semantic search.  |
| Storage: think       | Append to persistent working memory (Thinkpad). Mutable, survives session compaction.    |
| Storage: review      | Return list of learnings due for spaced repetition review.                               |
| Storage: learn       | Distill session context into structured learning. Auto-schedule review.                  |
| Storage: backend     | SQLite for structured state, Chroma for vector embeddings. Pluggable backend interface.  |
| Transport: primary   | MCP/stdio for request-response (recall, memorize, think, review, learn)                  |
| Transport: secondary | WebSocket for relevance cross-reference push on recall events                            |

### Layer 2 — Scheduler (Spaced Repetition)

| Field                 | Value                                                                          |
| --------------------- | ------------------------------------------------------------------------------ |
| Default interval      | 3 days                                                                         |
| Interval doubling     | interval * 2 after each successful review                                      |
| Review detection      | Markdown field: `review: last=YYYY-MM-DD; interval=N` or equivalent SQL schema |
| Boot surface          | On agent connect, push due reviews via WebSocket                               |
| Max overdue surfacing | 7 days — items older than this pushed to bottom                                |
| Storage               | SQLite `reviews` table: id, memory_id, last_reviewed, interval, due_date       |

### Layer 3 — Observer (DREAMSWAP Pipeline)

| Field         | Value                                                                                     |
| ------------- | ----------------------------------------------------------------------------------------- |
| Ingest stream | Agent queries, memorized content, session decisions                                       |
| Sequencer     | SHA-256 hash chain for tamper-evident audit trail                                         |
| Detector      | Pattern comparison — new input vs stored state. Detects drift, contradiction, degradation |
| Falsifier     | Hypothesis testing — "this new decision contradicts stored principle X"                   |
| Push channel  | WebSocket alert when drift/contradiction threshold crossed                                |
| Backend       | SQLite for history, in-memory for real-time detection                                     |

### Agent Lifecycle Integration

| Field                 | Value                                                                                   |
| --------------------- | --------------------------------------------------------------------------------------- |
| Boot: state injection | Load due reviews + thinkpad + principles. Push via WebSocket.                           |
| Boot: context prime   | Agent receives past context at session start — no blank slate.                          |
| Runtime: recall       | Agent calls recall("query") → MCP returns results → WebSocket pushes related context    |
| Runtime: crossref     | Every recall triggers cross-reference: due reviews, thinkpad entries, related decisions |
| Compaction: capture   | Hooks capture context before window reset. Stored in thinkpad.                          |
| Shutdown: distill     | Optional session-end learning distillation trigger                                      |

### Configuration

| Field            | Value                                            |
| ---------------- | ------------------------------------------------ |
| Config file      | ~/.intueina.toml or project-local .intueina.toml |
| Vector backend   | chroma (default), qdrant, pinecone (pluggable)   |
| DB path          | ~/.intueina/state.db (default)                   |
| Review interval  | 3 (default, days)                                |
| Observer enabled | false (default, opt-in for now)                  |
| WebSocket port   | 9123 (default)                                   |
| MCP transport    | stdio (default), http (port 9124)                |
