use crate::config::EncKeysFrom;
use crate::Error;
use cryptr::stream::s3::*;
use cryptr::{
    EncKeys, EncValue, FileReader, FileWriter, S3Reader, S3Writer, StreamReader, StreamWriter,
};
use tracing::warn;

#[derive(Debug, Clone)]
pub struct S3Config {
    pub bucket: Bucket,
}

impl S3Config {
    pub fn new<S>(
        endpoint: &str,
        name: S,
        region: S,
        key: S,
        secret: S,
        path_style: bool,
    ) -> Result<Self, Error>
    where
        S: Into<String>,
    {
        let endpoint = reqwest::Url::parse(endpoint).map_err(|err| Error::S3(err.to_string()))?;
        let region = Region(region.into());
        let credentials = Credentials {
            access_key_id: AccessKeyId(key.into()),
            access_key_secret: AccessKeySecret(secret.into()),
        };
        let options = Some(BucketOptions {
            path_style,
            list_objects_v2: true,
        });
        let bucket = Bucket::new(endpoint, name.into(), region, credentials, options)
            .map_err(|err| Error::S3(err.to_string()))?;

        Ok(Self { bucket })
    }

    pub(crate) async fn push(&self, path: &str, object: &str) -> Result<(), Error> {
        let reader = StreamReader::File(FileReader {
            path,
            print_progress: false,
        });
        let writer = StreamWriter::S3(S3Writer {
            bucket: &self.bucket,
            object,
        });

        EncValue::encrypt_stream(reader, writer)
            .await
            .map_err(|err| Error::S3(err.to_string()))
    }

    pub(crate) async fn pull(&self, path: &str, object: &str) -> Result<(), Error> {
        let reader = StreamReader::S3(S3Reader {
            bucket: &self.bucket,
            object,
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

pub fn init_enc_keys(enc_keys_from: &EncKeysFrom) -> Result<(), Error> {
    if let Err(err) = match enc_keys_from {
        EncKeysFrom::Env => EncKeys::from_env(),
        EncKeysFrom::File(path) => EncKeys::read_from_file(path),
    }
    .map_err(|err| Error::Error(err.to_string().into()))?
    .init()
    {
        warn!("{}", err);
    };
    Ok(())
}
