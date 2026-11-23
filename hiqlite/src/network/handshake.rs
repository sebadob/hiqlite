use crate::Error;
use crate::NodeId;
use crate::network::challenge_response::{Challenge, ChallengeResponse, ResponseFinal};
use fastwebsockets::{Frame, OpCode, Payload, WebSocket};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncWrite};
use tracing::debug;

// A peer that opens the WebSocket and then stalls is refused fast - one second is far beyond any
// localhost / LAN round trip.
const MAGIC_TIMEOUT: Duration = Duration::from_secs(1);

pub struct HandshakeSecret;

impl HandshakeSecret {
    /// Server step 1: build the Challenge - magic and all gates live in the challenge itself -
    /// and write its wire payload into `wire`. Returns the state needed to finish, plus the
    /// payload length.
    pub(crate) fn server_challenge(
        secret: &[u8],
        wire: &mut Vec<u8>,
    ) -> Result<(Challenge, usize), Error> {
        let challenge = Challenge::new(secret)?;
        wire.resize(Challenge::WIRE_SIZE, 0);
        challenge.write_wire(wire);
        Ok((challenge, Challenge::WIRE_SIZE))
    }

    /// Server step 2: verify the client's `ChallengeResponse` and write the `ResponseFinal` into
    /// `wire`. Returns the client's node id, plus the payload length.
    pub(crate) fn server_finish(
        input: &[u8],
        challenge: &Challenge,
        secret: &[u8],
        wire: &mut Vec<u8>,
    ) -> Result<(NodeId, usize), Error> {
        let (node_id, response) = ChallengeResponse::verify(input, challenge, secret)?;
        wire.resize(ResponseFinal::WIRE_SIZE, 0);
        response.write_wire(wire);
        Ok((node_id, ResponseFinal::WIRE_SIZE))
    }

    /// Client step 1: parse the server's Challenge (magic checked inside) and answer with the
    /// `ChallengeResponse` written into `wire`. Returns the state needed to verify the server's
    /// final answer, plus the payload length.
    pub(crate) fn client_answer(
        input: &[u8],
        secret: &[u8],
        node_id: NodeId,
        wire: &mut Vec<u8>,
    ) -> Result<(ChallengeResponse, usize), Error> {
        let challenge = Challenge::parse(input)?;
        let cr = ChallengeResponse::new(&challenge, secret, node_id)?;
        wire.resize(ChallengeResponse::WIRE_SIZE, 0);
        cr.write_wire(wire);
        Ok((cr, ChallengeResponse::WIRE_SIZE))
    }

    /// Client step 2: verify the server's `ResponseFinal`.
    pub(crate) fn client_finish(
        input: &[u8],
        challenge_response: &ChallengeResponse,
        secret: &[u8],
    ) -> Result<(), Error> {
        ResponseFinal::verify(input, challenge_response, secret)
    }

    pub async fn client<S>(
        ws: &mut WebSocket<S>,
        secret: &[u8],
        node_id: NodeId,
    ) -> Result<(), Error>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        debug!("Executing HandshakeSecret::client");

        // The largest wire payload is the ChallengeResponse; the buffer is reused for all frames.
        let mut wire = Vec::with_capacity(ChallengeResponse::WIRE_SIZE);

        let frame = Self::read_binary(ws, MAGIC_TIMEOUT).await?;
        let (cr, len) = Self::client_answer(&frame, secret, node_id, &mut wire)?;
        ws.write_frame(Frame::binary(Payload::from(&wire[..len])))
            .await?;

        let frame = Self::read_binary(ws, MAGIC_TIMEOUT).await?;
        Self::client_finish(&frame, &cr, secret)?;

        debug!("HandshakeSecret::client finished");
        Ok(())
    }

    pub(crate) async fn server<S>(ws: &mut WebSocket<S>, secret: &[u8]) -> Result<NodeId, Error>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        debug!("Executing HandshakeSecret::server");

        // The largest wire payload is the ChallengeResponse; the buffer is reused for all frames.
        let mut wire = Vec::with_capacity(ChallengeResponse::WIRE_SIZE);

        let (challenge, len) = Self::server_challenge(secret, &mut wire)?;
        ws.write_frame(Frame::binary(Payload::from(&wire[..len])))
            .await?;

        let frame = Self::read_binary(ws, MAGIC_TIMEOUT).await?;
        let (node_id, len) = Self::server_finish(&frame, &challenge, secret, &mut wire)?;
        ws.write_frame(Frame::binary(Payload::from(&wire[..len])))
            .await?;

        debug!("HandshakeSecret::server finished");
        Ok(node_id)
    }

    /// Reads the next frame, which must be a binary one, and hands out its raw payload.
    async fn read_binary<S>(ws: &mut WebSocket<S>, timeout: Duration) -> Result<Vec<u8>, Error>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        let frame = tokio::time::timeout(timeout, ws.read_frame())
            .await
            .map_err(|_| Error::Connect("Timeout waiting for frame from peer".to_string()))?
            .map_err(|err| Error::Connect(format!("Reading frame from peer failed: {err}")))?;

        match frame.opcode {
            OpCode::Binary => Ok(frame.payload.to_vec()),
            _ => Err(Error::BadRequest(
                "Invalid frame opcode from peer - peer runs an incompatible version".into(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::challenge_response::MAGIC_NO_NETWORK;

    const SECRET: &[u8] = b"SuperMegaSecure1337";
    const SECRET_BAD: &[u8] = b"SuperMegaSecure";

    fn new_wire() -> Vec<u8> {
        Vec::with_capacity(ChallengeResponse::WIRE_SIZE)
    }

    /// Full handshake over the raw payloads, both directions, node id on the wire.
    #[test]
    fn magic_and_secret_handshake_complete_in_both_directions() {
        let mut wire = new_wire();
        let (challenge, len) = HandshakeSecret::server_challenge(SECRET, &mut wire).unwrap();
        let server_hello = wire[..len].to_vec();

        let mut wire = new_wire();
        let (cr, len) =
            HandshakeSecret::client_answer(&server_hello, SECRET, 42, &mut wire).unwrap();
        let client_answer = wire[..len].to_vec();

        let mut wire = new_wire();
        let (node_id, len) =
            HandshakeSecret::server_finish(&client_answer, &challenge, SECRET, &mut wire).unwrap();
        assert_eq!(node_id, 42, "the wire carries the client's node id");
        let server_final = wire[..len].to_vec();

        HandshakeSecret::client_finish(&server_final, &cr, SECRET).unwrap();
    }

    /// Every handshake starts with the magic number, carried by the Challenge.
    #[test]
    fn magic_is_the_first_payload_of_the_handshake() {
        let mut wire = new_wire();
        let (_, len) = HandshakeSecret::server_challenge(SECRET, &mut wire).unwrap();
        assert!(wire[..len].starts_with(MAGIC_NO_NETWORK.as_bytes()));
    }

    /// A peer that does not send the magic is refused, before any secret is touched.
    #[test]
    fn magic_mismatch_is_refused() {
        let mut wire = new_wire();
        let (challenge, len) = HandshakeSecret::server_challenge(SECRET, &mut wire).unwrap();
        let server_hello = wire[..len].to_vec();

        for bad in [&b""[..], &b"NOT_HQLITE"[..], &server_hello[1..]] {
            let err = HandshakeSecret::client_answer(bad, SECRET, 1, &mut new_wire())
                .expect_err("missing or wrong magic must be refused");
            assert!(matches!(err, Error::BadRequest(_)), "{err}");

            let err = HandshakeSecret::server_finish(bad, &challenge, SECRET, &mut new_wire())
                .expect_err("missing or wrong magic must be refused");
            assert!(matches!(err, Error::BadRequest(_)), "{err}");
        }
    }

    /// The `secret_id` gate: a peer without the secret is refused before any secret-derived byte,
    /// and the final verification of a wrong secret fails on both sides.
    #[test]
    fn wrong_secret_is_refused() {
        let mut wire = new_wire();
        let (challenge, len) = HandshakeSecret::server_challenge(SECRET, &mut wire).unwrap();
        let server_hello = wire[..len].to_vec();

        let err = HandshakeSecret::client_answer(&server_hello, SECRET_BAD, 1, &mut new_wire())
            .expect_err("a challenge for a secret we don't have must be refused");
        assert!(matches!(err, Error::BadRequest(_)), "{err}");

        let mut wire = new_wire();
        let (cr, len) =
            HandshakeSecret::client_answer(&server_hello, SECRET, 1, &mut wire).unwrap();
        let client_answer = wire[..len].to_vec();

        let mut wire = new_wire();
        let (_, len) =
            HandshakeSecret::server_finish(&client_answer, &challenge, SECRET, &mut wire).unwrap();
        let server_final = wire[..len].to_vec();

        HandshakeSecret::client_finish(&server_final, &cr, SECRET_BAD)
            .expect_err("final verify with a wrong secret must fail");
        HandshakeSecret::client_finish(&server_final, &cr, SECRET).unwrap();
    }

    /// A tampered final payload must fail verification, not panic.
    #[test]
    fn tampered_payloads_are_refused() {
        let mut wire = new_wire();
        let (challenge, len) = HandshakeSecret::server_challenge(SECRET, &mut wire).unwrap();
        let server_hello = wire[..len].to_vec();

        let mut wire = new_wire();
        let (cr, len) =
            HandshakeSecret::client_answer(&server_hello, SECRET, 1, &mut wire).unwrap();
        let client_answer = wire[..len].to_vec();

        let mut wire = new_wire();
        let (_, len) =
            HandshakeSecret::server_finish(&client_answer, &challenge, SECRET, &mut wire).unwrap();
        let mut server_final = wire[..len].to_vec();
        let last = server_final.len() - 1;
        server_final[last] ^= 0xFF;

        assert!(HandshakeSecret::client_finish(&server_final, &cr, SECRET).is_err());
    }
}
