set shell := ["bash", "-uc"]

export TAG := `cat hiqlite/Cargo.toml | grep '^version =' | cut -d " " -f3 | xargs`
export MSRV := `cat hiqlite/Cargo.toml | grep '^rust-version =' | cut -d " " -f3 | xargs`
export USER := `echo "$(id -u):$(id -g)"`

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
    cargo clippy -- -D warnings
    cargo minimal-versions check -p hiqlite --features server
    cargo minimal-versions check -p hiqlite-wal

    # update at the end again for following clippy and testing
    cargo update

# checks all combinations of features with clippy
clippy:
    #!/usr/bin/env bash
    set -euxo pipefail
    clear

    cargo clippy
    cargo clippy --no-default-features

    cargo clippy --no-default-features --features sqlite,cast_ints
    cargo clippy --no-default-features --features sqlite,cast_ints_unchecked
    # auto-heal should only apply to sqlite
    cargo clippy --no-default-features --features auto-heal
    cargo clippy --no-default-features --features sqlite,auto-heal
    # backup / s3 should only apply to sqlite
    cargo clippy --no-default-features --features backup
    cargo clippy --no-default-features --features sqlite,backup
    cargo clippy --no-default-features --features sqlite,auto-heal,backup

    cargo clippy --no-default-features --features cache
    cargo clippy --no-default-features --features counters
    cargo clippy --no-default-features --features dlock
    cargo clippy --no-default-features --features listen_notify_local
    cargo clippy --no-default-features --features listen_notify
    cargo clippy --no-default-features --features sqlite,cache

    cargo clippy --no-default-features --features dashboard
    cargo clippy --no-default-features --features shutdown-handle

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

# build and open the docs
docs:
    cargo +nightly doc --all-features --no-deps --open

# runs the full set of tests
test test="":
    #!/usr/bin/env bash
    set -euxo pipefail
    clear
    # we need to run the tests with nightly to not get an error for docs auto cfg
    RUSTFLAGS="--cfg tokio_unstable" cargo +nightly test --features cache,counters,dlock,listen_notify,toml {{ test }}

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

# verifies the MSRV
msrv-verify:
    #!/usr/bin/env bash
    set -euxo pipefail
    cd hiqlite
    cargo msrv verify
    cd ..

    cd hiqlite-macros
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

# publish order: wal, core, macros - remember to update version in hiqlite-macros beforehand
publish-wal: verify-is-clean
    #!/usr/bin/env bash
    set -euxo pipefail
    cargo publish -p hiqlite-wal
    echo "WAL published - now update the version in hiqlite/Cargo.toml and publish-core"

publish-core:
    #!/usr/bin/env bash
    set -euxo pipefail
    cargo publish -p hiqlite
    echo "Core published - now update the version in hiqlite-macros/Cargo.toml and publish-macros"

publish-macros:
    #!/usr/bin/env bash
    set -euxo pipefail
    cargo publish -p hiqlite-macros
