use crate::Error;
use crate::app_state::AppState;
use crate::helpers::{deserialize, serialize, serialize_serde};
use axum::http::header::{ACCEPT, CONTENT_TYPE};
use axum::http::{HeaderMap, HeaderValue};
use axum::response::{IntoResponse, Response};
use axum::{Json, body};
use bincode_next::{Decode, Encode};
use constant_time_eq::constant_time_eq;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::sync::Arc;

pub use raft_client::NetworkStreaming;

pub(crate) mod api;
mod challenge_response;
pub(crate) mod handshake;
pub(crate) mod management;
mod raft_client;
pub(crate) mod raft_server;
pub(crate) mod tcp_socket;
pub(crate) mod web_socket_connect;

pub(crate) type AppStateExt = axum::extract::State<Arc<AppState>>;
pub(crate) type WireMetrics = openraft::RaftMetrics<TypeConfigWire>;
pub(crate) type WireMembership = openraft::type_config::alias::StoredMembershipOf<TypeConfigWire>;
pub(crate) type WireVote = openraft::type_config::alias::VoteOf<TypeConfigWire>;
pub(crate) type WireSnapshotMeta = openraft::type_config::alias::SnapshotMetaOf<TypeConfigWire>;

// openraft makes its membership, vote and snapshot types generic over the type config. The
// network layer serves both groups at runtime and pins them to this one config, because every
// type we carry over it is built from `NodeId`, `Node` and the macro defaults only - never from
// `D`/`R`. All our configs use the same values there, so the aliases below resolve to the exact
// same types as the groups' own and the seam is a pass, not a conversion. `RaftMetrics` is generic
// over the config itself, so it cannot be a single alias; shared code carries it as `WireMetrics`
// (the decode of a remote `/cluster/metrics` response) and the same argument applies to every type
// it is built from - which the guards below pin, one alias at a time.
openraft::declare_raft_types!(
    pub(crate) TypeConfigWire:
        D = String,
        R = String,
        Node = crate::Node,
);

// Guards the assumption the shared paths are built on: if a group ever changes its node id, node or
// membership type, these identity functions stop compiling instead of silently drifting.
#[cfg(feature = "sqlite")]
const _: fn(
    openraft::type_config::alias::VoteOf<crate::store::state_machine::sqlite::TypeConfigSqlite>,
) -> WireVote = |vote| vote;
#[cfg(feature = "sqlite")]
const _: fn(
    openraft::type_config::alias::SnapshotMetaOf<
        crate::store::state_machine::sqlite::TypeConfigSqlite,
    >,
) -> WireSnapshotMeta = |meta| meta;
#[cfg(feature = "sqlite")]
const _: fn(
    openraft::type_config::alias::StoredMembershipOf<
        crate::store::state_machine::sqlite::TypeConfigSqlite,
    >,
) -> WireMembership = |membership| membership;
#[cfg(feature = "cache")]
const _: fn(
    openraft::type_config::alias::VoteOf<crate::store::state_machine::memory::TypeConfigKV>,
) -> WireVote = |vote| vote;
#[cfg(feature = "cache")]
const _: fn(
    openraft::type_config::alias::SnapshotMetaOf<crate::store::state_machine::memory::TypeConfigKV>,
) -> WireSnapshotMeta = |meta| meta;
#[cfg(feature = "cache")]
const _: fn(
    openraft::type_config::alias::StoredMembershipOf<
        crate::store::state_machine::memory::TypeConfigKV,
    >,
) -> WireMembership = |membership| membership;

// The same guard for the aliases `RaftMetrics` and the `Error` payloads are built from. They all
// resolve to the macro defaults today, so each of these is an identity - and it stops being one the
// moment a config sets `D`/`R`-derived `NodeId`, `LeaderId`, `Membership`, `LogId` or `Instant`.
macro_rules! wire_alias_guard {
    ($alias:ident) => {
        #[cfg(feature = "sqlite")]
        const _: fn(
            openraft::type_config::alias::$alias<
                crate::store::state_machine::sqlite::TypeConfigSqlite,
            >,
        ) -> openraft::type_config::alias::$alias<TypeConfigWire> = |x| x;
        #[cfg(feature = "cache")]
        const _: fn(
            openraft::type_config::alias::$alias<crate::store::state_machine::memory::TypeConfigKV>,
        ) -> openraft::type_config::alias::$alias<TypeConfigWire> = |x| x;
    };
}

wire_alias_guard!(NodeIdOf);
wire_alias_guard!(CommittedLeaderIdOf);
wire_alias_guard!(MembershipOf);
wire_alias_guard!(LogIdOf);
wire_alias_guard!(SnapshotSignatureOf);
wire_alias_guard!(SerdeInstantOf);

// `RaftMetrics` and the openraft errors are generic over the config, so an identity function can't
// express the guard; size and alignment are what Rust can check here, and the test at the bottom of
// this module covers the layout itself.
#[cfg(feature = "sqlite")]
const _: () = assert!(
    size_of::<openraft::RaftMetrics<crate::store::state_machine::sqlite::TypeConfigSqlite>>()
        == size_of::<WireMetrics>()
);
#[cfg(feature = "sqlite")]
const _: () = assert!(
    align_of::<openraft::RaftMetrics<crate::store::state_machine::sqlite::TypeConfigSqlite>>()
        == align_of::<WireMetrics>()
);
#[cfg(feature = "cache")]
const _: () = assert!(
    size_of::<openraft::RaftMetrics<crate::store::state_machine::memory::TypeConfigKV>>()
        == size_of::<WireMetrics>()
);
#[cfg(feature = "cache")]
const _: () = assert!(
    align_of::<openraft::RaftMetrics<crate::store::state_machine::memory::TypeConfigKV>>()
        == align_of::<WireMetrics>()
);

// TODO change to default `Authorization` -> breaking change
pub const HEADER_NAME_SECRET: &str = "X-API-SECRET";

#[inline(always)]
fn get_payload<T>(headers: &HeaderMap, body: body::Bytes) -> Result<T, Error>
where
    T: for<'a> Deserialize<'a> + Decode<()>,
{
    if let Some(typ) = headers.get(CONTENT_TYPE)
        && typ == HeaderValue::from_static("application/json")
    {
        return Ok(serde_json::from_slice(body.as_ref())?);
    }
    Ok(deserialize(body.as_ref())?)
}

#[inline(always)]
fn fmt_ok<S: Debug + Serialize + Encode>(
    headers: HeaderMap,
    payload: S,
) -> Result<Response, Error> {
    if let Some(accept) = headers.get(ACCEPT)
        && accept == HeaderValue::from_static("application/json")
    {
        return Ok(Json(payload).into_response());
    }
    Ok(serialize_network(&payload).into_response())
}

// Types we don't own - openraft's - only have serde impls, so this path keeps the serde adapter,
// which the goldens pin as byte-identical to the native one for everything carried here.
#[inline(always)]
fn fmt_ok_serde<S: Debug + Serialize>(headers: HeaderMap, payload: S) -> Result<Response, Error> {
    if let Some(accept) = headers.get(ACCEPT)
        && accept == HeaderValue::from_static("application/json")
    {
        return Ok(Json(payload).into_response());
    }
    Ok(serialize_serde(&payload)
        .expect("Network payload serialization should always succeed")
        .into_response())
}

/// Serialization used for all network requests for non-raft-internal traffic and types.
/// # Panics
/// If the given type cannot be serialized with bincode-next successfully
#[inline(always)]
pub fn serialize_network<T: Encode>(value: &T) -> Vec<u8> {
    serialize(value).expect("Network payload serialization should always succeed")
}

#[inline(always)]
fn validate_secret(state: &AppStateExt, headers: &HeaderMap) -> Result<(), Error> {
    match headers.get(HEADER_NAME_SECRET) {
        None => Err(Error::Token("API Secret missing".into())),
        Some(secret) => {
            if !constant_time_eq(state.secret_api.as_bytes(), secret.as_bytes()) {
                Err(Error::Token("Invalid API Secret".into()))
            } else {
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::helpers::deserialize_serde;
    use std::collections::{BTreeMap, BTreeSet};

    // One sample, spelled once per type config, so the layout check compares values that are equal
    // by construction. `Instant` fields stay `None`: their serde round trip is documented to be off
    // by up to a microsecond, so they can't take part in a byte-exact check - the aliases they are
    // built from are pinned by the guards above instead.
    macro_rules! sample_metrics {
        ($config:ty) => {{
            openraft::RaftMetrics::<$config> {
                running_state: Ok(()),
                id: 3,
                current_term: 7,
                vote: openraft::type_config::alias::VoteOf::<$config> {
                    leader_id: openraft::impls::leader_id_adv::LeaderId {
                        term: 7u64,
                        node_id: 3u64,
                    },
                    committed: true,
                },
                last_log_index: Some(11),
                local_committed: Some(openraft::type_config::alias::LogIdOf::<$config> {
                    leader_id: openraft::impls::leader_id_adv::LeaderId {
                        term: 7u64,
                        node_id: 3u64,
                    },
                    index: 11,
                }),
                cluster_committed: Some(openraft::type_config::alias::LogIdOf::<$config> {
                    leader_id: openraft::impls::leader_id_adv::LeaderId {
                        term: 6u64,
                        node_id: 2u64,
                    },
                    index: 9,
                }),
                committed: None,
                last_applied: Some(openraft::type_config::alias::LogIdOf::<$config> {
                    leader_id: openraft::impls::leader_id_adv::LeaderId {
                        term: 5u64,
                        node_id: 1u64,
                    },
                    index: 7,
                }),
                snapshot: None,
                purged: Some(openraft::type_config::alias::LogIdOf::<$config> {
                    leader_id: openraft::impls::leader_id_adv::LeaderId {
                        term: 1u64,
                        node_id: 0u64,
                    },
                    index: 1,
                }),
                state: openraft::ServerState::Leader,
                current_leader: Some(3),
                millis_since_quorum_ack: None,
                last_quorum_acked: None,
                membership_config: std::sync::Arc::new(
                    openraft::type_config::alias::StoredMembershipOf::<$config>::new(
                        None,
                        openraft::type_config::alias::MembershipOf::<$config>::new(
                            vec![BTreeSet::new()],
                            BTreeMap::<u64, crate::Node>::new(),
                        )
                        .unwrap(),
                    ),
                ),
                committed_membership_config: std::sync::Arc::new(
                    openraft::type_config::alias::StoredMembershipOf::<$config>::new(
                        None,
                        openraft::type_config::alias::MembershipOf::<$config>::new(
                            vec![BTreeSet::new()],
                            BTreeMap::<u64, crate::Node>::new(),
                        )
                        .unwrap(),
                    ),
                ),
                heartbeat: Some(BTreeMap::from([(3u64, None)])),
                replication: Some(BTreeMap::from([(
                    3u64,
                    Some(openraft::type_config::alias::LogIdOf::<$config> {
                        leader_id: openraft::impls::leader_id_adv::LeaderId {
                            term: 7u64,
                            node_id: 3u64,
                        },
                        index: 11,
                    }),
                )])),
            }
        }};
    }

    /// The runtime half of the `TypeConfigWire` guard: `RaftMetrics` is generic over the config, so
    /// Rust cannot express the identity as a `const` - only size and alignment above - and this test
    /// pins the layout itself by reading one config's bytes through another's type, both ways.
    #[expect(deprecated)]
    #[test]
    fn raft_metrics_layout_is_config_independent() {
        let wire_bytes = serialize_serde(&sample_metrics!(TypeConfigWire)).unwrap();

        // The samples are equal by construction, so identical bytes mean identical layouts. Each
        // direction is then proven through the decoder the shared code paths use.
        #[cfg(feature = "sqlite")]
        {
            type Sqlite = crate::store::state_machine::sqlite::TypeConfigSqlite;
            let group_bytes = serialize_serde(&sample_metrics!(Sqlite)).unwrap();
            assert_eq!(
                group_bytes, wire_bytes,
                "the sqlite group's metrics no longer read as the wire config's"
            );
            let read = deserialize_serde::<WireMetrics>(&group_bytes).unwrap();
            assert_eq!(serialize_serde(&read).unwrap(), group_bytes);
            let read = deserialize_serde::<openraft::RaftMetrics<Sqlite>>(&wire_bytes).unwrap();
            assert_eq!(serialize_serde(&read).unwrap(), group_bytes);
        }

        #[cfg(feature = "cache")]
        {
            type Kv = crate::store::state_machine::memory::TypeConfigKV;
            let group_bytes = serialize_serde(&sample_metrics!(Kv)).unwrap();
            assert_eq!(
                group_bytes, wire_bytes,
                "the cache group's metrics no longer read as the wire config's"
            );
            let read = deserialize_serde::<WireMetrics>(&group_bytes).unwrap();
            assert_eq!(serialize_serde(&read).unwrap(), group_bytes);
            let read = deserialize_serde::<openraft::RaftMetrics<Kv>>(&wire_bytes).unwrap();
            assert_eq!(serialize_serde(&read).unwrap(), group_bytes);
        }
    }
}
