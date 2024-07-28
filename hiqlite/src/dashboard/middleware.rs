use crate::network::HEADER_NAME_SECRET;
use axum::http::{header, HeaderName, HeaderValue};
use std::str::FromStr;
use std::sync::Arc;
use tower::layer::util::{Identity, Stack};
use tower::ServiceBuilder;
use tower_http::sensitive_headers::{
    SetSensitiveRequestHeadersLayer, SetSensitiveResponseHeadersLayer,
};
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::ServiceBuilderExt;

type MiddlewareStack = Stack<
    SetResponseHeaderLayer<HeaderValue>,
    Stack<
        SetResponseHeaderLayer<HeaderValue>,
        Stack<
            SetResponseHeaderLayer<HeaderValue>,
            Stack<
                SetResponseHeaderLayer<HeaderValue>,
                Stack<
                    SetResponseHeaderLayer<HeaderValue>,
                    Stack<
                        SetSensitiveResponseHeadersLayer,
                        Stack<SetSensitiveRequestHeadersLayer, Identity>,
                    >,
                >,
            >,
        >,
    >,
>;

pub fn middleware() -> MiddlewareStack {
    let sensitive_headers: Arc<[_]> = vec![
        header::AUTHORIZATION,
        header::COOKIE,
        HeaderName::from_str(HEADER_NAME_SECRET).unwrap(),
    ]
    .into();

    ServiceBuilder::new()
        .sensitive_request_headers(sensitive_headers.clone())
        .sensitive_response_headers(sensitive_headers)
        .append_response_header(
            header::X_FRAME_OPTIONS,
            HeaderValue::from_static("SAMEORIGIN"),
        )
        .append_response_header(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        )
        .append_response_header(
            header::STRICT_TRANSPORT_SECURITY,
            HeaderValue::from_static("max-age=31536000; includeSubDomains"),
        )
        .append_response_header(
            header::REFERRER_POLICY,
            HeaderValue::from_static("no-referrer"),
        )
        .append_response_header(
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static("frame-ancestors 'none'; object-src 'none';"),
        )
        .into_inner()
}
