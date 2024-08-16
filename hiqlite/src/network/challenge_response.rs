use crate::{Error, NodeId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const SALT_LEN: usize = 24;

struct Salt(Vec<u8>);

impl Salt {
    pub fn new() -> Result<Self, Error> {
        let mut buf = [0u8; SALT_LEN];
        getrandom::getrandom(&mut buf).map_err(|_| Error::Error("getrandom Error".into()))?;
        Ok(Self(buf.to_vec()))
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Challenge(Vec<u8>);

impl Challenge {
    pub fn new() -> Result<Self, Error> {
        let salt = Salt::new()?;
        Ok(Self(salt.0))
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChallengeResponse {
    pub(crate) node_id: NodeId,
    challenge: Vec<u8>,
    response: Vec<u8>,
}

impl ChallengeResponse {
    pub fn new(node_id: NodeId, challenge: &Challenge, secret: &[u8]) -> Result<Self, Error> {
        let response = Sha256::new()
            .chain_update(&challenge.0)
            .chain_update(secret)
            .finalize()
            .to_vec();
        let challenge_new = Salt::new()?.0;

        Ok(Self {
            node_id,
            challenge: challenge_new,
            response,
        })
    }

    pub fn verify(&self, challenge: &Challenge, secret: &[u8]) -> Result<ResponseFinal, Error> {
        let verify = Sha256::new()
            .chain_update(&challenge.0)
            .chain_update(secret)
            .finalize()
            .to_vec();

        if self.response != verify {
            return Err(Error::BadRequest("Invalid ChallengeResponse".into()));
        }

        let response_new = Sha256::new()
            .chain_update(&self.challenge)
            .chain_update(secret)
            .finalize()
            .to_vec();

        Ok(ResponseFinal(response_new))
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResponseFinal(Vec<u8>);

impl ResponseFinal {
    pub fn verify(
        &self,
        challenge_response: &ChallengeResponse,
        secret: &[u8],
    ) -> Result<(), Error> {
        let verify = Sha256::new()
            .chain_update(&challenge_response.challenge)
            .chain_update(secret)
            .finalize()
            .to_vec();

        if self.0 != verify {
            Err(Error::BadRequest("Invalid ChallengeResponse".into()))
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::network::challenge_response::{Challenge, ChallengeResponse};

    #[test]
    fn test_challenge_response() {
        let secret = b"SuperMegaSecure1337";
        let secret_bad = b"SuperMegaSecure";

        let challenge = Challenge::new().unwrap();

        let challenge_response = ChallengeResponse::new(1, &challenge, secret.as_ref()).unwrap();

        assert!(challenge_response
            .verify(&challenge, secret_bad.as_ref())
            .is_err());
        let response = challenge_response
            .verify(&challenge, secret.as_ref())
            .unwrap();

        assert!(response
            .verify(&challenge_response, secret_bad.as_ref())
            .is_err());
        assert!(response
            .verify(&challenge_response, secret.as_ref())
            .is_ok());
    }
}
