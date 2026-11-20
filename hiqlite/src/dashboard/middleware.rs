use axum::http::{HeaderValue, header};
use std::sync::Arc;
use tower::ServiceBuilder;
use tower::layer::util::{Identity, Stack};
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
    // The API secret travels in the standard `Authorization` header, which every logger that
    // understands sensitive headers redacts by default. `COOKIE` covers the dashboard's own session.
    let sensitive_headers: Arc<[_]> = vec![header::AUTHORIZATION, header::COOKIE].into();

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
