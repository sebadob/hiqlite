FROM rust:1.81.0-bookworm AS builder

ENV DEBIAN_FRONTEND=noninteractive

# prepare an empty dir upfront to set proper access rights
RUN mkdir /empty

WORKDIR /work

COPY . .

# TODO create a builder base image
RUN apt update && apt install -y clang

RUN cargo build --features server --release


FROM gcr.io/distroless/cc-debian12:nonroot

WORKDIR /app

COPY --from=builder /empty ./data
COPY --from=builder /work/target/release/hiqlite ./hiqlite

ENTRYPOINT ["/app/hiqlite"]

CMD ["serve"]
