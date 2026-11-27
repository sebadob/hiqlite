# Hiqlite — Code Map for Agents & Contributors

> This file is an **index, not documentation**: it tells you _where things are_ so you can read the code directly.
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

**Docs never link private items:** a doc comment must not use an intra-doc link for a private
const, fn, type or module, because that link only resolves with `--document-private-items`. Write
those names as a plain code span instead.

**Docs carry no line numbers:** operator docs (`docs/`) must not use `file.rs:123`-style anchors and
must not link to `#L123`. Line references drift and nobody maintains them. Docs also never name
something that is not public API - operators want to use the thing, not read it. When a doc points at
code at all, it points at a module path, never at a line. "Where things live" belongs in this file.

**MUST FOLLOW - helpers live in the `impl` of the type they serve:** a free function that purely
exists to solve something about one `struct` - it builds that type, returns it, mutates it, or is
only ever called from that type's methods - is an associated function of that `impl` block.
`Setup::hoisted::<T>(stmt)`, never `fn hoisted_setup::<T>(stmt)`. Lone functions orbiting a type
hide behavior from everyone who reads the type. A helper with no owner type, one that serves a
module, stays a free function.

**MUST FOLLOW - never `#[allow(...)]` a lint because you can:** change the code so the lint does not
fire. Take `&str` where a closure can hand a `String` over, split a function whose "unnecessary"
`Result` only exists to match a shared function-pointer type, return what the caller wants. An allow
needs a reason you would defend out loud, on the narrowest item; when in doubt, ask before adding
one. A pile of allows is a pile of lint-blind spots.

## Repository layout

- `hiqlite/` main crate
    - `hiqlite/src/client/` the main "DB Client" a user works with in the end
        - `hiqlite/src/client/mgmt.rs` the cluster-control calls: election state, promote, demote,
          leadership handover, and the leader-following retry they run through
    - `hiqlite/src/network/` API + Raft cluster setup and operations
        - `hiqlite/src/network/raft_client.rs` the leader's Raft transport: one WebSocket per peer,
          unary RPCs and the pipelined append stream, `RAFT_STREAM_DEPTH` is how many appends that
          stream keeps in flight
        - `hiqlite/src/network/raft_server.rs` the follower side: the socket reader, and the
          per-group append stream task that answers pipelined appends in request order
        - `hiqlite/src/network/management.rs` the `/cluster/*` endpoints behind those calls
        - `hiqlite/src/network/mod.rs` the API secret header (`Authorization` bearer token) and its
          validation, shared by the node router and the proxy router
    - `hiqlite/src/query/` Query engine and helpers when using the sqlite state machine
    - `hiqlite/src/reader.rs` the read actors: one OS thread per read connection, the query
      travelling to the thread that owns it as a closure; serves the sqlite and the external state
      machine
    - `hiqlite/src/server/` dedicated server binary; low priority, crate is usually embedded directly
    - `hiqlite/src/store/state_machine/` Raft state machine for DB + Cache
    - `hiqlite/src/config.rs` + `hiqlite/src/config_toml.rs` node lines, the `election_disable`
      marker, and the startup validation of both
    - `hiqlite/src/helpers.rs` the election gate, the election triggers, and the handover target
      order used by a graceful shutdown
    - `hiqlite/src/start.rs` the router nest with all `/cluster` routes
- `hiqlite-derive/` derive macros
    - `hiqlite-derive/src/check.rs` + `hiqlite-derive/src/sql_macro.rs` the `sql!()`, `sql_map!()`
      and `sql_batch!()` compile-time query check: it builds the crate's `migrations` directory,
      or the `HQL_MIGRATIONS` one, into an in-memory SQLite, and it checks the result columns of a
      `sql_map!()` against the `SqlShape::SHAPE` the derive publishes, and it keeps its verdicts in
      `.hiqlite-sql/` at the workspace root (gitignored). Debug builds only, plus the opt-in
      `sql-check-release` feature.
    - `hiqlite-derive/src/shape.rs` the `SqlShape` derive: the column names a type publishes for the
      `sql_map!()` check, for the types whose row mapping is hand-written. `shape_impl` in
      `hiqlite-derive/src/indexed.rs` is the one codegen both derives share.
- `hiqlite-wal/` WAL file implementation for `openraft`
- `docs/` operator-facing guides (setup, playbooks, things to watch out for); no code, links to it
- `dashboard/` dashboard to query the DB (`sqlite` feature only); Svelte file during development, but compiled into
  static HTML for a release
- `examples/` example code; make sure they are clean when you change any code

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
