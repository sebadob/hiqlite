# Standalone Server and Proxy

Embedding Hiqlite in your own binary is the normal shape. The standalone server exists for the
other shapes: a shared database for services that are not Rust, one entry point in front of an
existing cluster, a quick evaluation, a platform team running the cluster for many apps.

## What the server is

- A Hiqlite node in a binary. Same engine, same files on disk, same cluster behaviour.
- It serves two ports: the Raft port for cluster traffic and the API port for clients, cluster
  control, health, and the dashboard.
- Data changes arrive from clients over the API. There is no SQLite wire protocol.
- On `SIGTERM` and `SIGINT` it shuts down gracefully: the node stops advertising readiness, hands
  leadership to another member, then exits. This is what makes a pod restart smooth.

## Install

```bash
cargo install hiqlite --features server
hiqlite -h
```

For containers, pin an image tag. Do not use `latest`: a tag you cannot roll back to is a tag you
cannot recover with.

## Configure

```bash
hiqlite generate-config        # writes $HOME/.hiqlite/hiqlite.toml
hiqlite generate-config -p     # asks for a dashboard password and prints the stored form
hiqlite generate-proxy-config  # writes $HOME/.hiqlite/hiqlite-proxy.toml
```

The generated file is commented and contains every key a node understands. Env vars override it.
`hiqlite serve --node-id 2` overrides the node id from the config, so one config file can be used
for every node. Keys and defaults: `hiqlite::REFERENCE_CONFIG`, and see `configuration.md`.

Flags:

- `serve`: `--node-id`, `-c / --config-file`, `--log-statements`, `-l / --log-level`
- `proxy`: `-c / --config-file`, `-l / --log-level`
- `generate-config`: `-p / --password`, `--insecure-cookie`

`--insecure-cookie` is for local testing only. It issues dashboard cookies without TLS.

## Run a three-node cluster locally

Three directories, three node ids, one config:

```bash
export HQL_NODES="1 127.0.0.1:8100 127.0.0.1:8200\n2 127.0.0.1:8101 127.0.0.1:8201\n3 127.0.0.1:8102 127.0.0.1:8202"
export HQL_SECRET_RAFT=change-me-raft-secret
export HQL_SECRET_API=change-me-api-secret

HQL_NODE_ID=1 HQL_DATA_DIR=./n1 hiqlite serve &
HQL_NODE_ID=2 HQL_DATA_DIR=./n2 hiqlite serve &
HQL_NODE_ID=3 HQL_DATA_DIR=./n3 hiqlite serve &
```

If the dashboard password is configured, open `http://127.0.0.1:8200` and log in.

## Proxy

The proxy is a single connection point that load-balances a remote client across all cluster
members. Use it when your app instances cannot reach every node: NAT'd deploys, one app network,
a database network you can only reach through a jump host.

What the proxy is:

- A hop in front of the cluster. It speaks the API port of each member only.
- Load balancer and connection multiplexer. Your client opens one connection.
- Stateless: it holds no data, is not a member, has no volume, cannot vote, cannot become a leader.

What the proxy is not:

- Not a cache in front of the database, not a read replica, not a write buffer.
- Not a high-availability component. If a proxy dies, only the clients behind it lose their path.
  Run one proxy per app instance, per pod, per zone, and let clients pick any of them.

Proxy config keys: `listen_addr`, `listen_port`, `nodes` (the same node lines as a node uses; the
Raft address must be present but is not used), `max_stream_connections`, the API TLS keys,
`secrets_file`, `secret_api`. It rejects unknown keys at startup, so node-only keys like
`node_id`, `data_dir`, and Raft tuning must not be in the proxy config. `hiqlite::REFERENCE_CONFIG`
is the node reference; the proxy subset is documented in the file `generate-proxy-config` writes.

Point a client at it and set `with_proxy` to `true`:

```rust, notest
let client = Client::remote(
    vec!["10.0.0.10:8200".to_string()],
    true,
    false,
    "change-me-api-secret".to_string(),
    true,
    None,
    None,
)
.await?;
```

`max_stream_connections` bounds how many clients can keep a long-lived stream open through that
proxy at once. Raise it when you see stream rejections, not before.

## Kubernetes

A 3-node cluster as a StatefulSet. The manifest is complete: copy it, replace the placeholders,
apply it in the order below.

### Node ids from pod names

```toml
node_id_from = "k8s"
```

The node id is derived from the pod name. StatefulSet ordinals start at 0 and Hiqlite node ids
start at 1, so pods `hiqlite-0` ... `hiqlite-2` become nodes 1 ... 3. One config file is shared by
all pods, and `nodes` must list the headless addresses of all of them.

### Namespace and config

```yaml
apiVersion: v1
kind: Namespace
metadata:
    name: hiqlite
---
apiVersion: v1
kind: Secret
metadata:
    name: hiqlite-config
    namespace: hiqlite
type: Opaque
stringData:
    hiqlite.toml: |
        [hiqlite]
        node_id_from = "k8s"
        nodes = [
            "1 hiqlite-0.hiqlite-headless:8100 hiqlite-0.hiqlite-headless:8200",
            "2 hiqlite-1.hiqlite-headless:8100 hiqlite-1.hiqlite-headless:8200",
            "3 hiqlite-2.hiqlite-headless:8100 hiqlite-2.hiqlite-headless:8200",
        ]

        secret_raft = "<raft-secret>"
        secret_api = "<api-secret>"

        enc_keys = [ "<enc-key-id/base64-key>" ]
        enc_key_active = "<enc-key-id>"

        password_dashboard = "<argon2id-hash-from-generate-config>"

        s3_url = "https://s3.example.com"
        s3_bucket = "<bucket>"
        s3_region = "<region>"
        s3_key = "<s3-key-id>"
        s3_secret = "<s3-secret>"
```

`enc_keys` and `enc_key_active` are needed for backup encryption and for the dashboard. Keep old
entries in `enc_keys` as long as you have backups made with them.

If you want the config to be reviewable without secrets inside it, keep the manifest as a ConfigMap
and set every secret-bearing key to `$SECRETS`, then mount a second file at `secrets_file` holding
only the secret values. See `configuration.md`.

The container reads the config from its `$HOME`. Mount the secret at the path the image expects, as
below, or pass `-c /path/to/hiqlite.toml` in the container args.

### Services

```yaml
apiVersion: v1
kind: Service
metadata:
    name: hiqlite
    namespace: hiqlite
spec:
    selector:
        app: hiqlite
    type: NodePort
    ports:
        - name: api
          protocol: TCP
          port: 8200
          targetPort: 8200
---
apiVersion: v1
kind: Service
metadata:
    name: hiqlite-headless
    namespace: hiqlite
spec:
    clusterIP: None
    publishNotReadyAddresses: true
    selector:
        app: hiqlite
    ports:
        - name: raft
          protocol: TCP
          port: 8100
          targetPort: 8100
        - name: api
          protocol: TCP
          port: 8200
          targetPort: 8200
```

`publishNotReadyAddresses: true` on the headless service is not optional. Nodes must resolve each
other while they are not ready, which is exactly the state every node is in during first formation
and during a re-join. Expose the API service only inside your cluster network. The Raft port must
never be reachable from outside.

### StatefulSet

```yaml
apiVersion: policy/v1
kind: PodDisruptionBudget
metadata:
    name: hiqlite
    namespace: hiqlite
spec:
    minReadySeconds: 10
    minAvailable: 2
    selector:
        matchLabels:
            app: hiqlite
---
apiVersion: apps/v1
kind: StatefulSet
metadata:
    name: hiqlite
    namespace: hiqlite
    labels:
        app: hiqlite
spec:
    replicas: 3
    # Restarts one pod at a time, waiting for readiness between them.
    podManagementPolicy: OrderedReady
    updateStrategy:
        # An image tag change rolls one pod at a time, gated by readiness. This is the normal
        # upgrade path. Only a release whose changelog demands a cold restart must not roll; see
        # `upgrades.md`.
        type: RollingUpdate
        rollingUpdate:
            partition: 0
            maxUnavailable: 1
            maxSurge: 0
    selector:
        matchLabels:
            app: hiqlite
    serviceName: hiqlite-headless
    template:
        metadata:
            labels:
                app: hiqlite
        spec:
            # Must cover the graceful shutdown: readiness off, leadership handover, ordered
            # teardown. It needs tens of seconds. SIGKILL at the deadline costs a state rebuild.
            terminationGracePeriodSeconds: 60
            containers:
                - name: hiqlite
                  image: ghcr.io/sebadob/hiqlite:<pinned-tag>
                  securityContext:
                      allowPrivilegeEscalation: false
                  ports:
                      - containerPort: 8100
                      - containerPort: 8200
                  env:
                      - name: HQL_DATA_DIR
                        value: /app/data
                  volumeMounts:
                      - name: hiqlite-config
                        mountPath: /app/hiqlite.toml
                        subPath: hiqlite.toml
                        readOnly: true
                      - name: hiqlite-data
                        mountPath: /app/data
                  startupProbe:
                      httpGet:
                          scheme: HTTP
                          port: 8200
                          path: /ready
                      periodSeconds: 5
                      # A node that must rebuild its state from a snapshot needs this headroom.
                      failureThreshold: 60
                  readinessProbe:
                      httpGet:
                          scheme: HTTP
                          port: 8200
                          path: /ready
                      initialDelaySeconds: 5
                      # Keep this short: the pod must leave the endpoints before it is stopped.
                      periodSeconds: 3
                      # One failure during a leader switch is normal.
                      failureThreshold: 2
                  livenessProbe:
                      httpGet:
                          scheme: HTTP
                          port: 8200
                          path: /health
                      initialDelaySeconds: 60
                      periodSeconds: 30
                      # A leader switch makes a node answer 503 for a short window.
                      failureThreshold: 2
                  resources:
                      requests:
                          memory: 64Mi
                          cpu: 100m
            volumes:
                - name: hiqlite-config
                    secret:
                        secretName: hiqlite-config
    volumeClaimTemplates:
        - metadata:
              name: hiqlite-data
          spec:
              accessModes:
                  - 'ReadWriteOnce'
              resources:
                  requests:
                      storage: 256Mi
              # Prefer local storage. Hiqlite replicates the data itself; a replicated volume
              # adds latency and a second failure domain.
              storageClassName: local-path
```

```bash
kubectl apply -f namespace.yaml
kubectl apply -f config.yaml
kubectl apply -f sts.yaml
```

`maxSurge: 0` is deliberate: a StatefulSet that starts a fourth pod with the same node id cannot
work.

### Rolling restart behaviour

Restart pods one at a time. Never in parallel: a restart takes a member out of the cluster, and a
majority of voters must stay up.

What happens when Kubernetes deletes one pod:

1. The node gets `SIGTERM` and marks itself not ready. `/ready` answers 503 from that moment.
2. The endpoint is removed. Clients stop sending it writes.
3. If it was the leader, leadership moves to another member. Clients follow the new leader
   automatically.
4. The node finishes its ordered teardown and exits, so its data is consistent on restart.

The deliberate pause inside the shutdown is there to make steps 2 and 3 land before the node
leaves, which is what keeps a restart from triggering a fresh election.

Settings that make this smooth, and what each one protects:

- `terminationGracePeriodSeconds: 60`: the graceful shutdown needs tens of seconds. If the grace
  period is shorter, the pod is killed mid-teardown and the node rebuilds its state on the next
  start.
- `podManagementPolicy: OrderedReady` with `maxUnavailable: 1`: one member at a time.
- Readiness `periodSeconds: 3`: removal from the endpoints must happen before the restart starts.
  Lengthening this is the usual cause of write timeouts during a rolling restart.
- Readiness and liveness `failureThreshold: 2`: a leader switch makes a node answer 503 briefly.
  A threshold of 1 restarts a healthy pod for nothing.
- Liveness on `/health`, readiness on `/ready`. Never point liveness at `/ready`: a node that is
  legitimately not ready yet would be killed.
- `startupProbe` with a generous `failureThreshold`: the first start after a full state rebuild
  streams a snapshot and replays the log.
- `PodDisruptionBudget` with `minAvailable: 2`: a node drain cannot take the cluster below a
  majority.
- Spread the pods. Three pods on one host is one failure. Prefer anti-affinity over a bigger CPU
  request.

A restart is safe to interrupt at the pod level: it is serial, and each step ends with a ready
cluster. If a pod is stuck, stop the rollout and fix that pod before continuing.

### Restarting a node that lost its volume

Do not start it as a voter next to a healthy cluster. Start that pod once with
`HQL_LEARNER_ONLY=true`, confirm it is caught up, promote it, then remove the env var.
See `topology.md` and `disaster-recovery.md`.

### Version upgrades

A version change is a rolling restart: change the image tag and the StatefulSet rolls pod by pod,
each step gated by readiness. That is the design goal, no downtime, and it is what this setup is
for. Expect a leadership handover on each pod and a fully available cluster throughout.

The exception is announced, not discovered. A release that cannot be rolled says so in the
`Breaking` section of the changelog. The v0.17 release is one: it replaces the Raft network layer,
so members of an older and a newer version cannot exchange cluster messages at all, and a mixed
cluster cannot replicate.

For a release like that, do not roll it. Set `updateStrategy: type: Recreate`, or scale to zero,
change the image, scale back up. Nodes of that version also check what their reachable members
report on `/version` and refuse to start when one of them is older, with a message telling you to
shut the whole cluster down first. `HQL_DISABLE_COLD_RESTART_GUARD=true` skips that check and is a
testing switch, not an upgrade strategy. See `upgrades.md`.

### Check the cluster

```bash
SECRET="$(kubectl -n hiqlite get secret hiqlite-config -o jsonpath='{.data.secret_api}' | base64 -d)"

kubectl -n hiqlite exec hiqlite-0 -- wget -qO- \
    --header="Authorization: Bearer $SECRET" http://127.0.0.1:8200/cluster/membership

kubectl -n hiqlite exec hiqlite-0 -- wget -qO- http://127.0.0.1:8200/ready
```

`GET /cluster/membership` shows every member of every Raft group on that node. `/ready` answers
200 when the node is part of a healthy group.
