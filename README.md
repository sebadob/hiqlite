# Hiqlite

Hiqlite is an embeddable SQLite database that can form a Raft cluster to provide strong consistency, high availability
(which is where `Hiqlite` derives from), replication, automatic leader fail-over and self-healing features.

## Project Status

This project is in a very early phase and if have quite a few things on the TODO before I would consider is to be
ready for testing in a real application. Until it hits v0.1.0, I will not not care about any changelog or keeping
track of changes, because it costs more time and effort than it's worth at this point.

However, you can take a look at the integration test or the example. These do work fine so far, but I am sure that
there are quite a few logic bugs. I also have many panics and assertions in case of logic errors all over the code.
I'd rather have my application panic so I can catch the error immediately than missing an error log and ending up in
an inconsistent state.

An integration tests for the currently implemented features does exist.  
Issues and discussions are not available in this early stage. It would simply not make any sense before v0.1.0.
I will also push directly to master until it's hitting the first release, which will most probably break the examples
from time to time. There will be an initial v0.0.1 tag though which put's the repo in a working state.

## Why

Why another SQLite replication solution? Other projects exist already that can do this. The problem is that none of
them checks all boxes. They either require an additional independent process running on the side which can do async
replication, need a special file system, or are running as a server.

I don't think that running SQLite as a server is a good solution. Yes, it is very resource friendly, but you loose its
biggest strength when doing this: having all you data local, which makes reads super fast without network latency.
Hiqlite builds on top of `rusqlite` and provides an async wrapper around it to make it easy usable with `tokio`.
For the Raft logic, it builds on top of`openraft` while providing its own storage and network implementations.

## Goal

Rust is such an efficient language that you usually only need one process to achieve whatever you need, for most
applications at least. An embedded SQLite makes the whole process very convenient. You get very fast local reads and at
the same time, it comes with the benefit that you don't have to manage an additional database, which you need to set up,
configure and more importantly maintain. And embedded SQLite will bring database updates basically for free when you
build a new version.

When configured correctly, SQLite offers really good performance and can handle most workloads these days. In very
first benchmarks that I did to find out if the project makes sense in the first place, I got up to 24.5k single
inserts / s on a cheap consumer grade M2 SSD. These tests were done on localhost with 3 different processes, but still
with real networking in between them. On another machine with older SATA SSDs it reached up to 16.5k inserts / s.

At the end, the goal is that you can have the simplicity and all the advantages of an embedded SQLite while still being
able to run your application highly available (which is almost always mandatory for me) and having automatic fail-over
in case of any errors or problems. The whole

### What is working

- whole Raft cluster setup
- everything a Raft is expected to do (thanks to `openraft`)
- Raft cluster auto-init based on given configuration
- persistent storage for Raft logs (with `rocksdb`) and SQLite state machine
- fully authenticated networking
- self-healing - features `auto-heal` and `auto-heal-logs`
- strongly consistent, replicated `execute` queries
    - on a leader node, the client will not even bother with using networking
    - on a non-leader node, it will automatically switch over to a network connection so the request
      is forwarded and initiated on the current Raft leader
- batched executes, all within a single Transaction (very fast)
- `query_as()` for local reads with auto-mapping to `struct`s implementing `serde::Deserialize`.
  This will end up behind a `serde` feature in the future which is not implemented yet.
- `query_map()` for local reads for `structs` that implement `impl<'r> From<&'r hiqlite::Row<'r>>` which is the
  faster method with more manual work

### TODOs

#### TODO implement

This list is my no means exhaustive, these are just the next big things on my list

- automatic database migrations
- simple, non-prepared batch queries
- TLS
- backups to s3
- restore from remote backup
- consistent queries on leader
- metrics / health endpoint or maybe even a simple health UI
- proper documentation
- more advanced examples

#### TODO add to tests

- shutdown / restart
- self-heal capabilities after data loss
