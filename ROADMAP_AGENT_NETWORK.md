# Decentralized Agent Information Network — Roadmap

## Phase 0: Core Protocol & Node (3-4 months, 1-2 people)

**Language: Rust** — memory safety, zero-cost abstractions, WASM targets, native crypto performance.

**Architecture:**
- Node exposes a single UDP/TCP port for P2P DHT traffic
- Agent-facing API binds to localhost only (configurable for LAN with explicit opt-in)
- Only speaks the structured envelope protocol — no HTTP, no shell, no filesystem access on the wire

**Security model:**
- All inbound data deserialized through `serde` from a single `from_slice` boundary — malformed packets are dropped, no undefined behavior
- Rust memory safety eliminates the class of CVEs that plague C/C++ network services
- Node key stored at `$HOME/.ainet/` with restricted permissions, never used for system access
- Rate-limited inbound per peer to prevent amplification attacks
- No shell execution, no file access outside data directory, no HTTP server

**Deliverables:**
- **Envelope spec** — CBOR wire format, Ed25519 signing, Blake3 hashing, TTL semantics
- **Kademlia DHT** with NAT traversal (STUN + hole-punching, relay fallback via WebRTC/QUIC)
- **Bloom filter gossip protocol** — each node broadcasts what it holds, peers merge locally
- **Basic topic schemas** — `crate_info`, `docs:fn`, `docs:struct`, `weather`, `news:item`, `package_meta`
- **CLI node** — `ainetd`, single binary, auto-configures, joins DHT on startup
- **Agent-facing query endpoint** — localhost TCP/Unix socket; agent sends structured query, node fans out to DHT, returns merged result
- **Minimal client library** — thin SDK that connects to local node, ~200 lines

**Gate:** Node runs on a Raspberry Pi behind a home router, peers discover each other, basic queries work. ~2k LOC.

---

## Phase 1: Parser Ecosystem (2-3 months)

**Language: Polyglot** — Rust for perf-critical parsers, Python for scraping glue, TypeScript for JS-ecosystem parsers.

**Deliverables:**
- `ainet-parser-cratesio` — polls crates.io API, publishes structured crate info + docs
- `ainet-parser-pypi` — same for PyPI
- `ainet-parser-wikipedia` — XML dump → structured fact extraction
- `ainet-parser-npm` — npm registry metadata
- `ainet-parser-weather` — pulls from weather.gov / OpenWeatherMap
- `ainet-parser-news-rss` — generic RSS/Atom structured converter
- **Parser SDK** — library (Rust) that handles auth, signing, publishing so anyone can write a parser in ~50 lines

**Gate:** At least 5 parsers running on community nodes, publishing fresh data daily. ~3k LOC core, plus variable per-parser.

---

## Phase 2: Query Layer & Discovery (2 months)

**Language: Rust**

**Deliverables:**
- **Structured query language** — simple JSON or CBOR query format (`{topic, filter: {field: value}, limit, freshness}`)
- **Volunteer index nodes** — nodes can opt in to index a topic (maintain inverted index for that namespace). Discoverable via DHT.
- **Multi-hop query routing** — if your node doesn't know, it asks neighbors recursively (TTL-limited)
- **Query merging** — parallel fan-out to multiple index nodes, merge results by hash dedup + score
- **Freshness-aware routing** — nodes advertise TTL ranges; stale data is de-prioritized

**Gate:** Complex queries work ("find Rust crates about async networking updated in the last 6 months"). ~1.5k LOC.

---

## Phase 3: Trust & Reputation (2 months)

**Language: Rust**

**Deliverables:**
- **Content signing** — every envelope signed by publisher's Ed25519 key
- **Key discovery** — DHT maps `key_hash → public_key_info`; you choose who to trust
- **Reputation scores** — nodes track peer reliability (uptime, response accuracy, signature validity). Gossiped but not global — each node computes its own view.
- **Web of trust** — optionally, you can sign another node's key as "trusted for topic X"
- **Spam backpressure** — nodes reject peers that repeatedly publish unverifiable or low-value data

**Gate:** A bad actor's data is rejected by the network within minutes of detection. ~1.5k LOC.

---

## Phase 4: Client Libraries (1-2 months)

**Languages: Rust, Python, TypeScript**

**Deliverables:**
- `ainet-client` crate — Rust SDK for agents. `ainet::query("crate_info", filter!({"name": "tokio"})).await`
- `ainet-py` — Python bindings via PyO3
- `ainet-js` — WASM bundle (compile Rust node core to WASM, wrap in a thin JS API)
- **Cache layer** — local SQLite/LMDB cache so repeated queries are instant
- **Auto-peer-discovery** — embedded bootstrap node list, refreshed periodically from the network

**Gate:** An AI agent (in any of the three languages) can install a library, write 3 lines of code, and query the network. ~1k LOC.

---

## Phase 5: Distribution & Community (ongoing)

**Deliverables:**
- **Docker image** for `ainetd` — one-click node deployment
- **Home router guide** — port forwarding, UPnP, or relay mode
- **Raspberry Pi image** — flash-and-forget node
- **Bootstrap node** — well-known public bootstrap endpoints (run by project maintainers, gossiped and rotated)
- **Parser contribution guide** — how to write and publish a parser in 15 minutes
- **Topic registry** — where new schemas are proposed and standardized

**Gate:** Anyone can spin up a node in under 5 minutes. New parsers appear weekly.

---

## Phase 6: Evolve (continuous)

- **Schema versioning** — topics get `version` field, backward-compatible evolution
- **Payment layer (optional)** — micro-tipping for high-value nodes (energy costs)
- **Federated trust** — domain-specific reputation (e.g., Mozilla runs a "trusted for Rust" key)
- **Gateway bridge** — HTTP gateway so legacy agents can query the network without a native client

---

### Totals

- **Core node + protocol:** ~10-12k LOC
- **Per parser:** ~5k LOC
- **One solid engineer:** Phase 0-1 functional in 6 months
- **Small team (2-3):** Phase 0-3 in a year

Rust wins for: DHT + crypto tight loops, WASM compilation for browser clients, memory safety in a trustless network, single static binary for hobbyist deployment.
