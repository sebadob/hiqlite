set shell := ["bash", "-uc"]

export TAG := `cat Cargo.toml | grep '^version =' | cut -d " " -f3 | xargs`
export MSRV := `cat Cargo.toml | grep '^rust-version =' | cut -d " " -f3 | xargs`
export USER :=  `echo "$(id -u):$(id -g)"`

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


# clippy lint + check with minimal versions from nightly
check:
    #!/usr/bin/env bash
    set -euxo pipefail
    clear
    cargo update
    cargo +nightly clippy -- -D warnings
    cargo minimal-versions check


# checks all combinations of features with clippy
clippy-features:
    #!/usr/bin/env bash
    set -euxo pipefail
    clear

    cargo clippy
    cargo clippy --no-default-features

    cargo clippy --no-default-features --features sqlite
    # auto-heal should only apply to sqlite
    cargo clippy --no-default-features --features auto-heal
    cargo clippy --no-default-features --features sqlite,auto-heal
    # backup / s3 should only apply to sqlite
    cargo clippy --no-default-features --features backup
    cargo clippy --no-default-features --features sqlite,backup
    cargo clippy --no-default-features --features sqlite,auto-heal,backup

    cargo clippy --no-default-features --features cache
    cargo clippy --no-default-features --features sqlite,cache


# runs the full set of tests
test:
    #!/usr/bin/env bash
    set -euxo pipefail
    clear
    cargo test


# builds the code
build:
    #!/usr/bin/env bash
    set -euxo pipefail
    cargo build


# builds the code in --release mode
build-release:
    #!/usr/bin/env bash
    set -euxo pipefail
    cargo build --release


# verifies the MSRV
msrv-verify:
    cargo msrv verify


# find's the new MSRV, if it needs a bump
msrv-find:
    cargo msrv --min {{MSRV}}


# verify thats everything is good
verify: check test build msrv-verify


# makes sure everything is fine
verfiy-is-clean: verify
    #!/usr/bin/env bash
    set -euxo pipefail

    # make sure everything has been committed
    git diff --exit-code

    echo all good


# sets a new git tag and pushes it
release: verfiy-is-clean
    #!/usr/bin/env bash
    set -euxo pipefail

    # make sure git is clean
    git diff --quiet || exit 1

    git tag "v$TAG"
    git push origin "v$TAG"


# dry-run publishing the latest version
publish-dry: verfiy-is-clean
    #!/usr/bin/env bash
    set -euxo pipefail
    cargo publish --dry-run


# publishes the current version to cargo.io
publish: verfiy-is-clean
    #!/usr/bin/env bash
    set -euxo pipefail
    cargo publish
