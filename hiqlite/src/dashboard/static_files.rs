use axum::body::Body;
use axum::extract::Request;
use axum::http::Uri;
use axum::{
    http::{header, Response, StatusCode},
    response,
};
use rust_embed::RustEmbed;
use std::borrow::Cow;
use tracing::debug;

// cache lifetime in seconds -> 6 months
static CACHE_CTRL_VAL: &str = "max-age=15552000, public";

#[derive(RustEmbed)]
#[folder = "static"]
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
            (Cow::from(format!("{path}.br")), "br")
        } else if accept_encoding.contains("gzip") {
            (Cow::from(format!("{path}.gz")), "gzip")
        } else {
            (Cow::from(path), "none")
        }
    };

    let cache_ctrl = if path.starts_with("_app/") {
        CACHE_CTRL_VAL
    } else {
        "max-age=3600, public"
    };

    match DashboardHtml::get(path.as_ref()) {
        Some(content) => Response::builder()
            .header(header::CACHE_CONTROL, cache_ctrl)
            .header(header::CONTENT_TYPE, mime.first_or_octet_stream().as_ref())
            .header(header::CONTENT_ENCODING, encoding)
            .body(Body::from(content.data))
            .unwrap(),

        None => {
            debug!("Asset {path} not found");
            // for a in DashboardHtml::iter() {
            //     warn!("Available asset: {}", a);
            // }
            Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from("not found"))
                .unwrap()
        }
    }
}
