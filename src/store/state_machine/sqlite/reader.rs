// use crate::store::sqlite_writer::WriterRequest;
// use crate::store::state_machine_sqlite::{Params, Query, Sql, StateMachineStore};
// use crate::ApiError;
// use std::fmt::Debug;
// use std::thread;
// use std::time::Duration;
// use tokio::sync::oneshot;
//
// pub trait SqlReader {
//     type F: FnOnce(&rusqlite::Row<'_>) -> rusqlite::Result<Self::T> + Send;
//     type T: Debug + Send + 'static;
// }
//
// // pub type SqlReader {
// //     F: FnOnce(&rusqlite::Row<'_>) -> rusqlite::Result<T> + Send,
// //     T: Debug + Send + 'static,
// // }
//
// // pub type QueryMapRow =
// //     dyn FnOnce(&rusqlite::Row<'_>) -> Result<QueryMapResult, rusqlite::Error> + Send;
// // pub type QueryMapResult = dyn Debug + Send;
//
// #[derive(Debug)]
// pub enum ReaderRequest<
//     F: FnOnce(&rusqlite::Row<'_>) -> rusqlite::Result<T> + Send,
//     T: Debug + Send + 'static,
// > {
//     QueryMap(QueryMap<F, T>),
//     // QueryAs(QueryAs),
// }
//
// // #[derive(Debug)]
// // pub enum ReaderRequest<
// //     F: FnOnce(&rusqlite::Row<'_>) -> rusqlite::Result<T> + Send,
// //     T: Debug + Send + 'static,
// // > {
// //     QueryMap(QueryMap<F, T>),
// //     // QueryAs(QueryAs),
// // }
//
// #[derive(Debug)]
// pub struct QueryMap<
//     F: FnOnce(&rusqlite::Row<'_>) -> rusqlite::Result<T> + Send,
//     T: Debug + Send + 'static,
// > {
//     sql: Sql,
//     callback: F,
//     ack: oneshot::Sender<Result<T, ApiError>>,
// }
//
// #[derive(Debug)]
// pub struct QueryAs {}
//
// pub fn spawn_reader<F, T>(
//     path_db: String,
//     filename_db: String,
//     in_memory: bool,
// ) -> flume::Sender<ReaderRequest<F, T>>
// where
//     F: FnOnce(&rusqlite::Row<'_>) -> rusqlite::Result<T> + Send + 'static,
//     T: Debug + Send + 'static,
// {
//     let (tx, rx) = flume::bounded::<ReaderRequest<F, T>>(2);
//
//     let _handle = thread::spawn(move || {
//         let conn = wait_for_connection(&path_db, &filename_db, in_memory);
//
//         while let Ok(req) = rx.recv() {
//             match req {
//                 ReaderRequest::QueryMap(QueryMap { sql, callback, ack }) => {
//                     let mut stmt = conn.prepare_cached(&sql.sql).unwrap();
//                     let r = stmt.query_row((), callback).map_err(ApiError::from);
//                     ack.send(r).unwrap()
//                 } // ReaderRequest::QueryAs(Sql { sql, params }) => {}
//             }
//         }
//     });
//
//     tx
// }
//
// // pub fn spawn_reader(
// //     path_db: String,
// //     filename_db: String,
// //     in_memory: bool,
// // ) -> flume::Sender<ReaderRequest<F, T>> {
// //     let (tx, rx) = flume::bounded::<ReaderRequest<F, T>>(2);
// //
// //     let _handle = thread::spawn(move || {
// //         let conn = wait_for_connection(&path_db, &filename_db, in_memory);
// //
// //         while let Ok(req) = rx.recv() {
// //             match req {
// //                 ReaderRequest::QueryMap(QueryMap { sql, callback, ack }) => {
// //                     let mut stmt = conn.prepare_cached(&sql.sql).unwrap();
// //                     let r = stmt.query_row((), callback).map_err(ApiError::from);
// //                     ack.send(r).unwrap()
// //                 } // ReaderRequest::QueryAs(Sql { sql, params }) => {}
// //             }
// //         }
// //     });
// //
// //     tx
// // }
//
// /// MUST NOT be executed in async context!
// fn wait_for_connection(path_db: &str, filename_db: &str, in_memory: bool) -> rusqlite::Connection {
//     let mut conn = StateMachineStore::connect(path_db, filename_db, in_memory, true);
//     // we connect in a loop here to not get into trouble with the writer setting up a new DB from scratch which is
//     // not ready yet and therefore locked
//     while conn.is_err() {
//         thread::sleep(Duration::from_millis(100));
//         conn = StateMachineStore::connect(&path_db, &filename_db, in_memory, true);
//     }
//     conn.unwrap()
// }
