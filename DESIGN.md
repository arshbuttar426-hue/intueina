# Intueina — Design Document

## Core Insight

Existing memory solutions treat agent memory as a database problem — store and retrieve.
Intueina treats it as a cognition problem — learn, forget, recall, detect.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      INTUEINA SERVER                         │
│                                                              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐  │
│  │   Storage     │  │  Scheduler   │  │    Observer      │  │
│  │               │  │              │  │                  │  │
│  │ recall()      │  │ review()     │  │ ingest stream    │  │
│  │ memorize()    │  │ intervals    │  │ SHA-256 seq      │  │
│  │ think()       │  │ boot surf.   │  │ drift detect     │  │
│  │ learn()       │  │ due queue    │  │ falsify          │  │
│  │               │  │              │  │ push alerts      │  │
│  └──────┬────────┘  └──────┬───────┘  └────────┬─────────┘  │
│         │                  │                    │            │
│  ┌──────┴──────────────────┴────────────────────┴─────────┐ │
│  │                    Backend Layer                         │ │
│  │  SQLite (state) + Chroma/Qdrant/Pinecone (vectors)      │ │
│  └─────────────────────────────────────────────────────────┘ │
│                                                              │
│  MCP/stdio ←──────── request-response ────────→ Agent        │
│  WebSocket ←─────── relevance push ──────────→ Agent        │
└─────────────────────────────────────────────────────────────┘
```

## Three Layers

### Layer 1 — Storage

The cognitive stack. Five MCP tools:

| Tool     | Purpose                                  | Backend                     |
| -------- | ---------------------------------------- | --------------------------- |
| recall   | Full-text + semantic search              | Chroma vectors + SQLite FTS |
| memorize | Store a learning with metadata           | SQLite + Chroma embedding   |
| think    | Append to working memory                 | SQLite thinkpad table       |
| review   | Due reviews for spaced repetition        | SQLite review_schedule      |
| learn    | Distill session into structured learning | SQLite + Chroma             |

### Layer 2 — Scheduler

Spaced repetition engine. Default 3-day interval, doubling after each review.
On boot, surfaces all due items. Items overdue >7 days deprioritized.

### Layer 3 — Observer

DREAMSWAP v3 pipeline adapted for decision pattern monitoring:

- Ingest: agent queries, memorized content, session decisions
- Sequencer: SHA-256 hash chain for tamper evidence
- Detector: pattern comparison (new vs stored state)
- Falsifier: hypothesis testing ("contradicts stored principle X")
- Push: WebSocket alert on threshold breach

Observer is opt-in (`observer.enabled = true` in config).

## Transport

Two channels, not one:

1. **MCP/stdio** — synchronous request-response. Agent calls `recall()`, gets results.
2. **WebSocket** — asynchronous push. When agent calls `recall("X")`, server pushes
   cross-references: "also check Y (due review) and Z (thinkpad entry)."

No polling. No polling interval config. Just relevant context pushed when it makes sense.

## Boot Flow

1. Agent connects via MCP
2. Server loads state:
   - Due reviews from SQLite scheduler
   - Thinkpad contents
   - Recent learnings for context priming
3. Push via WebSocket: "Boot state loaded. 2 reviews due. Thinkpad has 3 entries."
4. Agent is alive with yesterday's context

## Why Not Siyuan

Siyuan is a note-taking app doing double duty as memory. The API is hostile (HTML block trees,
opaque error codes, title param ignored), the footprint is heavy (Electron app running a
full kernel), and there's no native MCP support.

Intueina is a memory server from the ground up:

- No UI needed — it's a server, not an app
- MCP-native — the protocol it speaks, not a bridge we built
- Backend-agnostic — swap Chroma for Qdrant, SQLite for Postgres
- Lightweight — pip install, ~/.intueina/ directory, done
