# Topology

How many nodes to run, what each one is allowed to do, and how leadership moves.

## Cluster size

A write needs a majority of voters. The table is the whole reason to run more nodes.

| Voters | Write needs     | Survives losing | Notes                                                |
| ------ | --------------- | --------------- | ---------------------------------------------------- |
| 1      | the node itself | nothing         | dev, cache, a box you accept losing                  |
| 2      | both            | nothing         | a restart of one node is a write outage              |
| 3      | two             | one node        | the default shape                                    |
| 4      | three           | one node        | costs you, buys nothing over 3                       |
| 5      | three           | two nodes       | when you run regions, or need a bigger safety margin |

Three voters is the shape to start with. Four is the worst option: same fault tolerance as three,
more write latency, more nodes to restart.

Learners do not count in the majority. Two voters plus two learners still commits at two, and
losing a learner costs nothing.

## Voter and learner

A voter replicates the data, serves local reads, votes, can be elected, and is part of the majority.

A learner replicates the data and serves local reads. It does not vote, is not elected, and is not
part of the majority. Use it for a read replica, for a node in a far region you never want to lead,
and for a supervised re-join of a node that lost its data.

- Start a node as a learner: `learner_only = true` / `HQL_LEARNER_ONLY=true`. It does not demote an
  existing voter.
- Promote a learner: `Client::promote_node(node_id)` / `POST /cluster/promote`. The node must
  already be a learner; this only moves it into the voter set.
- Take a voter out of every group: `Client::demote_node(node_id, stay_as_learner)` /
  `DELETE /cluster/membership`. With `stay_as_learner = true` it keeps replicating as a learner,
  which is what a master-slave setup wants. With `false` it leaves the cluster, and leadership is
  handed to the most up-to-date remaining voter - demoting the leader this way _is_ the planned
  switchover.
- Per group: `POST /cluster/add_learner/{raft_type}` adds a learner,
  `POST /cluster/become_member/{raft_type}` makes a learner a voter,
  `DELETE /cluster/membership/{raft_type}` removes a member,
  `GET /cluster/membership/{raft_type}` reads one group, and `POST /cluster/membership/{raft_type}`
  writes a membership you have to repair by hand.

A leader that is still a member cannot demote itself to a learner: it would keep leading a group it
no longer votes in. That call is refused. Hand the leadership over first, then demote the old leader
from the node that took it.

## Read the cluster

```bash
AUTH="Authorization: Bearer $SECRET"

curl -H "$AUTH" http://10.0.0.1:8200/cluster/membership
curl -H "$AUTH" http://10.0.0.1:8200/cluster/metrics/sqlite
curl -H "$AUTH" http://10.0.0.1:8200/cluster/election
```

- `Client::cluster_membership` shows every member of every Raft group of a node.
- `Client::metrics_db` and `Client::metrics_cache` are the full state of a group: the leader, the
  voters, the learners, the log position.
- `Client::cluster_election_state` shows which nodes may nominate themselves.

Cluster control acts on all Raft groups of a node at once: the database group and every cache
group. When you promote, demote, hand over leadership, or change the election state of a node, it
applies to all of them. That is deliberate: a node is one unit, and a split between its groups is a
state nobody wants to debug.

## Leadership

- Which node leads: `Client::is_leader_db`, `Client::is_leader_cache`. A node can lead one group
  and follow in the other.
- Hand leadership to a node you name: `Client::transfer_leadership(node_id)` /
  `POST /cluster/transfer_leader`. This is a handover, not an election: one term, no waiting for a
  timeout, no cluster-wide disruption.
- A handover is refused when the target is a learner, and when the target is behind the rest of the
  cluster. Make it a voter, catch it up, then hand over.
- A graceful shutdown hands leadership away by itself. You never need to transfer leadership before
  restarting a node.

## Elections

- Make a node nominate itself: `Client::elect_node(node_id)` / `POST /cluster/election`. It runs
  openraft's Pre-Vote round, so a healthy cluster refuses it.
- Move the seat deliberately: `Client::transfer_leadership(node_id)` /
  `POST /cluster/transfer_leader`. This is the graceful path and the only one.
- Turn self-nomination on and off per node: `Client::set_node_election(node_id, enable)` /
  `PUT /cluster/election`. This is the runtime switch for the `election_disable` marker.
- The marker from the node line is applied at start. A restart re-applies the node line, so a
  runtime change is not permanent.
- When a leader dies, the cluster picks a new one on its own. Expect a failover to be settled in a
  few seconds; writes that were in flight fail and you retry them.

## Geo-replication

The pattern for a far node: two nodes in one region, a third in another, the far one marked
`election_disable` in every node list. The far node stays a full voter, keeps the majority, accepts
a handover, and never nominates itself. In steady state the two acknowledgements a write needs come
from the local pair, so the far round trip is not in your write path.

Setup, the hot switchover playbook, region loss, and all the watch-outs:
**[Geo Replication Setup](geo-replication.md)**.

The same tools do a planned switchover in one region: promote the target, hand leadership to it,
demote the old leader. `Client::demote_node(leader, false)` is the one-call version of that.

## Changing the cluster

Add a node: give it a data dir of its own, add its line to the node list of every node, start it as
a learner with `learner_only = true`, wait until it is caught up, promote it. Restart each existing
node with the new node list so every leader knows about the new member.

Replace a node: start the replacement as a learner, catch it up, promote it, then demote and stop
the old node.

Move a node to another machine: same shape. Do not copy a data dir between machines.

Remove a node: demote it to a learner, stop it, remove its line from every node, restart the
remaining nodes with the new list.

Node lists are config. A cluster does not discover members on its own, and a leader only knows the
markers of the nodes in its list.

## Rules of thumb

- Never restart a majority at once, in any operation, in any orchestrator.
- Watch the leader before and after every topology change.
- Promote only after a node is caught up, and demote only after the new leader is visible.
- A learner is cheap: it costs disk and replication bandwidth, not quorum.
- Prefer 3 voters in one region over 5 spread across the world.
