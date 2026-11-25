use crate::Error;
use axum::BoxError;
use axum::body::Body;
use axum::http::HeaderValue;
use axum::http::header::CONTENT_TYPE;
use axum::response::Response;
use bytes::Bytes;
use flume::r#async::RecvStream;
use futures_util::TryStream;
use futures_util::TryStreamExt as _;
use futures_util::stream::StreamExt as _;
use serde::Serialize;
use std::future::poll_fn;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Rows a streaming query keeps buffered between the thread running the query and its consumer:
/// none.  A capacity of `0` is a rendezvous channel, so the thread running the query hands a row
/// straight into the consumer's `next` and only then reads the following one: a stream never reads
/// a row nobody has asked for, and it parks on exactly that row.
pub(crate) const PREFETCH: usize = 0;

/// The media type of [`JsonLinesExt::into_json_response`]: one JSON document per line.
const NDJSON: HeaderValue = HeaderValue::from_static("application/x-ndjson");

/// How the actor running a streaming query left its scan, answered on the verdict channel of
/// [`QueryStream`] when the scan ends.
#[derive(Debug)]
pub(crate) enum StreamVerdict {
    /// The cursor reached its last row.
    Done,
    /// The scan left the loop on a failed cursor, on a row mapping that returned an `Err`, or on
    /// the panic the actor caught around the mapping.
    Err(Error),
}

/// The rows of a query, produced while you consume them, as returned by
/// [`crate::Client::query_map_stream`], [`crate::Client::query_try_map_stream`] and
/// [`crate::Client::query_raw_stream`].
///
/// A `QueryStream` is the async iterator over one query's result set: [`QueryStream::next`] hands
/// you one row at a time with nothing to import, and it implements `futures_util::stream::Stream`,
/// so the combinators of [`crate::StreamExt`] work on it.
///
/// Its rows arrive as you consume them: the query holds a single row at a time, so a result set of
/// a million rows costs the memory of one of them. Dropping a `QueryStream` stops its query and
/// returns its connection to the read pool, so a stream you stop consuming early never keeps
/// scanning the table.
///
/// The other side of that is worth knowing: a stream you hold on to without consuming parks the
/// read-pool connection that runs its query, on the very next row - and it stays parked for as long
/// as you hold the stream. A node answers local reads from `read_pool_size` connections (one per
/// core by default), so a few streams nobody consumes starves every other read on that node.
/// Consume the rows you need, and drop the stream.
///
/// # Panics
///
/// A row whose data does not match the mapped type panics on the thread running the query, exactly
/// like [`crate::Client::query_map`] does.
#[derive(Debug)]
#[must_use]
pub struct QueryStream<T: 'static> {
    rx: RecvStream<'static, Result<T, Error>>,
    verdict: RecvStream<'static, StreamVerdict>,
    finished: bool,
}

impl<T> QueryStream<T> {
    pub(crate) fn new(
        rx: flume::Receiver<Result<T, Error>>,
        verdict: flume::Receiver<StreamVerdict>,
    ) -> Self {
        Self {
            rx: rx.into_stream(),
            verdict: verdict.into_stream(),
            finished: false,
        }
    }

    /// The next row of the query, `None` after its last one.
    ///
    /// A query that fails while you consume it is the `Err` of this call, so a `?` on it carries a
    /// truncated result out of your loop instead of ending that loop like the last row did.
    ///
    /// # Errors
    ///
    /// The [`Error::Sqlite`] of a cursor that cannot step on, and of a row mapping that returns an
    /// `Err`, and the [`Error::Error`] of the row mapping that panicked on the thread running the
    /// query.
    ///
    /// # Panics
    ///
    /// On the thread running the query, if a row's data does not match the mapped type; the panic
    /// ends the stream, and this call answers with its `Error`.
    pub async fn next(&mut self) -> Result<Option<T>, Error> {
        match poll_fn(|cx| futures_util::Stream::poll_next(Pin::new(&mut *self), cx)).await {
            Some(res) => res.map(Some),
            None => Ok(None),
        }
    }
}

impl<T: Serialize + Send> QueryStream<T> {
    /// This stream as an API response that answers with one JSON document per line, the rows in
    /// result-set order and nothing collected in memory.
    ///
    /// ```rust, ignore
    /// let stream = client
    ///     .query_map_stream::<MyStruct, _>("SELECT * FROM users", params!())
    ///     .await?;
    ///
    /// Ok::<_, hiqlite::Error>(stream.into_json_response())
    /// ```
    ///
    /// The shape of that body, what a mid-query failure does to it, and what a mapped type needs to
    /// be one are the docs of [`JsonLinesExt::into_json_response`], the very same method for a
    /// combinator chain out of [`crate::StreamExt`].
    #[must_use = "the rows of a stream are only read while its response is sent"]
    pub fn into_json_response(self) -> Response {
        lines_response(self)
    }

    /// The rows of this stream as a streaming JSON body, with your own status code and headers
    /// around it: the body of [`JsonLinesExt::into_json_response`] on its own.
    ///
    /// ```rust, ignore
    /// use axum::http::{header::CONTENT_TYPE, StatusCode};
    ///
    /// let stream = client
    ///     .query_raw_stream("SELECT * FROM users LIMIT 100", params!())
    ///     .await?;
    ///
    /// let response = Response::builder()
    ///     .status(StatusCode::PARTIAL_CONTENT)
    ///     .header(CONTENT_TYPE, "application/x-ndjson")
    ///     .body(stream.into_json_body())
    ///     .unwrap();
    /// ```
    #[must_use = "the rows of a stream are only read while its body is sent"]
    pub fn into_json_body(self) -> Body {
        lines_body(self)
    }
}

impl<T> futures_util::Stream for QueryStream<T> {
    type Item = Result<T, Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.finished {
            return Poll::Ready(None);
        }

        match Pin::new(&mut self.rx).poll_next(cx) {
            Poll::Ready(Some(item)) => return Poll::Ready(Some(item)),
            Poll::Pending => return Poll::Pending,
            Poll::Ready(None) => {}
        }

        // Every sender is gone, which only tells us the read pool stopped. Ask it whether the query
        // ran to its end, ended on an error, or died on a panic, so a truncated result is never
        // reportable as a complete one.
        match Pin::new(&mut self.verdict).poll_next(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Some(StreamVerdict::Done)) => {
                self.finished = true;
                Poll::Ready(None)
            }
            Poll::Ready(Some(StreamVerdict::Err(err))) => {
                self.finished = true;
                Poll::Ready(Some(Err(err)))
            }
            Poll::Ready(None) => {
                self.finished = true;
                Poll::Ready(Some(Err(Error::Sqlite(
                    "the read pool of the query stream was closed before its query finished".into(),
                ))))
            }
        }
    }
}

/// The two JSON response methods of [`QueryStream`], on a combinator chain over it as well.
///
/// A chain out of [`crate::StreamExt`] is not a `QueryStream` anymore, so this trait is what its
/// last combinator answers a response with.  A stream you got straight from a `query_*_stream` call
/// has both methods as inherent ones and needs no import of this trait.
pub trait JsonLinesExt: TryStream + Send + 'static
where
    Self::Ok: Serialize,
    Self::Error: Into<BoxError>,
{
    /// The rows of this stream as an API response that answers with one JSON document per line,
    /// with nothing of the result set collected into memory at any point: a `200 OK`, a
    /// `CONTENT_TYPE` of `application/x-ndjson`, and [`Self::into_json_body`] as its body.
    ///
    /// Each row is serialized on its own, so it goes out the moment it is ready.  A [`QueryStream`]
    /// of [`crate::RowOwned`] writes an object keyed by the column names of the query, the shape
    /// [`crate::Client::query_raw_stream`] rows have; a stream of your own type writes that type,
    /// which means it needs [`serde::Serialize`], and a stream of the `Result` rows of
    /// [`crate::Client::query_try_map_stream`] writes `{"Ok": ...}` and `{"Err": ...}` - map those
    /// rows into the shape you want first.
    ///
    /// The rows keep traveling while the response is sent, so the memory of the query stays a
    /// single row: a client that stops reading parks the thread that runs the query on the row after
    /// the one it has received, and it unparks as soon as the client reads again.
    ///
    /// ```rust, ignore
    /// use hiqlite::{JsonLinesExt, StreamExt};
    ///
    /// let stream = client
    ///     .query_map_stream::<MyStruct, _>("SELECT * FROM users", params!())
    ///     .await?
    ///     .map(|res| res.map(|user| user.id));
    ///
    /// Ok::<_, hiqlite::Error>(stream.into_json_response())
    /// ```
    ///
    /// # Truncated responses
    ///
    /// The status line and the headers of a response are sent before the first row is read, so a
    /// query that fails mid-scan cannot turn into a `500` anymore: its body simply stops, and the
    /// client sees a response that ends before its last line.  Rows you have already sent are sent
    /// for real, and a truncated stream is never reportable as a complete one.
    ///
    /// # Panics
    ///
    /// Never, in practice: the only fallible step of building this response is a header value that
    /// this crate hard-codes.
    #[must_use = "the rows of a stream are only read while its response is sent"]
    fn into_json_response(self) -> Response
    where
        Self: Sized,
    {
        lines_response(self)
    }

    /// The rows of this stream as a streaming API body that carries one JSON document per line,
    /// with nothing of the result set collected into memory at any point: [`Self::into_json_response`]
    /// without the status code and the headers, for a response you build yourself.
    ///
    /// ```rust, ignore
    /// use axum::http::{header::CONTENT_TYPE, StatusCode};
    /// use hiqlite::JsonLinesExt;
    ///
    /// let stream = client
    ///     .query_map_stream::<MyStruct, _>("SELECT * FROM users", params!())
    ///     .await?
    ///     .map(|res| res.map(|user| user.id));
    ///
    /// let response = Response::builder()
    ///     .status(StatusCode::PARTIAL_CONTENT)
    ///     .header(CONTENT_TYPE, "application/x-ndjson")
    ///     .body(stream.into_json_body())
    ///     .unwrap();
    /// ```
    ///
    /// # Truncated responses
    ///
    /// The same as for [`Self::into_json_response`]: a query that fails mid-scan ends the body, and
    /// the rows already sent are sent for real.
    #[must_use = "the rows of a stream are only read while its body is sent"]
    fn into_json_body(self) -> Body
    where
        Self: Sized,
    {
        lines_body(self)
    }
}

impl<S> JsonLinesExt for S
where
    S: TryStream + Send + 'static,
    S::Ok: Serialize,
    S::Error: Into<BoxError>,
{
}

/// The streaming JSON body of a stream of rows: one serialized document per line, each one written
/// the moment its row is ready.
fn lines_body<S>(stream: S) -> Body
where
    S: TryStream + Send + 'static,
    S::Ok: Serialize,
    S::Error: Into<BoxError>,
{
    // Each row is serialized on its own, so the JSON of a row goes out the moment that row is
    // ready, and a failure of the query stays the failure of the body it travels in.
    Body::from_stream(stream.map_err(Into::into).map(|res| {
        match res {
            Ok(row) => serde_json::to_vec(&row)
                .map(|mut line| {
                    // One JSON document per line: the newline is what makes a body a stream of
                    // documents a client can read one by one.
                    line.push(b'\n');
                    Bytes::from(line)
                })
                .map_err(Into::into),
            Err(err) => Err(err),
        }
    }))
}

/// [`lines_body`] with a `200 OK` status and [`CONTENT_TYPE`] `application/x-ndjson` around it.
fn lines_response<S>(stream: S) -> Response
where
    S: TryStream + Send + 'static,
    S::Ok: Serialize,
    S::Error: Into<BoxError>,
{
    // A static header value and a body built from a stream cannot make the builder fail.
    Response::builder()
        .header(CONTENT_TYPE, NDJSON)
        .body(lines_body(stream))
        .expect("static response parts")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use tokio::task;

    /// The rows of a stream are read by the thread that owns its connection, one per request of its
    /// consumer: a `take(n)` ends that thread at its `n + 1`th row, and never reads a row behind
    /// the limit it was given.
    #[tokio::test]
    async fn a_stream_reads_only_the_rows_its_consumer_asks_for() {
        let (tx, rx) = flume::bounded::<Result<u64, Error>>(PREFETCH);
        let (verdict_tx, verdict_rx) = flume::bounded(1);
        // The producer publishes its end here, so the count below is read after its last row.
        let (done_tx, done_rx) = flume::bounded(1);
        let read = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&read);

        let producer = task::spawn_blocking(move || {
            for i in 0..1000u64 {
                // The very handoff a real scan does: a row is produced, handed over, and only then
                // comes the next one.
                if tx.send(Ok(i)).is_err() {
                    break;
                }
                counter.fetch_add(1, Ordering::SeqCst);
            }
            let _ = verdict_tx.send(StreamVerdict::Done);
            let _ = done_tx.send(());
        });

        let mut stream = QueryStream::new(rx, verdict_rx);
        let rows: Vec<u64> = (&mut stream)
            .take(3)
            .map(|res| res.expect("a row"))
            .collect()
            .await;
        assert_eq!(rows, [0, 1, 2]);

        // `take` ends the stream at its limit, which drops it and stops the thread behind it.
        drop(stream);
        done_rx.recv_async().await.expect("the producer ended");
        assert_eq!(
            read.load(Ordering::SeqCst),
            3,
            "the query read exactly the rows its consumer asked for"
        );
        drop(producer);
    }

    /// A scan that leaves its cursor on an error answers that error as the row after its last one:
    /// a truncated result is never reportable as a complete one.
    #[tokio::test]
    async fn a_scan_that_fails_midway_ends_the_stream_with_its_error() {
        let (tx, rx) = flume::bounded::<Result<u64, Error>>(PREFETCH);
        let (verdict_tx, verdict_rx) = flume::bounded(1);

        let producer = task::spawn_blocking(move || {
            for i in 0..3u64 {
                if tx.send(Ok(i)).is_err() {
                    return;
                }
            }
            let _ = verdict_tx.send(StreamVerdict::Err(Error::Sqlite(
                "the cursor cannot step".into(),
            )));
        });

        let mut stream = QueryStream::new(rx, verdict_rx);
        let mut rows = Vec::new();
        let mut tail = None;
        loop {
            match stream.next().await {
                Ok(Some(row)) => rows.push(row),
                Ok(None) => break,
                Err(err) => {
                    tail = Some(err);
                    break;
                }
            }
        }

        assert_eq!(rows, [0, 1, 2]);
        assert!(
            matches!(tail, Some(Error::Sqlite(_))),
            "the scan's own error is the row after the last one: {tail:?}"
        );
        drop(producer);
    }

    /// A row mapping that panics reaches its consumer the same way, on the verdict the actor
    /// answers after it caught the panic.
    #[tokio::test]
    async fn a_panicking_row_mapping_ends_the_stream_with_its_error() {
        let (tx, rx) = flume::bounded::<Result<u64, Error>>(PREFETCH);
        let (verdict_tx, verdict_rx) = flume::bounded(1);

        let producer = task::spawn_blocking(move || {
            let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                if tx.send(Ok(0)).is_ok() {
                    panic!("the row data does not match the mapped type");
                }
            }));
            let text = match res {
                Ok(()) => "no panic".to_string(),
                Err(payload) => payload
                    .downcast_ref::<String>()
                    .cloned()
                    .unwrap_or_else(|| "a read actor panicked without a panic message".to_string()),
            };
            let _ = verdict_tx.send(StreamVerdict::Err(Error::Error(text.into())));
        });

        let mut stream = QueryStream::new(rx, verdict_rx);
        assert_eq!(stream.next().await.expect("a row"), Some(0));
        let err = stream
            .next()
            .await
            .expect_err("the panic reaches its consumer");
        assert!(matches!(err, Error::Error(_)), "{err:?}");

        drop(producer);
    }
}
