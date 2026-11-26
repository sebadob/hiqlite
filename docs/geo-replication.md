# Geo-Replication Without Latency Penalties

Goal: run a Raft cluster where one node sits in a far-away region, its latency never lands in the
commit path, and that node is still a fully voting member that can take over the leadership when
you tell it to.

This document is the recipe: the config, the API, the handover rules, the ops playbook, and the list
of things that will bite you.

TL;DR:

- the marker is an optional 4th token on a node line: `id addr_raft addr_api election_disable`
- a marked node starts with `enable_elect = false` for both of its Raft groups: it never nominates
  itself, for the SQLite group and for the cache group
- a marked node is a **full voter**: it grants votes, it counts towards the quorum, and a leader
  hands leadership over to it
- `GET` / `PUT` / `POST` on `/cluster/election` read and change that state at runtime, per node
- a leader skips the marked nodes when it picks its handover candidates, and only uses them as the
  last fallback

## 1. The shape, and why it is shaped like that

A Raft commit needs a majority of the voters to acknowledge. The latency of your slowest
acknowledgement is inside the commit path. With three voters that gives you:

| Shape         | Voters | Geo role           | Quorum | Geo node in commit path      | Survives loss of |
| ------------- | ------ | ------------------ | ------ | ---------------------------- | ---------------- |
| 2 LAN + 1 geo | 3      | voter, marked      | 2 of 3 | only when a LAN node is gone | 1 voter          |
| 3 LAN + 1 geo | 4      | learner            | 3 of 4 | never                        | 1 voter          |
| 2 LAN + 2 geo | 4      | voters, geo marked | 3 of 4 | always                       | 1 voter          |
| 1 LAN + 1 geo | 2      | learner            | 2 of 2 | always                       | nobody           |

The 2 LAN + 1 geo shape is the one this document is about:

```
node 1: region A (LAN)   node 2: region A (LAN)   node 3: region B (far)
```

Quorum is 2 of 3. In steady state the two acknowledgements are the LAN pair, so the geo round
trip never enters the write path. The geo node joins the quorum exactly when one LAN node is
missing.

The quorum is a majority of the voters, so 2 voters need both of them, 3 need 2 and 4 need 3. The
last row of the table has no redundancy at all, and 2 LAN + 2 geo buys you availability for one
lost voter at the price of a geo hop in every single commit - which is the thing you wanted to
avoid.

Why a voter and not a learner: a learner can never become a leader (Raft rule), it cannot promote
itself, and it does not count towards the quorum at all - so a learner geo node buys you a read
replica and nothing else. A marked voter keeps the quorum of three, survives the loss of one voter,
and can be promoted to leader by a leader that hands over, or by you at runtime.

Why the marked voter does not make the leader switch slow: a node that nominates itself has to
collect votes from its peers, and a leader that hands leadership to a far node moves the whole write
path into that region. The marker removes the self-nomination, and the handover order prefers the
nodes that can win on their own.

## 2. Configuration

### The node line

Each entry in `nodes` / `HQL_NODES` is `id addr_raft addr_api`, plus the optional 4th token
`election_disable`:

```bash
HQL_NODES="1 10.0.0.1:8100 10.0.0.1:8200\n2 10.0.0.2:8100 10.0.0.2:8200\n3 10.1.0.3:8100 10.1.0.3:8200 election_disable"
```

```toml
[hiqlite]
node_id = 3
nodes = [
  "1 10.0.0.1:8100 10.0.0.1:8200",
  "2 10.0.0.2:8100 10.0.0.2:8200",
  "3 10.1.0.3:8100 10.1.0.3:8200 election_disable",
]
```

Anything the parser cannot understand is a startup `panic!`: an unknown 4th token, a 5th value, a
missing address. A broken startup config is unrecoverable and has to be fixed, which is exactly what
a loud panic is for.

Watch out: put the marker into the node list of **every** node. The list is config, it is not
replicated through Raft. A leader only skips the marked nodes it sees in its own config, so a
leader whose copy of the list lost the marker will happily hand leadership to the far node.

`NodeConfig::is_valid()` refuses a config whose `election_disable` list contains an id that is not
in `nodes`, so a typo in a hand-written list is caught at startup.

### Code-built configs

The env and TOML parsers handle the marker themselves. If you build a `NodeConfig` in code, set the
list and apply it before you hand the config to `start_node`:

```rust
let mut config = NodeConfig::from_toml("hiqlite.toml", enc_keys, None, None).await?;
config.election_disable = vec![3];
config.apply_election_disable();
```

`apply_election_disable()` sets `raft_config.enable_elect = false` when the config's own `node_id`
is in the list, and logs a warning about it.

### The marker is deliberately not a field of `Node`

`hiqlite::Node` is the node type that openraft serializes into Raft log entries, snapshots, and
network messages, and the WAL encodes with a non-self-describing legacy bincode layout. A field
there changes bytes on disk and on the wire in both directions. The marker lives in `NodeConfig`
only, and that never reaches the log, so an existing data dir stays loadable and rollback to an
older release keeps working.

The price of that choice: the marker is per-node config, not cluster state. Keep the node lists of
all nodes identical, in one place, and ship them together.

## 3. What `enable_elect = false` does - and what it does not do

This is the section to read before you rely on the marker.

It does:

- the node never nominates itself when its election timeout fires, for both Raft groups
- a leader that hands over prefers the other voters first and uses the marked node last
- the node stays in the membership, so it keeps its vote and keeps the quorum

It does not:

- it does not stop the node from granting votes to others
- it does not stop the node from **accepting** a leadership transfer, which is how it becomes a
  leader when it is the only candidate left
- it does not gate the paths where openraft re-establishes a leadership for a node: an incumbent
  leader that is challenged by a higher vote takes its position back without an election timeout, so
  the gate does not apply there
- it does not gate openraft's own step-down target when the leader is removed from the membership:
  openraft picks that target itself
- it does not remove the node from the quorum, and it does not make the cluster tolerate the loss of
  two voters

A learner is a different thing entirely: it can never win an election, it cannot promote itself, and
`POST /cluster/election` refuses to trigger an election for a learner, because that only inflates
terms.

`enable_elect` is openraft's **runtime** config, held in memory. Hiqlite keeps its own copy for each
of the node's two groups, because openraft exposes a setter only. Nothing about it is persisted: a
restart applies the marker of the node line again.

## 4. The API

All three endpoints are under `/cluster/election`, there is no `{raft_type}` in the path, and each
call acts on every Raft group the node runs, in the same way. They authenticate with the API secret
as a bearer token, `Authorization: Bearer <secret_api>`. None of them require the calling node to be
the leader - every
node answers for itself. `GET` answers whenever the node serves its metrics. `PUT` and `POST` are
refused while one of the node's Raft groups is stopped or not initialized, and `POST` refuses a node
that is a learner in any of its groups, because the calls always act on all groups together.

The header is the standard `Authorization` on purpose: proxies, clients and request loggers redact
it by default, which a custom `X-API-SECRET` never was. The `Bearer ` scheme is required - matched
case-insensitively - and a request that carries the bare secret is refused. The `hiqlite::Client`
builds the header itself, so client code is untouched; this is for `curl`, monitoring scripts and
anything else that calls the API by hand.

| Method                   | Body / query               | Effect                                               |
| ------------------------ | -------------------------- | ---------------------------------------------------- |
| `GET /cluster/election`  | -                          | the state of each group on this node                 |
| `PUT /cluster/election`  | `{"enable_elect": <bool>}` | sets it for all groups of this node                  |
| `POST /cluster/election` | -                          | this node campaigns for the leadership of all groups |

```bash
# every call carries the node's API secret as a bearer token
AUTH="Authorization: Bearer $SECRET"

# read the state of a node
curl -s -H "$AUTH" -H "Accept: application/json" \
  http://10.1.0.3:8200/cluster/election

# let the far node start elections on its own again
curl -s -X PUT -H "$AUTH" -H "content-type: application/json" \
  -H "Accept: application/json" \
  -d '{"enable_elect":true}' http://10.1.0.3:8200/cluster/election

# switch it off again
curl -s -X PUT -H "$AUTH" -H "content-type: application/json" \
  -H "Accept: application/json" \
  -d '{"enable_elect":false}' http://10.1.0.3:8200/cluster/election

# make a node campaign for the leadership of all its groups
curl -s -X POST -H "$AUTH" -H "Accept: application/json" \
  http://10.1.0.3:8200/cluster/election
```

`Accept: application/json` gives you readable answers from a terminal. Without it the answers are
bincode, which is what the `hiqlite::Client` uses, and the request body follows the same rule with
`content-type`.

A JSON answer looks like this:

```json
{
    "sqlite": {
        "node_id": 3,
        "state": "Follower",
        "leader": 2,
        "is_voter": true,
        "enable_elect": false,
        "election_disable": true
    },
    "cache": {
        "node_id": 3,
        "state": "Follower",
        "leader": 2,
        "is_voter": true,
        "enable_elect": false,
        "election_disable": true
    }
}
```

`enable_elect` is the effective runtime value, `election_disable` is the marker of the node line.
They differ after a `PUT`, and they converge again on the next restart. `state` is openraft's server
state (`Leader`, `Follower`, `Learner`, `PreCandidate`, `Candidate`), `leader` is that group's
current leader.

`POST` runs openraft's Pre-Vote round: the term is untouched when the node cannot win, a node whose
leader lease is still valid is refused, and a node whose own elections are disabled is refused by its
own gate. That makes it safe as a probe, and it is the only way to ask a node to take over by itself.
A leader change you actually want is a transfer, asked of the node that holds the seat:
`POST /cluster/transfer_leader` with `{ "node_id": 2 }` - see section 8 for why there is no bypass.

## 5. The client helpers

The same three calls, typed, from `hiqlite::Client`:

```rust
let states = client.cluster_election_state().await?; // BTreeMap<NodeId, ElectionState>
client.set_node_election(3, true).await?;
client.elect_node(3).await?;
```

A **remote** client (`Client::remote`, built from the node addresses and the API secret) reaches
every node: it asks each configured address for its state, resolves a node id to its address through
the answers, and only fails when no node answers. Nodes that do not answer are skipped with an
`error` log, so a half-reachable cluster gives you a partial map instead of an error.

An **embedded** client (a node running inside your process) can only reach its own node -
`node_id` must be its own id, and the calls go through the same implementation the endpoints use,
without HTTP.

The `ElectionState` type is re-exported at the crate root, so you can name it in your code.

## 6. Membership control: promote, demote, hand the leadership over

The election markers decide _who may lead_. The membership endpoints decide _who is in the voter
set_, and they are what turns a leaner node into the master. They act on **every** Raft group of the
node in the same way, so sqlite and cache never drift apart.

| Method + Path                   | Body                                           | What it does                                     |
| ------------------------------- | ---------------------------------------------- | ------------------------------------------------ |
| `GET /cluster/membership`       | -                                              | voters, learners, leader and state of each group |
| `POST /cluster/promote`         | `{"node_id": <id>}`                            | learner becomes a voter in every group           |
| `DELETE /cluster/membership`    | `{"node_id": <id>, "stay_as_learner": <bool>}` | voter drops to learner, or leaves entirely       |
| `POST /cluster/transfer_leader` | `{"node_id": <id>}`                            | this node hands its leadership to that node      |

```bash
# the same bearer token as in section 4
AUTH="Authorization: Bearer $SECRET"

# who leads what, and who is allowed to vote
curl -s -H "$AUTH" -H "Accept: application/json" \
  http://127.0.0.1:31001/cluster/membership

# the leaner node (id 3) becomes a full voting member
curl -s -X POST -H "$AUTH" -H "content-type: application/json" \
  -d '{"node_id":3}' http://127.0.0.1:31001/cluster/promote

# the old leader (id 1) drops to a learner - this is the switchover
curl -s -X DELETE -H "$AUTH" -H "content-type: application/json" \
  -d '{"node_id":1,"stay_as_learner":true}' http://127.0.0.1:31001/cluster/membership

# hand the leadership to node 2, keep every voter
curl -s -X POST -H "$AUTH" -H "content-type: application/json" \
  -d '{"node_id":2}' http://127.0.0.1:31001/cluster/transfer_leader
```

The answer of every call is the full membership of the node that ran it, in the same shape as the
`GET`:

```json
{
    "sqlite": {
        "node_id": 1,
        "state": "Learner",
        "leader": 2,
        "voters": [2, 3],
        "learners": [1],
        "is_voter": false,
        "is_learner": true
    },
    "cache": { "...": "same shape" }
}
```

`node_id`, `state`, `is_voter` and `is_learner` in an answer describe the node that answered it;
`voters`, `learners` and `leader` describe the Raft group. Read them from the node you care about:
`Client::cluster_membership()` gives you one entry per node.

`GET` answers on any node. `POST` / `DELETE` / `PUT` run on the **leader** of the cluster: a
follower answers with the redirect payload that names the leader, exactly like the write path does,
so a client follows it. The typed routes with a `{raft_type}` (`POST /cluster/membership/sqlite` for
a full voter set, `DELETE /cluster/membership/sqlite` for a node leaving) are unchanged, and they
stay the tool for repairing a half-applied change.

The typed helpers on `hiqlite::Client`:

```rust
let members = client.cluster_membership().await?; // BTreeMap<NodeId, MembersState>
client.promote_node(3).await?;                    // learner -> voter
client.demote_node(1, true).await?;               // voter -> learner, keeps replicating
client.transfer_leadership(2).await?;             // node 2 becomes the leader
```

`MembersState` and `NodeMembers` are re-exported at the crate root. A remote client sends the three
calls to the leader address it knows and follows a redirect, an embedded client runs them on its own
node and fails with the redirect payload when its node is not the leader.

Three things are worth knowing before you use them:

- **A transfer is a request.** The target takes it only if it can - a node whose log is behind
  ignores it, and a learner can never accept it, so `transfer_leader` refuses a learner outright.
  The answer always tells you where the leadership ended up; if it did not move, the answer shows
  the old leader and the node that asked logs a `warn`.
- **Demoting the leader is the switchover, and a leader cannot demote itself to a learner.**
  `DELETE` with `stay_as_learner: false` on the current leader takes it out of the cluster, and
  openraft's step-down hands the leadership to the most up-to-date remaining voter: no election
  timeout, and the leader grants the target its vote, the same target rule as the shutdown handover
  (section 7). `stay_as_learner: true` on
  the leader is refused, because a leader that stays in the membership as a learner keeps leading,
  and openraft cannot complete the two-step change in that shape. Dropping a leader to a learner is
  what a master-slave flip wants, so do it in the order the playbooks show: `transfer_leader` to
  the node that shall lead, then `DELETE` from that node.
- **The promote window has no redundancy.** 2 voters need 2 of 2 acks: in the window between
  `promote` and `demote`, one node down means no leader and no commits. Keep that window as short as
  your runbook allows, and do it while both nodes are healthy.

## 7. How the handover picks its target

A graceful shutdown of the leader hands leadership over before it leaves, so the shutdown does not
cost the cluster an election. The order is openraft's own rule for a leader that is removed from the
membership - the voter with the greatest matching log first, ties to the higher node id - and
Hiqlite appends the marked nodes at the very end:

```
[ best matching LAN voter, ..., marked nodes last ]
```

A leadership transfer is a request, not a command: a target that is unreachable, or whose log is
behind, silently ignores it. The handover therefore walks the list and gives each candidate a slice
of the remaining budget, moving to the next one when the current one did not take over. When the
list is exhausted the node shuts down without the handover and the cluster elects a leader normally.

Practical consequences:

- a planned shutdown of the LAN leader moves the leadership to the LAN follower, not to the geo node
- if the LAN region has one node left, that node has only the marked node to hand over to, and the
  transfer reaches it - this is the designed fallback
- a single node has nobody to hand over to and shuts down exactly like before
- `Client::shutdown()` sleeps 9.5 s first (Kubernetes rolling-release smoothing) and is bounded by a
  20 s timeout, so a rolling restart is graceful by design

## 8. What to watch out for

1. **A full outage of the LAN region does not self-heal.** With 3 voters, the quorum is 2 of 3. If
   both LAN nodes go down, the geo node is alone, cannot reach a quorum, and cannot become leader -
   with or without the marker. It has no votes to collect. It applies the committed entries it
   already has and waits. The cluster has a leader again as soon as one LAN node is back. Plan a
   failover that survives a region loss as a planned action: promote the geo node **before** the
   outage, not during it (section 9).
2. **The marker is config, not cluster state.** It is not replicated through Raft. If the leader's
   node list does not carry the marker, the leader will hand leadership to the far node. Ship one
   source of truth for the node list (one ConfigMap, one env file) and diff it before you apply it.
3. **A runtime change is not persisted.** `PUT` changes the in-memory value; the restart applies the
   node line. A node that was switched on for a failover and then restarted silently goes back to
   its config value. Alert on `enable_elect != election_disable` if the drift matters to you.
4. **A campaign is a request, not a command.** `POST /cluster/election` runs openraft's Pre-Vote
   round, and a healthy cluster refuses it: the term is untouched when the node cannot win, a live
   leader holding its lease is refused, and a node with `enable_elect` off is refused by its own
   gate. There is no bypass, and that is deliberate - a campaign past that gate needs only its own
   vote plus one peer's grant to be a quorum of three, so it can install a leader the operator has
   declared must never lead. A leader change that must happen is
   `POST /cluster/transfer_leader`, asked of the node that holds the seat.
5. **Failover time is the election timeout, not the API call.** `election_timeout_min` is 1500 ms
   and `election_timeout_max` is 3000 ms by default, the leader lease adds on top. A node that is
   enabled for elections takes over within a few seconds, not instantly.
6. **Learners are not part of this.** A learner never wins, cannot promote itself, and `POST`
   refuses it. Use the `learner_only` config for read replicas and for re-adding a node you wiped.
7. **Keep the geo node's latency out of the tuning knobs.** Defaults (heartbeat 500 ms, election
   1500-3000 ms, snapshot install timeout 10 s) work for a geo voter. If you shorten the election
   timeouts, you get unnecessary leader switches over a far link; the geo node also needs the
   snapshot transfer to fit into `install_snapshot_timeout`.
8. **The marker does not change what the geo node stores.** Its WAL, snapshots and state machine
   are identical to any other voter's, so disk sizing, backup and restore behave the same
   everywhere.
9. **A leader cannot demote itself to a learner.** It keeps leading in that shape and openraft
   cannot finish the change, so `DELETE` with `stay_as_learner: true` on the current leader is
   refused. Hand the leadership to the node that shall lead, then demote from that node (section 6,
   and the master-slave playbook in section 9).

## 9. Playbooks

### Planned shutdown of the LAN leader

Nothing to do. `client.shutdown()` hands leadership to the LAN follower; the geo node stays follower
and keeps its quorum role.

### Hot switchover of the leadership to the geo node

1. Check the geo node is caught up: `GET /cluster/metrics/sqlite` on it, and
   `client.wait_for_cluster_sync()` from an embedded client.
2. Enable its elections: `PUT /cluster/election {"enable_elect":true}` (client:
   `set_node_election(3, true)`).
3. Graceful shutdown of the current LAN leader. Its only remaining candidates are the other LAN node
   and the geo node, in that order, so this is the step that moves the leadership: shut down both
   LAN nodes one after the other, and the second handover lands on the geo node.
4. If the LAN nodes are already gone, make the geo node campaign: `POST /cluster/election`. Step 2
   is what makes it possible at all: a node whose elections are disabled is refused by its own gate,
   and there is no bypass.
5. Decide the new steady state: keep the geo node enabled for elections (it is now the leader you
   want), and switch the LAN nodes back on with their markers, so they follow it.

The write path moves into the geo region after a switchover, by definition. Every commit now needs
the geo node.

### Master-slave: make the leaner node the master

The shape is 1 voter plus 1 leaner node that replicates everything and never votes. Flip it:

1. Check the leaner node is caught up: `GET /cluster/metrics/sqlite` on it, and
   `client.wait_for_cluster_sync()` from an embedded client.
2. `promote_node(2)`: 2 voters, quorum 2 of 2. Both must ack from here on, so this window is the
   fragile one - do it while both nodes are healthy, and move on to step 3 immediately.
3. `transfer_leadership(2)` from the current leader. This is optional and it is the step that names
   the target: node 1 steps down and node 2 leads. The leader grants the target its own vote, so it
   costs one term increment and no election timeout.
4. `demote_node(1, true)` on the leader (node 2). Node 1 is a follower by now, so this is the clean
   demotion: it drops to a learner, keeps replicating, and the voter set is `{2}` again. Running it
   on node 1 itself is refused on purpose - a leader that keeps a Learner entry keeps leading.
5. If you skipped step 3, `demote_node(1, false)` on node 1 is the switchover in one call: node 1
   leaves the cluster and openraft's step-down hands the leadership to node 2. Bring node 1 back as
   a leaner with `POST /cluster/add_learner/{raft_type}` and
   `POST /cluster/become_member/{raft_type}` on it: with an intact data dir it catches up.
6. Check the shape on every node: `cluster_membership()` must show `voters: [2]`, `learners: [1]`,
   `leader: 2` in both groups.

The leaner node keeps its data dir, so the demoted node re-joins as a learner by itself, and the
flip back is the same three calls with the ids swapped. If you wipe a node, start it with
`learner_only` and promote it - do not put it straight into the voter set.

### Split-brain guard for a 2-voter window

A `{1,2}` window with 2 of 2 as the quorum cannot lose either node. Keep the old voter in the
cluster until the new one leads and commits, and never script the promote without the demote: a
cluster that stays at 2 voters with a leaner-shaped runbook is a cluster where one outage is a
total outage.

### Region loss

- One LAN node lost: the cluster keeps committing with the geo node in the quorum. Latency rises to
  the geo RTT for the duration. Nothing to do.
- Both LAN nodes lost: no quorum, no leader (watch-out 1). Restore one LAN node.
- Geo node lost: nothing changes in the write path, the LAN pair keeps the quorum. Re-add it, and
  keep the marker in its node line.

### Replace a node

Wipe it, start it with `learner_only` (`HQL_LEARNER_ONLY=true`), let it sync, promote it to a voter,
and put its node line - marker included - into every node's list. With
`raft_config.allow_log_reversion = Some(true)` (the v0.17 default) a node whose volume was wiped
re-syncs under the same leader by itself.

### Kubernetes

One ConfigMap holds the node list; `node_id_from = "k8s"` gives each pod its `node_id` from its
hostname. The marked node is one line in that ConfigMap, so a hot
switchover changes two things only: the ConfigMap for the marker (a restart) and the runtime `PUT`
(no restart). Use the `shutdown-handle` feature so SIGTERM runs the graceful shutdown, and keep the
termination grace period above the 9.5 s pre-sleep plus the handover time.

## 10. Monitoring

Worth an alert:

- `leader` of a group points at a marked node while you did not plan a failover
- `enable_elect != election_disable` on a node, for longer than your change window (section 4)
- a `current_term` jump on a node that was not involved in a planned switchover (a leader lease
  expired and a campaign ran)
- a marked node that is `Leader` for longer than expected and your clients sit in the LAN region
- a node whose `state` is `Learner` in a group where it should be a voter
- `voters` of a group that is not your planned shape - `GET /cluster/membership` on every node
- a leadership transfer that did not land: the answer of `POST /cluster/transfer_leader`
  still names the old leader

`GET /cluster/metrics/{sqlite|cache}` gives you the full Raft metrics of a node - `current_leader`,
`current_term`, `replication`, `state` - next to `/cluster/election` for the election state.

