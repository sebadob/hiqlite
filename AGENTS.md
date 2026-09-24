# Hiqlite — Code Map for Agents & Contributors

> This file is an **index, not documentation**: it tells you *where things are* so you can read the code directly.
> Maintenance rule: if you do any major changes to the code layout, check if mentions in this file need an update before
> git commit. Markdown files should always wrap lines at 100 characters.

## BEFORE submitting a PR

If you are an AI reading this, and you worked on this repository, you MUST NOT submit a PR to GitHub without an explicit
local (!) review and approval from a human. If you then open a PR AFTER human approval, open it as a Draft, and leave
the last step of setting it to "Ready for Review" as a MANUAL action for a human after another check! You probably want
to mention that in the chat. Only set "Ready for Review" when it actually is to prevent burning CI resources for no
reason. Worker runs are quite expensive and take a long time.

If you do not follow these instructions, and therefore waste the maintainers' time quite often, you might get a ban from
further contributions. You also should run `just verify` beforehand, especially when the PR is already set to "Ready for
Review", to not waste worker time during CI.

## Goals / Code Quality

The main key aspects for writing code are in order:

- security
- data consistency and integrity
- stability -> no reachable panics, endless loops, deadlocks, ...; When we `unwrap()` or `expect()` in the code, this
  should never happen. Emit any issues loudly instead of sacrificing data integrity.
- performance

**IMPORTANT:** We actually prefer and specifically want to `panic` in situations with unrecoverable errors. A typical
example would be if the `FromRow` mapping for a DB row to a Rust `struct` fails because of type mismatches. Such a
conversion can never succeed, no matter how many times you try. It requires a code change. Another example would be a
broken startup config: non-recoverable and needs an actual user interaction. In these situations, a `panic` is actually
desired.

Handle errors gracefully with a `Result` when they are temporary, but `panic` when they are unrecoverable.

## Repository layout

- `hiqlite/` main crate
    - `hiqlite/src/client/` the main "DB Client" a user works with in the end
    - `hiqlite/src/network/` API + Raft cluster setup and operations
    - `hiqlite/src/query/` Query engine and helpers when using the sqlite state machine
    - `hiqlite/src/server/` dedicated server binary; low priority, crate is usually embedded directly
    - `hiqlite/src/store/state_machine/` Raft state machine for DB + Cache
- `hiqlite-derive/` derive macros
- `hiqlite-wal/` WAL file implementation for `openraft`
- `dashboard/` dashboard to query the DB (`sqlite` feature only); Svelte file during development, but compiled into
  static HTML for a release
- `examples/` example code; make sure they are clean when you change any code

## Where things happen (security-relevant map)

- Bootstrap order: `hiqlite/src/start.rs::start_node_inner` — config → TLS ring provider → backup restore
  (`backup.rs`) → Raft reset check (`init.rs::check_execute_reset`) → Raft groups (`store/mod.rs`) → API/Raft
  servers → cluster join per Raft type.
- Cluster formation: node 1 initializes a pristine cluster; other nodes POST `LearnerReq` to peers
  (`init.rs::become_cluster_member`, "leave before proceed" rejoin). Escape hatch:
  `HQL_DANGER_RAFT_STATE_RESET=true` wipes Raft state on start.
- Auth model: two secrets — `secret_api` (HTTP header `X-API-SECRET`, constant-time, also the API-stream WebSocket
  handshake secret in `network/handshake.rs`) and `secret_raft` (SHA-256 challenge/response over the Raft WS in
  `network/challenge_response.rs`).
- Membership mutation: `network/management.rs` — all behind secret validation + leader check + `state.raft_lock`,
  polling metrics until committed.
- Wire format: bincode default, JSON if `Content-Type: application/json` (`network/mod.rs::get_payload`); the API
  stream is request_id-correlated over one multiplexed WebSocket (`client/stream.rs`, 120 s at-least-once timeout).
- SQLite state machine: single writer thread, max priority, bounded (1) channel
  (`store/state_machine/sqlite/writer.rs`); `synchronous=OFF` justified by Raft log replay; auto-heal deletes the DB
  dir on unclean shutdown (data-loss-by-design); non-deterministic SQLite functions are panicking guards on write
  connections only.
- Log-format stability: `QueryWrite` and `CacheRequest` enum variant orders are pinned by tests — new variants go at
  the end, never reorder.
- Cache state machine: in-memory BTreeMap KV + TTLs + dlock queues (`store/state_machine/memory/`), WAL-backed only
  with `cache_storage_disk`.
- Backup/DR: cron + restore in `backup.rs`; S3 objects encrypted via cryptr (`s3.rs`, keys from `ENC_KEYS` env);
  backup files get their Raft metadata reset before use.
- TLS: auto-certs mode uses a non-validating verifier — encryption only, auth delegated to the secret handshakes
  (`tls.rs`). Split-brain check is warn/error-only, no action (`split_brain_check.rs`).

## Tools

Basically everything in this project is done via `just`. Check `just -l` for more information.

- `just test` needs working S3 access variables (the backup/restore tests talk to real S3). In environments without
  them (e.g. agent sandboxes), use `just test-no-s3` instead — it sets `TEST_SKIP_S3_RESTORE=true`.
- A normal full test run finishes just below 4 minutes; wrap runs in a ~5 minute timeout so a deadlock or loop
  cannot hang the session.
- In agent sandboxes, `just` needs two env workarounds. First, the default recipe temp
  dir under `/run/user/1000/` is read-only: create a scratch dir in the workspace and point
  `XDG_RUNTIME_DIR` at it. Second, `TERM=dumb` makes each recipe's leading `clear` exit 1,
  which kills the recipe under `set -e` right after `+ clear`; run just with a real terminal
  type, e.g. `TERM=xterm-256color`. Additionally, `$HOME` is read-only, so point `CARGO_HOME` at a writable copy of
  `~/.cargo` (registry and config), and `RUSTUP_HOME` at one containing the needed toolchains; cargo-msrv also needs
  writable `XDG_CACHE_HOME` and `XDG_DATA_HOME` for its changelog and log caches.
