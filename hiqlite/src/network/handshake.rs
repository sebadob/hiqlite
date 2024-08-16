use crate::network::challenge_response::{Challenge, ChallengeResponse, ResponseFinal};
use crate::{Error, NodeId};
use fastwebsockets::{Frame, OpCode, Payload, WebSocket};
use hyper::upgrade::Upgraded;
use hyper_util::rt::TokioIo;

pub struct HandshakeSecret;

impl HandshakeSecret {
    pub async fn client(
        ws: &mut WebSocket<TokioIo<Upgraded>>,
        secret: &[u8],
        node_id: NodeId,
    ) -> Result<(), Error> {
        let frame = ws.read_frame().await?;
        let challenge_response = match frame.opcode {
            OpCode::Binary => {
                let bytes = frame.payload.as_ref();
                let challenge: Challenge = bincode::deserialize(bytes)?;
                ChallengeResponse::new(node_id, &challenge, secret)?
            }
            _ => {
                return Err(Error::BadRequest("Invalid Challenge from Server".into()));
            }
        };

        let frame = Frame::binary(Payload::from(
            bincode::serialize(&challenge_response).unwrap(),
        ));
        ws.write_frame(frame).await?;

        let frame = ws.read_frame().await?;
        match frame.opcode {
            OpCode::Binary => {
                let bytes = frame.payload.as_ref();
                let response: ResponseFinal = bincode::deserialize(bytes)?;
                response.verify(&challenge_response, secret)?;
            }
            _ => {
                return Err(Error::BadRequest(
                    "Invalid ResponseFinal from Server".into(),
                ));
            }
        };

        Ok(())
    }

    pub(crate) async fn server(
        ws: &mut WebSocket<TokioIo<Upgraded>>,
        secret: &[u8],
    ) -> Result<NodeId, Error> {
        let challenge = Challenge::new()?;

        let frame = Frame::binary(Payload::from(bincode::serialize(&challenge).unwrap()));
        ws.write_frame(frame).await?;

        // we are not using a fragment collector and don't check for a full frame either
        // it should never be an issue though because the handshake packets are tiny
        let frame = ws.read_frame().await?;
        let (node_id, response) = match frame.opcode {
            OpCode::Binary => {
                let bytes = frame.payload.as_ref();
                let challenge_response: ChallengeResponse = bincode::deserialize(bytes)?;
                let resp = challenge_response.verify(&challenge, secret)?;
                (challenge_response.node_id, resp)
            }
            _ => {
                return Err(Error::BadRequest(
                    "Invalid ChallengeResponse from Client".into(),
                ));
            }
        };

        let frame = Frame::binary(Payload::from(bincode::serialize(&response).unwrap()));
        ws.write_frame(frame).await?;

        Ok(node_id)
    }
}
