FROM debian:12.6-slim AS builder

RUN mkdir /empty


FROM gcr.io/distroless/cc-debian12:nonroot

WORKDIR /app

COPY --from=builder /empty ./data
COPY out/hiqlite .

ENTRYPOINT ["/app/hiqlite"]

CMD ["serve"]
