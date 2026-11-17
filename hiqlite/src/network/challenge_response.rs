//! Fixed-size wire format for the secret handshake, serialized and parsed by hand - no bincode,
//! no allocations. This module is the sole guard of the network protocol: every packet has an
//! exact, known length upfront, so a peer can never make this side allocate from a length the
//! peer itself chose.
//!
//! - `Challenge`: `magic(8) || secret_id(24) || salt(24)` = 56 bytes
//! - `ChallengeResponse`: `node_id(8, big-endian) || response(32) || salt(24)` = 64 bytes
//! - `ResponseFinal`: `hash(32)` = 32 bytes
//!
//! The magic number is the version gate: a peer that does not send it runs an incompatible
//! version and is refused on the very first payload. `secret_id` is the truncated SHA-256 of the
//! shared secret; it gates the response, so a peer without the secret is refused before this
//! side writes a single secret-derived byte. `response` is `SHA256(peer_salt || secret)`, the
//! final `hash` is `SHA256(our_salt || secret)`.

use crate::{Error, NodeId};
use constant_time_eq::constant_time_eq_32;
use sha2::{Digest, Sha256};

/// The magic number identifying the network protocol version. It is embedded into the
/// `Challenge` as its first payload: every handshake starts with it, in both directions.
pub(crate) const MAGIC_NO_NETWORK: &str = "HQLITE_1";
const MAGIC_LEN: usize = MAGIC_NO_NETWORK.len();

const NODE_ID_LEN: usize = 8;
const SECRET_ID_LEN: usize = 24;
const SALT_LEN: usize = 24;
const HASH_LEN: usize = 32;

struct Salt([u8; SALT_LEN]);

impl Salt {
    fn new() -> Result<Self, Error> {
        let mut buf = [0u8; SALT_LEN];
        getrandom::fill(&mut buf).map_err(|_| Error::Error("getrandom Error".into()))?;
        Ok(Self(buf))
    }
}

#[derive(Debug)]
pub(crate) struct Challenge {
    magic: &'static str,
    secret_id: [u8; SECRET_ID_LEN],
    challenge: [u8; SALT_LEN],
}

impl Challenge {
    pub(crate) const WIRE_SIZE: usize = MAGIC_LEN + SECRET_ID_LEN + SALT_LEN;

    pub(crate) fn new(secret: &[u8]) -> Result<Self, Error> {
        let salt = Salt::new()?;
        Ok(Self {
            magic: MAGIC_NO_NETWORK,
            secret_id: secret_id(secret),
            challenge: salt.0,
        })
    }

    /// Parse a challenge from the wire. The magic is checked here: a peer that does not send it
    /// runs an incompatible version.
    pub(crate) fn parse(input: &[u8]) -> Result<Self, Error> {
        if input.len() < Self::WIRE_SIZE {
            return Err(Error::BadRequest("invalid challenge length".into()));
        }

        let magic = &input[..MAGIC_LEN];
        if magic != MAGIC_NO_NETWORK.as_bytes() {
            return Err(Error::BadRequest(
                "invalid magic number - peer runs an incompatible version".into(),
            ));
        }

        Ok(Self {
            magic: MAGIC_NO_NETWORK,
            secret_id: input[MAGIC_LEN..MAGIC_LEN + SECRET_ID_LEN]
                .try_into()
                .expect("slice length checked to be SECRET_ID_LEN above"),
            challenge: input[MAGIC_LEN + SECRET_ID_LEN..Self::WIRE_SIZE]
                .try_into()
                .expect("slice length checked to be SALT_LEN above"),
        })
    }

    pub(crate) fn write_wire(&self, buf: &mut [u8]) {
        debug_assert_eq!(buf.len(), Self::WIRE_SIZE);
        buf[..MAGIC_LEN].copy_from_slice(self.magic.as_bytes());
        buf[MAGIC_LEN..MAGIC_LEN + SECRET_ID_LEN].copy_from_slice(&self.secret_id);
        buf[MAGIC_LEN + SECRET_ID_LEN..].copy_from_slice(&self.challenge);
    }
}

#[derive(Debug)]
pub(crate) struct ChallengeResponse {
    node_id: NodeId,
    response: [u8; HASH_LEN],
    challenge: [u8; SALT_LEN],
}

impl ChallengeResponse {
    pub(crate) const WIRE_SIZE: usize = NODE_ID_LEN + HASH_LEN + SALT_LEN;

    /// Build the response for the challenge - the challenge's `secret_id` must match the
    /// cluster secret, which is the one the finalization runs on.
    pub(crate) fn new(
        challenge: &Challenge,
        secret: &[u8],
        node_id: NodeId,
    ) -> Result<Self, Error> {
        // Not the mismatch detection - the answer and the finalization are that. This is the gate
        // in front of them: the only check answered before this side writes a single
        // secret-derived byte. Without it, anyone who can open the cluster port gets
        // `SHA256(their_own_salt || secret)` for a salt they chose, pre-auth and unlimited, and
        // this connection then waits on a finalize the refusing peer never sends.
        if secret_id(secret) != challenge.secret_id {
            return Err(Error::BadRequest(
                "unknown cluster secret used for the challenge".into(),
            ));
        }

        let mut response = [0u8; HASH_LEN];
        response.copy_from_slice(
            Sha256::new()
                .chain_update(challenge.challenge)
                .chain_update(secret)
                .finalize()
                .as_slice(),
        );

        Ok(Self {
            node_id,
            response,
            challenge: Salt::new()?.0,
        })
    }

    pub(crate) fn write_wire(&self, buf: &mut [u8]) {
        debug_assert_eq!(buf.len(), Self::WIRE_SIZE);
        buf[..NODE_ID_LEN].copy_from_slice(&self.node_id.to_be_bytes());
        buf[NODE_ID_LEN..NODE_ID_LEN + HASH_LEN].copy_from_slice(&self.response);
        buf[NODE_ID_LEN + HASH_LEN..].copy_from_slice(&self.challenge);
    }

    pub(crate) fn verify(
        input: &[u8],
        challenge: &Challenge,
        secret: &[u8],
    ) -> Result<(NodeId, ResponseFinal), Error> {
        if input.len() < Self::WIRE_SIZE {
            return Err(Error::BadRequest(
                "invalid challenge response length".into(),
            ));
        }

        let node_id = u64::from_be_bytes(
            input[..NODE_ID_LEN]
                .try_into()
                .expect("slice length checked to be NODE_ID_LEN above"),
        );
        let response = &input[NODE_ID_LEN..NODE_ID_LEN + HASH_LEN];
        let challenge_resp = &input[NODE_ID_LEN + HASH_LEN..Self::WIRE_SIZE];

        let verify = Sha256::new()
            .chain_update(challenge.challenge)
            .chain_update(secret)
            .finalize()
            .to_vec();

        if !constant_time_eq_32(
            <&[u8; HASH_LEN]>::try_from(response).unwrap(),
            <&[u8; HASH_LEN]>::try_from(verify.as_slice()).unwrap(),
        ) {
            return Err(Error::BadRequest("invalid challenge response".into()));
        }

        let mut response_new = [0u8; HASH_LEN];
        response_new.copy_from_slice(
            Sha256::new()
                .chain_update(challenge_resp)
                .chain_update(secret)
                .finalize()
                .as_slice(),
        );

        Ok((node_id, ResponseFinal(response_new)))
    }
}

#[derive(Debug)]
pub(crate) struct ResponseFinal([u8; HASH_LEN]);

impl ResponseFinal {
    pub(crate) const WIRE_SIZE: usize = HASH_LEN;

    pub(crate) fn write_wire(&self, buf: &mut [u8]) {
        debug_assert_eq!(buf.len(), HASH_LEN);
        buf.copy_from_slice(&self.0);
    }

    pub(crate) fn verify(
        input: &[u8],
        challenge_response: &ChallengeResponse,
        secret: &[u8],
    ) -> Result<(), Error> {
        if input.len() < HASH_LEN {
            return Err(Error::BadRequest(
                "invalid input length for challenge response".into(),
            ));
        }
        let fin = &input[..HASH_LEN];

        let verify = Sha256::new()
            .chain_update(challenge_response.challenge)
            .chain_update(secret)
            .finalize()
            .to_vec();

        if !constant_time_eq_32(
            <&[u8; HASH_LEN]>::try_from(fin).unwrap(),
            <&[u8; HASH_LEN]>::try_from(verify.as_slice()).unwrap(),
        ) {
            Err(Error::BadRequest("invalid challenge response".into()))
        } else {
            Ok(())
        }
    }
}

#[inline(always)]
fn secret_id(secret: &[u8]) -> [u8; SECRET_ID_LEN] {
    let hash = Sha256::digest(secret);
    let mut id = [0u8; SECRET_ID_LEN];
    id.copy_from_slice(&hash[..SECRET_ID_LEN]);
    id
}

#[cfg(test)]
mod tests {
    use super::{Challenge, ChallengeResponse, Error, ResponseFinal};
    use super::{HASH_LEN, MAGIC_LEN, MAGIC_NO_NETWORK, secret_id};

    const SECRET: &[u8] = b"SuperMegaSecure1337";
    const SECRET_BAD: &[u8] = b"SuperMegaSecure";
    const NODE_ID: u64 = 7;

    fn wire<const N: usize>() -> Vec<u8> {
        vec![0u8; N]
    }

    /// Full round trip over the exact wire bytes, like the peers exchange them.
    #[test]
    fn test_challenge_response() {
        let challenge = Challenge::new(SECRET).unwrap();
        let mut challenge_buf = wire::<{ Challenge::WIRE_SIZE }>();
        challenge.write_wire(&mut challenge_buf);

        let cr =
            ChallengeResponse::new(&Challenge::parse(&challenge_buf).unwrap(), SECRET, NODE_ID)
                .unwrap();
        let mut cr_buf = wire::<{ ChallengeResponse::WIRE_SIZE }>();
        cr.write_wire(&mut cr_buf);

        let (node_id, final_resp) = ChallengeResponse::verify(&cr_buf, &challenge, SECRET).unwrap();
        assert_eq!(node_id, NODE_ID, "wire carries the node id in big-endian");

        let mut final_buf = wire::<{ ResponseFinal::WIRE_SIZE }>();
        final_resp.write_wire(&mut final_buf);

        // Wrong secrets must fail both verify directions.
        assert!(ChallengeResponse::verify(&cr_buf, &challenge, SECRET_BAD).is_err());
        assert!(ResponseFinal::verify(&final_buf, &cr, SECRET_BAD).is_err());
        ResponseFinal::verify(&final_buf, &cr, SECRET).unwrap();
    }

    /// The magic sits at the very front of the challenge wire, and the challenge carries it.
    #[test]
    fn test_magic_is_the_first_payload() {
        let challenge = Challenge::new(SECRET).unwrap();
        let mut challenge_buf = wire::<{ Challenge::WIRE_SIZE }>();
        challenge.write_wire(&mut challenge_buf);
        assert!(challenge_buf.starts_with(MAGIC_NO_NETWORK.as_bytes()));

        let parsed = Challenge::parse(&challenge_buf).unwrap();
        assert_eq!(parsed.magic, MAGIC_NO_NETWORK);
        assert_eq!(parsed.secret_id, secret_id(SECRET));
    }

    /// A peer without the magic is refused on the challenge parse.
    #[test]
    fn test_missing_magic_is_refused() {
        for len in 0..MAGIC_LEN {
            let bytes = vec![b'H'; len];
            assert!(
                Challenge::parse(&bytes).is_err(),
                "challenge with {len} magic bytes must be refused"
            );
        }

        let mut challenge_buf = wire::<{ Challenge::WIRE_SIZE }>();
        Challenge::new(SECRET)
            .unwrap()
            .write_wire(&mut challenge_buf);
        challenge_buf[..MAGIC_LEN].copy_from_slice(b"NOT_HQLI");
        assert!(Challenge::parse(&challenge_buf).is_err());
    }

    /// The `secret_id` gate: a peer without the secret is refused on the challenge, before any
    /// secret-derived byte is produced.
    #[test]
    fn test_unknown_secret_is_gated() {
        let challenge = Challenge::new(SECRET).unwrap();
        let mut challenge_buf = wire::<{ Challenge::WIRE_SIZE }>();
        challenge.write_wire(&mut challenge_buf);

        let err = ChallengeResponse::new(
            &Challenge::parse(&challenge_buf).unwrap(),
            SECRET_BAD,
            NODE_ID,
        )
        .expect_err("a challenge for a secret we don't have must be refused");
        assert!(matches!(err, Error::BadRequest(_)), "{err}");
    }

    /// Every short, network-supplied packet must return `Err` on the length gate, never panic.
    #[test]
    fn test_malformed_lengths_do_not_panic() {
        let challenge = Challenge::new(SECRET).unwrap();

        for len in 0..Challenge::WIRE_SIZE {
            let bytes = vec![0u8; len];
            assert!(
                Challenge::parse(&bytes).is_err(),
                "challenge of {len} bytes must be rejected"
            );
        }

        for len in 0..ChallengeResponse::WIRE_SIZE {
            let bytes = vec![0u8; len];
            assert!(
                ChallengeResponse::verify(&bytes, &challenge, SECRET).is_err(),
                "challenge response of {len} bytes must be rejected"
            );
        }

        for len in 0..HASH_LEN {
            let bytes = vec![0u8; len];
            let cr = ChallengeResponse {
                node_id: NODE_ID,
                response: [0u8; HASH_LEN],
                challenge: [0u8; 24],
            };
            assert!(
                ResponseFinal::verify(&bytes, &cr, SECRET).is_err(),
                "response final of {len} bytes must be rejected"
            );
        }
    }
}
