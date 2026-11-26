set shell := ["bash", "-uc"]

export TAG := `cat hiqlite/Cargo.toml | grep '^version =' | cut -d " " -f3 | xargs`
export MSRV := `cat hiqlite/Cargo.toml | grep '^rust-version =' | cut -d " " -f3 | xargs`
export USER := `echo "$(id -u):$(id -g)"`

# Allows overwriting a dependency version if we pin it specifically.
export CARGO_RESOLVER_INCOMPATIBLE_PUBLISH_AGE := 'allow'

[private]
default:
    @just -l

# Creates a new Root + Intermediate CA for development and testing TLS certificates
create-root-ca:
    # Password for both root and intermediate dev CA is always: 123SuperMegaSafe

    mkdir -p tls/ca
    chmod 0766 tls/ca

    # Root CA
    docker run --rm -it -v ./tls/ca:/ca -u $USER \
          ghcr.io/sebadob/nioca \
          x509 \
          --stage root \
          --clean

    # Intermediate CA
    docker run --rm -it -v ./tls/ca:/ca -u $USER \
          ghcr.io/sebadob/nioca \
          x509 \
          --stage intermediate

    cp tls/ca/x509/intermediate/ca-chain.pem tls/ca-chain.pem

# Create a new End Entity TLS certificate for development and testing

# Intermediate CA DEV password: 123SuperMegaSafe
create-end-entity-tls:
    # create the new certificate
    docker run --rm -it -v ./tls/ca:/ca -u $USER \
          ghcr.io/sebadob/nioca \
          x509 \
          --cn 'localhost' \
          --alt-name-dns 'localhost' \
          --alt-name-dns 'hiqlite.local' \
          --alt-name-ip '127.0.0.1' \
          --usages-ext server-auth \
          --usages-ext client-auth \
          --o 'Hiqlite DEV Certificate' \
          --stage end-entity

    # copy it in the correct place
    cp tls/ca/x509/end_entity/$(cat tls/ca/x509/end_entity/serial)/cert-chain.pem tls/cert-chain.pem
    cp tls/ca/x509/end_entity/$(cat tls/ca/x509/end_entity/serial)/key.pem tls/key.pem

# prints out the currently set version
version:
    #!/usr/bin/env bash
    echo "v$TAG"

# cleanup the data dir
cleanup:
    rm -rf data/*

# clippy lint + check with minimal versions from nightly
check:
    #!/usr/bin/env bash
    set -euxo pipefail
    clear
    cargo update
    cargo fmt --check
    cargo clippy
    cargo minimal-versions check -p hiqlite --features server
    cargo minimal-versions check -p hiqlite --no-default-features --features external-state-machine
    cargo minimal-versions check -p hiqlite-wal

    # update at the end again for following clippy and testing
    cargo update

# checks all combinations of features with clippy
clippy:
    #!/usr/bin/env bash
    set -euxo pipefail
    clear

    # base features, each standalone: sqlite here, cache and external-state-machine below
    cargo clippy --no-default-features --features sqlite,aws_lc_rs

    cargo clippy --no-default-features --features sqlite,aws_lc_rs,cast_ints
    cargo clippy --no-default-features --features sqlite,aws_lc_rs,cast_ints_unchecked
    # auto-heal needs one of the base features - cover both
    cargo clippy --no-default-features --features cache,auto-heal,aws_lc_rs
    cargo clippy --no-default-features --features sqlite,auto-heal,aws_lc_rs
    # backup / s3 should only apply to sqlite
    cargo clippy --no-default-features --features backup,aws_lc_rs
    cargo clippy --no-default-features --features sqlite,backup,aws_lc_rs
    cargo clippy --no-default-features --features sqlite,auto-heal,backup,aws_lc_rs

    cargo clippy --no-default-features --features cache,aws_lc_rs
    cargo clippy --no-default-features --features cache,toml,aws_lc_rs
    cargo clippy --no-default-features --features in-memory-snapshots,aws_lc_rs
    cargo clippy --no-default-features --features counters,aws_lc_rs
    cargo clippy --no-default-features --features dlock,aws_lc_rs
    cargo clippy --no-default-features --features listen_notify,aws_lc_rs
    cargo clippy --no-default-features --features sqlite,cache,webpki-roots,aws_lc_rs

    cargo clippy --no-default-features --features dashboard,aws_lc_rs
    # shutdown-handle needs one of the base features - cover both
    cargo clippy --no-default-features --features cache,shutdown-handle,aws_lc_rs
    cargo clippy --no-default-features --features sqlite,shutdown-handle,aws_lc_rs

    # external-state-machine must build standalone without pulling in the
    # internal `__cluster` boundary, and must not break any regular combination
    cargo clippy --no-default-features --features external-state-machine
    cargo clippy --no-default-features --features sqlite,external-state-machine,aws_lc_rs
    cargo clippy --no-default-features --features full,external-state-machine
    cargo clippy --features external-state-machine

    cargo clippy --no-default-features --features server

    # the rustls crypto backends: ring standalone, and the aws-lc-rs s3 path
    cargo clippy --no-default-features --features sqlite,ring
    cargo clippy --no-default-features --features sqlite,s3,aws_lc_rs

    # opt-in features of the WAL crate, including its tests
    cargo clippy -p hiqlite-wal --features oversized-entry-error -- -D warnings
    cargo check -p hiqlite-wal --tests --features oversized-entry-error
    cargo clippy --no-default-features --features sqlite,oversized-entry-error,aws_lc_rs -- -D warnings

# the aws-lc-rs FIPS backend; needs a C toolchain + cmake + libclang to build aws-lc-fips-sys
clippy-fips:
    #!/usr/bin/env bash
    set -euxo pipefail
    clear
    cargo clippy --no-default-features --features sqlite,fips

clippy-examples:
    #!/usr/bin/env bash
    set -euxo pipefail
    clear

    cd examples
    for example in */; do
      cd $example
      cargo clippy
      cd ..
    done
    cd ..

# build and open the docs - the same feature set docs.rs uses (see [package.metadata.docs.rs])
docs:
    #!/usr/bin/env bash
    set -euxo pipefail
    clear

    # `doc_cfg` is a nightly feature, so this crate documents itself with nightly only.
    # `--all-features` is not an option: `cast_ints` and `cast_ints_unchecked` exclude each other.
    cargo +nightly doc --no-deps --open --features full,external-state-machine

# builds the docs and fails on any rustdoc warning: broken links, links to private items, ...
docs-check:
    #!/usr/bin/env bash
    set -euxo pipefail
    clear

    RUSTDOCFLAGS="-D warnings" cargo +nightly doc --no-deps --features full,external-state-machine

# runs the full set of tests
test test="":
    #!/usr/bin/env bash
    set -euxo pipefail
    clear
    # The v0.17 cold restart guard is off for tests. It checks other nodes on startup, and our
    # tests spin up whole clusters inside a single process all the time.
    HQL_DISABLE_COLD_RESTART_GUARD="true" cargo test --features full,counters,external-state-machine,server {{ test }}

# runs the full set of tests excluding backup to S3 tests
test-no-s3:
    #!/usr/bin/env bash
    set -euxo pipefail
    clear
    TEST_SKIP_S3_RESTORE="true" HQL_DISABLE_COLD_RESTART_GUARD="true" cargo test --features cache,counters,dlock,listen_notify,macros,toml,external-state-machine

# builds the code
build ty="server":
    #!/usr/bin/env bash
    set -euxo pipefail

    if [[ {{ ty }} == "server" ]]; then
          cargo build
    elif [[ {{ ty }} == "ui" ]]; then
      rm -rf hiqlite/static
      cd dashboard
      rm -rf build
      npm run build
      git add ../hiqlite/static
    fi

# builds a container image
build-image name="ghcr.io/sebadob/hiqlite":
    #!/usr/bin/env bash
    set -euxo pipefail

    rm -rf hiqlite/static
    cd dashboard
    npm run build
    cd ..
    git add hiqlite/static

    #cargo build --features server --release
    #mkdir -p out
    #cp target/release/hiqlite out/

    docker build -t {{ name }}:{{ TAG }} .
    docker push {{ name }}:{{ TAG }}
    docker tag {{ name }}:{{ TAG }} {{ name }}:latest
    docker push {{ name }}:latest

# builds the code in --release mode
build-release:
    #!/usr/bin/env bash
    set -euxo pipefail
    cargo build --release

run ty="server" node_id="1":
    #!/usr/bin/env bash
    set -euxo pipefail
    clear

    if [[ {{ ty }} == "server" ]]; then
      HQL_DATA_DIR=data/server_{{ node_id }} cargo run --features server,backup -- serve -c hiqlite.toml --node-id {{ node_id }}
    elif [[ {{ ty }} == "ui" ]]; then
      cd dashboard
      npm run dev -- --host=0.0.0.0
    fi

test-migrate:
    #!/usr/bin/env bash
    set -euxo pipefail
    clear
    cp -r data/logs_bkp/* data/server_1/logs/
    HQL_DATA_DIR=data/server_1 cargo run --features server -- serve -c hiqlite.env --node-id 1

# runs the connection pool bench in a fixed core envelope so runs are comparable
bench-conn-pool *args:
    #!/usr/bin/env bash
    set -euxo pipefail
    # macOS has no thread affinity (`thread_policy_set` answers `KERN_NOT_SUPPORTED` on Apple
    # Silicon), so the envelope is the `utility` QoS clamp, which the scheduler uses to keep
    # threads on performance cores.  Linux pins properly with `taskset`.
    case "$(uname -s)" in
        Darwin)
            WRAP=( /usr/sbin/taskpolicy -c utility )
            ENVELOPE="taskpolicy -c utility - $(sysctl -n hw.perflevel0.logicalcpu) P + $(sysctl -n hw.perflevel1.logicalcpu) E cores"
            ;;
        Linux)
            LAST=$(( $(nproc) - 1 ))
            WRAP=( taskset -c "0-${LAST}" )
            ENVELOPE="taskset -c 0-${LAST} - $(nproc) cpus"
            ;;
        *)
            WRAP=()
            ENVELOPE="none"
            ;;
    esac
    echo "core envelope: ${ENVELOPE}"
    ${WRAP[@]+"${WRAP[@]}"} cargo bench --features sqlite --bench conn_pool -- {{args}}

# verifies the MSRV
msrv-verify:
    #!/usr/bin/env bash
    set -euxo pipefail
    cd hiqlite
    cargo msrv verify
    cd ..

    cd hiqlite-derive
    cargo msrv verify
    cd ..

    cd hiqlite-wal
    cargo msrv verify

# find's the new MSRV, if it needs a bump
msrv-find:
    cargo msrv find --min {{ MSRV }} --all-features

# verify thats everything is good
verify:
    # we don't want to rebuild the UI each time because it's checked into git
    #just build ui
    just check
    just clippy
    just clippy-examples
    just docs-check
    just test
    just msrv-verify

# makes sure everything is fine
verify-is-clean: verify
    #!/usr/bin/env bash
    set -euxo pipefail

    # make sure everything has been committed
    git diff --exit-code

    echo all good

# sets a new git tag and pushes it
release:
    #!/usr/bin/env bash
    set -euxo pipefail

    # make sure git is clean
    git diff --quiet || exit 1

    git tag "v$TAG"
    git push origin "v$TAG"

    just build-image

# publish order: wal, derive, core - remember to update version in hiqlite-macros beforehand
publish-wal: verify-is-clean
    #!/usr/bin/env bash
    set -euxo pipefail
    cargo publish -p hiqlite-wal
    echo "WAL published - now update the version in hiqlite/Cargo.toml and publish-derive"

publish-derive:
    #!/usr/bin/env bash
    set -euxo pipefail
    cargo publish -p hiqlite-derive
    echo "Derive published - now update the version in hiqlite/Cargo.toml and publish-core"

publish-core:
    #!/usr/bin/env bash
    set -euxo pipefail
    cargo publish -p hiqlite

# does a `cargo update` + `npm update` for the UI
update:
    #!/usr/bin/env bash

    # We need at least nightly-2026-06-21 for the min release age feature
    # from .cargo/config.toml
    MIN_DATE="2026-06-21"
    NIGHTLY_DATE=$(rustc +nightly --version | grep -oE '[0-9]{4}-[0-9]{2}-[0-9]{2}' | head -n 1)
    if [[ -z "$NIGHTLY_DATE" || "$NIGHTLY_DATE" < "$MIN_DATE" ]]; then
        echo "Error: The nightly toolchain must be at least $MIN_DATE. (Found: ${NIGHTLY_DATE:-unknown})"
        exit 1
    fi
    cargo +nightly update

    cd dashboard
    # min release is set via `dashboard/.npmrc`
    npm update
