set shell := ["bash", "-uc"]

export TAG := `cat Cargo.toml | grep '^version =' | cut -d " " -f3 | xargs`
export MSRV := `cat Cargo.toml | grep '^rust-version =' | cut -d " " -f3 | xargs`


default:
    @just -l

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
