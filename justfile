set shell := ["bash", "-uc"]

export TAG := `cat Cargo.toml | grep '^version =' | cut -d " " -f3 | xargs`
export MSRV := `cat hiqlite/Cargo.toml | grep '^rust-version =' | cut -d " " -f3 | xargs`
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
    cargo minimal-versions check -p hiqlite --all-features


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
    cargo clippy --no-default-features --features dlock
    cargo clippy --no-default-features --features listen_notify
    cargo clippy --no-default-features --features sqlite,cache

    cargo clippy --no-default-features --features dashboard
    cargo clippy --no-default-features --features shutdown-handle


# build and open the docs
docs:
    cargo +nightly doc --all-features --no-deps --open


# runs the full set of tests
test:
    #!/usr/bin/env bash
    set -euxo pipefail
    clear
    # we need to run the tests with nightly to not get an error for docs auto cfg
    cargo +nightly test --features cache,dlock,listen_notify


# builds the code
build ty="server":
    #!/usr/bin/env bash
    set -euxo pipefail

    if [[ {{ty}} == "server" ]]; then
          cargo build
    elif [[ {{ty}} == "ui" ]]; then
      cd dashboard
      rm -rf build
      npm run build
      git add ../hiqlite/static
    fi


# builds a container image
build-image name="cr.sebastiandobe.de/hiqlite/hiqlite":
    #!/usr/bin/env bash
    set -euxo pipefail

    #cd dashboard
    #npm run build
    #cd ..
    #git add hiqlite/static

    cargo build --features server --release
    mkdir -p out
    cp target/release/hiqlite out/

    docker build -t {{name}} .
    docker push {{name}}


# builds the code in --release mode
build-release:
    #!/usr/bin/env bash
    set -euxo pipefail
    cargo build --release


run ty="server":
    #!/usr/bin/env bash
    set -euxo pipefail
    clear

    if [[ {{ty}} == "server" ]]; then
      cargo run --features server -- serve
    elif [[ {{ty}} == "ui" ]]; then
      cd dashboard
      npm run dev -- --host=0.0.0.0
    fi


# verifies the MSRV
msrv-verify:
    #!/usr/bin/env bash
    set -euxo pipefail
    cd hiqlite
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


# publishes the current lib version to cargo.io
publish: verfiy-is-clean
    #!/usr/bin/env bash
    set -euxo pipefail
    cargo publish -p hiqlite
