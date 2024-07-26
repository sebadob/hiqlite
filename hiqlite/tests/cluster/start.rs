use crate::{log, TEST_DATA_DIR};
use cryptr::stream::s3::{
    AccessKeyId, AccessKeySecret, Bucket, BucketOptions, Credentials, Region,
};
use hiqlite::{start_node, DbClient, EncKeysFrom, Error, Node, NodeConfig, S3Config};
use std::env;
use std::time::Duration;
use tokio::{fs, task, time};

pub async fn start_test_cluster() -> Result<(DbClient, DbClient, DbClient), Error> {
    let handle_client_1 = task::spawn(start_node(build_config(1).await));
    let handle_client_2 = task::spawn(start_node(build_config(2).await));
    let handle_client_3 = task::spawn(start_node(build_config(3).await));

    let client_1 = handle_client_1.await??;
    let client_2 = handle_client_2.await??;
    let client_3 = handle_client_3.await??;

    Ok((client_1, client_2, client_3))
}

pub async fn build_config(node_id: u64) -> NodeConfig {
    let dir_1 = format!("{}/node_1", TEST_DATA_DIR);
    let dir_2 = format!("{}/node_2", TEST_DATA_DIR);
    let dir_3 = format!("{}/node_3", TEST_DATA_DIR);

    fs::create_dir_all(&dir_1).await.unwrap();
    fs::create_dir_all(&dir_2).await.unwrap();
    fs::create_dir_all(&dir_3).await.unwrap();

    let nodes = vec![
        Node {
            id: 1,
            addr_raft: "127.0.0.1:32001".to_string(),
            addr_api: "127.0.0.1:31001".to_string(),
        },
        Node {
            id: 2,
            addr_raft: "127.0.0.1:32002".to_string(),
            addr_api: "127.0.0.1:31002".to_string(),
        },
        Node {
            id: 3,
            addr_raft: "127.0.0.1:32003".to_string(),
            addr_api: "127.0.0.1:31003".to_string(),
        },
    ];
    let data_dir = match node_id {
        1 => dir_1.to_string().into(),
        2 => dir_2.to_string().into(),
        3 => dir_3.to_string().into(),
        _ => unreachable!(),
    };

    let s3_config = {
        dotenvy::dotenv().ok();
        if let Ok(url) = env::var("S3_URL") {
            // we assume that all values exist when we can read the url successfully

            let url = reqwest::Url::parse(&url).unwrap();
            let bucket_name = env::var("S3_BUCKET").unwrap();
            let region = Region(env::var("S3_REGION").unwrap());
            let access_key_id = AccessKeyId(env::var("S3_KEY").unwrap());
            let access_key_secret = AccessKeySecret(env::var("S3_SECRET").unwrap());
            let credentials = Credentials {
                access_key_id,
                access_key_secret,
            };
            let options = Some(BucketOptions {
                path_style: true,
                list_objects_v2: true,
            });

            let bucket = Bucket::new(url, bucket_name, region, credentials, options).unwrap();

            log("S3 env vars found");
            Some(S3Config { bucket })
        } else {
            log("No S3 env vars found - will skip S3 tests");
            None
        }
    };

    NodeConfig {
        node_id,
        nodes,
        data_dir,
        filename_db: "hiqlite".into(),
        log_statements: true,
        config: NodeConfig::raft_config(1000),
        // TODO currently we can't test with TLS, because this depends on `axum_server`.
        // This does not support graceful shutdown, which we need for testing from
        // a single process
        tls_raft: None,
        tls_api: None,
        secret_raft: "asdasdasdasdasdasd".to_string(),
        secret_api: "qweqweqweqweqweqwe".to_string(),
        enc_keys_from: EncKeysFrom::Env,
        s3_config,
    }
}

pub async fn wait_for_healthy_cluster(
    client_1: &DbClient,
    client_2: &DbClient,
    client_3: &DbClient,
) -> Result<(), Error> {
    for i in 1..=3 {
        loop {
            time::sleep(Duration::from_secs(1)).await;

            let client = match i {
                1 => client_1,
                2 => client_2,
                3 => client_3,
                _ => unreachable!(),
            };

            match client.is_healthy().await {
                Ok(_) => {
                    log(format!("Node {} is healthy", i));
                    break;
                }
                Err(err) => {
                    log(format!("Waiting for Node {} to become healthy: {}", i, err));
                }
            }
        }
    }

    let metrics = client_1.metrics_db().await?;
    assert!(metrics.running_state.is_ok());

    let node_count = metrics.membership_config.membership().nodes().count();
    assert_eq!(node_count, 3);

    Ok(())
}
