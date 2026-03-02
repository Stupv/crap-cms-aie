//! Per-IP gRPC rate limiting as a tower Layer/Service.

use std::sync::Arc;
use std::task::{Context, Poll};

use tower::{Layer, Service};

use crate::core::rate_limit::GrpcRateLimiter;

/// Tower layer that applies per-IP rate limiting to gRPC requests.
#[derive(Clone)]
pub struct GrpcRateLimitLayer {
    limiter: Arc<GrpcRateLimiter>,
}

impl GrpcRateLimitLayer {
    pub fn new(limiter: Arc<GrpcRateLimiter>) -> Self {
        Self { limiter }
    }
}

impl<S> Layer<S> for GrpcRateLimitLayer {
    type Service = GrpcRateLimitService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        GrpcRateLimitService {
            inner,
            limiter: self.limiter.clone(),
        }
    }
}

/// Tower service that checks the per-IP rate limit before forwarding requests.
#[derive(Clone)]
pub struct GrpcRateLimitService<S> {
    inner: S,
    limiter: Arc<GrpcRateLimiter>,
}

impl<S, ReqBody, ResBody> Service<axum::http::Request<ReqBody>> for GrpcRateLimitService<S>
where
    S: Service<axum::http::Request<ReqBody>, Response = axum::http::Response<ResBody>>
        + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
    ResBody: Default + Send + 'static,
    ReqBody: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>,
    >;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: axum::http::Request<ReqBody>) -> Self::Future {
        // Extract IP from TcpConnectInfo (set by tonic's TCP acceptor).
        let ip = req
            .extensions()
            .get::<tonic::transport::server::TcpConnectInfo>()
            .and_then(|info| info.remote_addr())
            .map(|addr| addr.ip().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        if !self.limiter.check_and_record(&ip) {
            let status = tonic::Status::resource_exhausted("rate limit exceeded");
            let response = status.into_http();
            return Box::pin(async move { Ok(response) });
        }

        let mut inner = self.inner.clone();
        Box::pin(async move { inner.call(req).await })
    }
}
