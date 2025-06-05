use crate::Error;
use cryptr::{EncValue, FileReader, FileWriter, S3Reader, S3Writer, StreamReader, StreamWriter};
use std::env;
use std::sync::Arc;

pub use cryptr::stream::s3::*;
pub use cryptr::EncKeys;

#[derive(Debug, Clone)]
pub struct S3Config {
    pub bucket: Bucket,
}

impl S3Config {
    pub fn new<S>(
        endpoint: &str,
        bucket_name: S,
        region: S,
        key: S,
        secret: S,
        path_style: bool,
    ) -> Result<Arc<Self>, Error>
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
        let bucket = Bucket::new(endpoint, bucket_name.into(), region, credentials, options)
            .map_err(|err| Error::S3(err.to_string()))?;

        Ok(Arc::new(Self { bucket }))
    }

    pub fn try_from_env() -> Option<Arc<Self>> {
        if let Ok(url) = env::var("HQL_S3_URL") {
            // we assume that all values exist when we can read the url successfully

            let url = reqwest::Url::parse(&url).expect("Cannot parse HQL_S3_URL as URL");
            let bucket_name = env::var("HQL_S3_BUCKET").expect("HQL_S3_BUCKET not found");
            let region = Region(env::var("HQL_S3_REGION").expect("HQL_S3_REGION not found"));
            let path_style = env::var("HQL_S3_PATH_STYLE")
                .expect("HQL_S3_PATH_STYLE not found")
                .parse::<bool>()
                .expect("Cannot parse HQL_S3_PATH_STYLE as bool");

            let access_key_id = AccessKeyId(env::var("HQL_S3_KEY").expect("HQL_S3_KEY not found"));
            let access_key_secret =
                AccessKeySecret(env::var("HQL_S3_SECRET").expect("HQL_S3_SECRET not found"));
            let credentials = Credentials {
                access_key_id,
                access_key_secret,
            };

            let options = Some(BucketOptions {
                path_style,
                list_objects_v2: true,
            });

            let bucket = Bucket::new(url, bucket_name, region, credentials, options).unwrap();

            Some(Arc::new(S3Config { bucket }))
        } else {
            None
        }
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

    pub(crate) async fn pull(&self, object: &str, path: &str) -> Result<(), Error> {
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
