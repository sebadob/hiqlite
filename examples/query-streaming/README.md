# Query Streaming

A single Hiqlite node with 2.000 seeded users, and a real **axum** API on `127.0.0.1:8080` whose
endpoints answer with the rows of a streaming query.  Nothing here runs in-process: open the page on
`/` in a browser, or call the endpoints with `curl`, and watch the rows arrive while the query is
still running.

```bash
cd examples/query-streaming
cargo run
```

The node starts on the ports of the repo's `hiqlite.toml`, seeds the table, and prints the commands
below.  Exit with `CTRL + C`, which shuts the API and the node down gracefully.

## The page on `/`

`http://127.0.0.1:8080/` is a single plain HTML file, `static/stream.html`, served by the same
router.  It is the JavaScript side of the whole thing: `fetch` hands over the body as a
`ReadableStream`, `getReader()` reads it piece by piece, and every JSON line that arrives becomes a
table row.  The buttons pick an endpoint, the counters show rows, bytes and elapsed time while the
query runs, and "stop reading" aborts the fetch - which stops the query on the server.

The one thing to know about a stream like this: the chunks you read have nothing to do with the
lines of the body.  A chunk can hold half a line, and a line can be spread over several chunks, so
the page keeps the tail of what it has read and cuts a line only at its newline.  Reading the whole
body with `response.json()` would wait for the query to finish and lose the streaming entirely.

## The endpoints

```bash
curl -N http://127.0.0.1:8080/users
curl -N http://127.0.0.1:8080/users/slow
curl -N http://127.0.0.1:8080/users/raw
curl -i  http://127.0.0.1:8080/users/custom
curl -s  http://127.0.0.1:8080/users/mails | head
```

Every answer is one JSON document per line, `content-type: application/x-ndjson`, written while the
query runs.

- `/users` - all 2.000 rows through `query_map_stream` and `stream.into_json_response()`: the whole
  result with a single call, and its memory stays a single row.
- `/users/slow` - the same query with `hiqlite::StreamExt` combinators on it: the email is
  upper-cased in flight, `take(40)` stops the scan after 40 rows, and a 250 ms pause per row makes
  one line arrive after the other.
- `/users/raw` - `query_raw_stream` rows, keyed by the column names of the query, 250 ms apart: an
  `RowOwned` serializes itself with exactly its own header.
- `/users/custom` - `stream.into_json_body()`: the very same streaming body behind a
  `206 Partial Content` and two headers of the endpoint's own.
- `/users/mails` - rows mapped into another type on the way out, answered with `into_json_response`
  from `hiqlite::JsonLinesExt`, because a combinator chain is not a stream anymore.

## Watching the streaming

`-N` keeps `curl` from buffering, which is what makes the streaming visible:

```
$ curl -N http://127.0.0.1:8080/users/slow
{"id":1,"email":"USER1@EXAMPLE.COM","name":"User 1","role":"member","logins":1}
{"id":2,"email":"USER2@EXAMPLE.COM","name":"User 2","role":"member","logins":2}
...one line every 250 ms...
```

The whole result is fast, and it is streaming on the server side as well.  All 2.000 rows of
`/users`, 170 KB, are out in about 15 ms on a dev machine:

```
$ time curl -s http://127.0.0.1:8080/users | wc -l
    2000
```

A client that stops reading stops the query: the rows travel one at a time, and a stream whose
consumer is gone ends its query and hands its connection back to the pool.  A query that fails
mid-scan ends the response - the status line and the headers have gone already - and the body simply
stops, so a truncated result never reads as a complete one.

Holding a stream is the one way to make this leak: a stream you keep without consuming it parks the
thread that runs its query, and the read pool connection that thread owns, on the very next row.  A
node reads on `read_pool_size` actors (one per core by default), so a handful of streams nobody consumes
starves every other local read.  The header print in `main` drops its stream right after its first
row for exactly that reason.
