# Glossary

The words the rest of the docs use, in the meaning they carry there.

- **Node** - one running Hiqlite instance: your process, a pod, a container. A node has a data dir,
  a Raft port, and an API port.
- **Cluster** - the set of nodes in your node list, running the same data.
- **Raft group** - one replicated state machine with its own members and its own leader. A node
  runs a group for its SQLite database and groups for its caches.
- **`raft_type`** - the group selector in the endpoints: `sqlite` or `cache`.
- **Leader** - the member that decides the order of writes for a group. Writes go to the leader.
- **Follower** - a member that applies what the leader ordered.
- **Voter** - a member that counts in the majority. A write commits when a majority of voters
  acknowledge it.
- **Learner** - a member that replicates and serves local reads, does not vote, and cannot be
  elected. A read replica, a supervised re-join, a far node you never want to lead.
- **Majority (quorum)** - more than half of the voters. 1 of 1, 2 of 2, 2 of 3, 3 of 4, 3 of 5.
- **Term** - the leadership era. A handover moves the cluster to the next term.
- **Marker (`election_disable`)** - the flag in a node line: this node never nominates itself. It
  keeps its vote, keeps the majority intact, and accepts a leadership handover.
- **Handover (leadership transfer)** - the current leader giving leadership to a member you name.
  One term, no election timeout, no cluster-wide disruption.
- **Election** - members choosing a leader after the old one is gone. Takes a few seconds.
- **Log (write-ahead log)** - the ordered record of everything the cluster has committed. Nodes
  replay it to build the same state.
- **Snapshot** - a compact copy of the state at a log position, so a new node does not replay
  history from the beginning.
- **Log roll-over** - the log moving to a new position range, which a restore does for you.
- **State machine** - the state a group produces from its log: a SQLite database, or a cache.
- **Log index / log position** - the number of a committed entry. Comparing positions tells you
  whether a node is caught up.
- **Cache index** - which cache a call addresses. It is the position of the variant in your
  `CacheVariants` enum, it is persisted in the cache group's log and snapshots, and it never
  changes: append at the end, validated at every startup.
- **Data dir** - the directory that holds one node's database, log, snapshots, cache files, and
  local backups. One node per directory.
- **Auto-heal** - the rebuild a node performs after a hard kill: latest snapshot plus the log, so it
  starts consistent. Correct, and slower than a graceful restart.
- **Graceful shutdown** - readiness off, leadership handed over, state written. The shutdown you
  always want.
- **Rolling release** - upgrading one node at a time, each restart gated by readiness. The default
  upgrade shape, and the reason a cluster runs without downtime.
- **Cold restart** - every node stopped, every binary replaced, every node started. Required only by
  the releases that announce it in the changelog's `Breaking` section.
- **API secret (`secret_api`)** - the shared secret your clients send as a bearer token on the API
  port, and the secret that authenticates cluster control.
- **Raft secret (`secret_raft`)** - the shared secret for cluster traffic on the Raft port.
- **Encryption keys (`enc_keys`, `enc_key_active`)** - the keys that encrypt backups and the
  dashboard store. The active key writes; every key in the list can read.
- **Proxy** - a single connection point that load-balances a remote client across cluster members.
  No data, no membership, one extra hop.
- **Remote client** - a `Client` in your app that talks to a cluster over the network and runs no
  local state.
- **Read replica** - a learner you send reads to. Its reads are as fresh as the log it has applied.
- **Lease (lock)** - the time window a distributed lock is granted for, extended while the holder is
  alive.
- **`REFERENCE_CONFIG`** - `hiqlite::REFERENCE_CONFIG`: the annotated key list with defaults.
- **Changelog** - what a release changed and what you must do. Read it before every upgrade.
