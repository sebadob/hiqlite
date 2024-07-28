use axum::body::Body;
use axum::extract::Request;
use axum::http::Uri;
use axum::{
    http::{header, Response, StatusCode},
    response,
};
use rust_embed::RustEmbed;
use std::borrow::Cow;
use tracing::{error, warn};

// cache lifetime in seconds -> 6 months
static CACHE_CTRL_VAL: &str = "max-age=15552000, public";

#[derive(RustEmbed)]
#[folder = "../dashboard/build"]
pub struct DashboardHtml;

pub async fn handler(uri: Uri, req: Request) -> response::Response {
    let (_, path) = uri.path().split_at(1); // split off the first `/`
    let mime = mime_guess::from_path(path);

    // if path.len() < 4 {
    //     warn!("path: {}", path);
    // }

    // skip encoding on already compressed data types
    let path_ending = &path[path.len().saturating_sub(4)..];
    let (path, encoding) = if path_ending == ".png"
        || path_ending == ".ico"
        || path_ending == ".jpg"
        || path_ending == ".svg"
        || path_ending == "jpeg"
    {
        (Cow::from(path), "none")
    } else {
        let accept_encoding = req
            .headers()
            .get("accept-encoding")
            .map(|h| h.to_str().unwrap_or("none"))
            .unwrap_or("none");
        if accept_encoding.contains("br") {
            (Cow::from(format!("{}.br", path)), "br")
        } else if accept_encoding.contains("gzip") {
            (Cow::from(format!("{}.gz", path)), "gzip")
        } else {
            (Cow::from(path), "none")
        }
    };

    match DashboardHtml::get(path.as_ref()) {
        Some(content) => Response::builder()
            .header(header::CACHE_CONTROL, CACHE_CTRL_VAL)
            .header(header::CONTENT_TYPE, mime.first_or_octet_stream().as_ref())
            .header(header::CONTENT_ENCODING, encoding)
            .body(Body::from(content.data))
            .unwrap(),

        None => {
            error!("Asset {} not found", path);
            for a in DashboardHtml::iter() {
                warn!("Available asset: {}", a);
            }
            Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from("not found"))
                .unwrap()
        }
    }
}
