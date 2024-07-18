# Basic Example

This example shows the basic usage of the project.

Most options are hardcoded for simplicity and you only need to provide the `--node-id` during startup to make this
example as simple and easy as possible.

You can run the example either with a single node, or with a 3 node Raft cluster.

Single node:

```
cargo run -- single
```

For the 3 node cluster you need to open 3 terminals and start one node in each of them:

```
cargo run -- server --node-id 1
```

... will start the first node. To keep this example simple, only the node with id `1` will insert data.
The others will just start, sleep, and replicate the data.

The Raft will not be initialized until not all members are online at least once, so start the other
nodes as well:

```
cargo run -- server --node-id 2
```

```
cargo run -- server --node-id 3
```

As soon as node 1 can ping the others, it will initialize the cluster and then start the tests.
