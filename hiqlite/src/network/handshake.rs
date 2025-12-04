use crate::helpers::deserialize;
use crate::network::challenge_response::{Challenge, ChallengeResponse, ResponseFinal};
use crate::network::serialize_network;
use crate::{Error, NodeId};
use fastwebsockets::{Frame, OpCode, Payload, WebSocket};
use hyper::upgrade::Upgraded;
use hyper_util::rt::TokioIo;
use tracing::info;

pub struct HandshakeSecret;

impl HandshakeSecret {
    pub async fn client(
        ws: &mut WebSocket<TokioIo<Upgraded>>,
        secret: &[u8],
        node_id: NodeId,
    ) -> Result<(), Error> {
        info!("Executing HandshakeSecret::client");
        let frame = ws.read_frame().await?;
        let challenge_response = match frame.opcode {
            OpCode::Binary => {
                let bytes = frame.payload.as_ref();
                let challenge: Challenge = deserialize(bytes)?;
                ChallengeResponse::new(node_id, &challenge, secret)?
            }
            _ => {
                return Err(Error::BadRequest("Invalid Challenge from Server".into()));
            }
        };

        let frame = Frame::binary(Payload::from(serialize_network(&challenge_response)));
        ws.write_frame(frame).await?;

        let frame = ws.read_frame().await?;
        match frame.opcode {
            OpCode::Binary => {
                let bytes = frame.payload.as_ref();
                let response: ResponseFinal = deserialize(bytes)?;
                response.verify(&challenge_response, secret)?;
            }
            _ => {
                return Err(Error::BadRequest(
                    "Invalid ResponseFinal from Server".into(),
                ));
            }
        };

        info!("HandshakeSecret::client finished");
        Ok(())
    }

    pub(crate) async fn server(
        ws: &mut WebSocket<TokioIo<Upgraded>>,
        secret: &[u8],
    ) -> Result<NodeId, Error> {
        info!("Executing HandshakeSecret::server");
        let challenge = Challenge::new()?;

        let frame = Frame::binary(Payload::from(serialize_network(&challenge)));
        ws.write_frame(frame).await?;

        // we are not using a fragment collector and don't check for a full frame either
        // it should never be an issue though because the handshake packets are tiny
        let frame = ws.read_frame().await?;
        let (node_id, response) = match frame.opcode {
            OpCode::Binary => {
                let bytes = frame.payload.as_ref();
                let challenge_response: ChallengeResponse = deserialize(bytes)?;
                let resp = challenge_response.verify(&challenge, secret)?;
                (challenge_response.node_id, resp)
            }
            _ => {
                return Err(Error::BadRequest(
                    "Invalid ChallengeResponse from Client".into(),
                ));
            }
        };

        let frame = Frame::binary(Payload::from(serialize_network(&response)));
        ws.write_frame(frame).await?;

        info!("HandshakeSecret::server finished");
        Ok(node_id)
    }
}
