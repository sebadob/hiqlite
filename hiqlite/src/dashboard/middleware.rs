use crate::network::HEADER_NAME_SECRET;
use axum::http::{header, HeaderName, HeaderValue};
use std::str::FromStr;
use std::sync::Arc;
use tower::layer::util::{Identity, Stack};
use tower::ServiceBuilder;
use tower_http::sensitive_headers::SetSensitiveRequestHeadersLayer;
use tower_http::set_header::SetResponseHeaderLayer;

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
                    Stack<SetSensitiveRequestHeadersLayer, Identity>,
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
        .layer(SetSensitiveRequestHeadersLayer::from_shared(
            sensitive_headers,
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::X_FRAME_OPTIONS,
            HeaderValue::from_static("SAMEORIGIN"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::STRICT_TRANSPORT_SECURITY,
            HeaderValue::from_static("max-age=31536000; includeSubDomains"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::REFERRER_POLICY,
            HeaderValue::from_static("no-referrer"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static("frame-ancestors 'none'; object-src 'none';"),
        ))
        .into_inner()
}
