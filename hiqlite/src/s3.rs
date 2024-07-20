use crate::Error;
use cryptr::stream::writer::s3_writer::{Bucket, Credentials, UrlStyle};
use cryptr::{EncValue, FileReader, FileWriter, S3Reader, S3Writer, StreamReader, StreamWriter};
use std::borrow::Cow;

#[derive(Debug, Clone)]
pub struct S3Config {
    bucket: Bucket,
    credentials: Credentials,
    danger_tls_no_verify: bool,
}

impl S3Config {
    pub fn new<C, S, U>(
        endpoint: U,
        path_style: UrlStyle,
        name: C,
        region: C,
        key: S,
        secret: S,
    ) -> Result<Self, Error>
    where
        C: Into<Cow<'static, str>>,
        S: Into<String>,
        U: Into<reqwest::Url>,
    {
        Ok(Self {
            bucket: Bucket::new(endpoint.into(), path_style, name, region)
                .map_err(|err| Error::S3(err.to_string()))?,
            credentials: Credentials::new(key, secret),
            danger_tls_no_verify: false,
        })
    }

    pub(crate) async fn push(&self, path: &str, object: &str) -> Result<(), Error> {
        let reader = StreamReader::File(FileReader {
            path,
            print_progress: false,
        });
        let writer = StreamWriter::S3(S3Writer {
            credentials: Some(&self.credentials),
            bucket: &self.bucket,
            object,
            danger_accept_invalid_certs: self.danger_tls_no_verify,
        });

        EncValue::encrypt_stream(reader, writer)
            .await
            .map_err(|err| Error::S3(err.to_string()))
    }

    pub(crate) async fn pull(&self, path: &str, object: &str) -> Result<(), Error> {
        let reader = StreamReader::S3(S3Reader {
            credentials: Some(&self.credentials),
            bucket: &self.bucket,
            object,
            danger_accept_invalid_certs: self.danger_tls_no_verify,
            print_progress: false,
        });
        let writer = StreamWriter::File(FileWriter {
            path,
            overwrite_target: true,
        });

        EncValue::decrypt_stream(reader, writer)
            .await
            .map_err(|err| Error::S3(err.to_string()))
    }
}
