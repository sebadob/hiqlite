# External State Machine

For applications that already own a trusted consensus log. You decide the order of operations, you
make them durable, and you decide what a snapshot means. Hiqlite gives you the SQLite engine, the
read threads, the restore machinery, and the snapshot format.

This is opt-in and it is deliberately outside `default` and `full`. Enable `external-state-machine`
only when you own the consensus yourself.

## What you get

- `ExternalSqlite`: a single-writer SQLite engine. Your code applies the operations, in your order.
- Pooled read-only connections. Nothing else writes, so readers never block a writer.
- `ExternalDurability`: `synchronous=FULL` by default, with `NORMAL` and a rebuildable
  `ReplayableOff` as explicit choices. You pick what a power loss can cost you.
- Bounded retries: submitting the same entry again recovers the stored result, without executing it
  twice.
- Snapshots you can build, validate, and restore. `ExternalSnapshotEvidence` is the evidence you
  carry in your own format, and `ExternalSqlite::validate_snapshot` checks a decoded image before
  you touch a live projection.
- `ExternalCodec` for the encoding your log uses, `CommitSequence` for the position,
  `ExternalCommit` for the entry, `ExternalEntryKind` to say "applied" or "advance only",
  `ExternalApplied` and `ApplyOutcome` for the answer, `ExternalError` and `ExternalApplyError` for
  the failure.

Implement `DeterministicSqliteOperation` for your operations and use `hiqlite::rusqlite` so your
`Transaction` type is always the version Hiqlite was built against.

## Ownership

- No cluster, no network, no membership, no replication. If you want a cluster, run a node.
- The sequence you submit is the truth. It must be dense and monotonic: every position is
  represented, either by an applied operation or by an explicit advance-only entry.
- Every operation must be deterministic. The same operation on the same database produces the same
  result. Non-deterministic functions are refused on the write path, see `limits.md`.
- This engine never rebuilds your database and never deletes it. Auto-heal is not in play.
- No atomicity claim between SQLite, your consensus log, and any other storage. A crash between your
  durable commit and your SQLite apply is your contract to design for.

## Durability

`ExternalDurability` is the only dial, and it is a decision, not a tuning knob:

- `synchronous=FULL` - the default, and the answer for anything you cannot rebuild.
- `NORMAL` - less flush work, and a defined amount of loss you accept.
- `ReplayableOff` - the fastest, and only for a projection you can rebuild from your log.

## Snapshots

`build_snapshot_into` writes a snapshot to a path you own, and does not replace an existing path.
Restore closes and recreates the read connections. A read that is already inside its query is waited
for, up to a 30s grace; past it the restore proceeds, and that one read keeps answering the rows it
started on. Nothing can interrupt a read mid-query, so a read closure that blocks is the one way to
survive a restore.

The evidence you publish is `ExternalSnapshotEvidence`, and `EXTERNAL_SQLITE_SNAPSHOT_FORMAT` names
the page-image format. Your manifest codec maps that evidence into a versioned format that you own
and freeze. The Serde shapes here are descriptive: they describe the format, they do not promise it
as a wire contract. Your format is the contract.

Validating a snapshot is a call, not a ritual:

```rust, notest
ExternalSqlite::<MyCoordinate, MyOperation>::validate_snapshot(&snapshot).await?;
```

## Adopting an existing database

A legacy database becomes the initial projection only through `adopt_existing_projection`, and only
when the database is quiescent and you can prove it is the agreed initial state. There is no
auto-detect for this, on purpose: guessing which database is canonical is how a projection drifts.

`ExternalSqlite::open` is the normal constructor, with `ExternalSqliteOptions`.

## Where to start

The `external-state-machine` example in the repository is a walkthrough with a mocked consensus log:
the sequence, the operations, the retries, a snapshot, a restore. It is the shortest way to see the
ownership boundary in code.
