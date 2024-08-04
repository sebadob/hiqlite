FROM gcr.io/distroless/cc-debian12:nonroot

WORKDIR /app

COPY out/hiqlite .

ENTRYPOINT ["/app/hiqlite"]

CMD ["serve"]
