//! gRPC API server (Tonic) implementing the ContentAPI service.

pub mod rate_limit;
pub mod service;
pub mod upload;

use std::{path::PathBuf, sync::Arc};

use anyhow::Result;
use tonic::transport::Server;

use crate::{
    config::CrapConfig,
    core::{
        Registry,
        email::EmailRenderer,
        event::EventBus,
        rate_limit::{GrpcRateLimiter, LoginRateLimiter},
    },
    db::DbPool,
    hooks::lifecycle::HookRunner,
};

/// Generated gRPC content service types.
pub mod content {
    tonic::include_proto!("crap");

    /// File descriptor set for gRPC reflection.
    pub const FILE_DESCRIPTOR_SET: &[u8] =
        tonic::include_file_descriptor_set!("content_descriptor");
}

/// Parameters for starting the gRPC API server.
pub struct GrpcStartParams {
    pub pool: DbPool,
    pub registry: Arc<Registry>,
    pub hook_runner: HookRunner,
    pub jwt_secret: String,
    pub config: CrapConfig,
    pub config_dir: PathBuf,
    pub event_bus: Option<EventBus>,
}

impl GrpcStartParams {
    /// Create a builder for `GrpcStartParams`.
    pub fn builder() -> GrpcStartParamsBuilder {
        GrpcStartParamsBuilder::new()
    }
}

/// Builder for [`GrpcStartParams`]. Created via [`GrpcStartParams::builder`].
pub struct GrpcStartParamsBuilder {
    pool: Option<DbPool>,
    registry: Option<Arc<Registry>>,
    hook_runner: Option<HookRunner>,
    jwt_secret: Option<String>,
    config: Option<CrapConfig>,
    config_dir: Option<PathBuf>,
    event_bus: Option<EventBus>,
}

impl GrpcStartParamsBuilder {
    fn new() -> Self {
        Self {
            pool: None,
            registry: None,
            hook_runner: None,
            jwt_secret: None,
            config: None,
            config_dir: None,
            event_bus: None,
        }
    }

    pub fn pool(mut self, pool: DbPool) -> Self {
        self.pool = Some(pool);
        self
    }

    pub fn registry(mut self, registry: Arc<Registry>) -> Self {
        self.registry = Some(registry);
        self
    }

    pub fn hook_runner(mut self, hook_runner: HookRunner) -> Self {
        self.hook_runner = Some(hook_runner);
        self
    }

    pub fn jwt_secret(mut self, jwt_secret: String) -> Self {
        self.jwt_secret = Some(jwt_secret);
        self
    }

    pub fn config(mut self, config: CrapConfig) -> Self {
        self.config = Some(config);
        self
    }

    pub fn config_dir(mut self, config_dir: PathBuf) -> Self {
        self.config_dir = Some(config_dir);
        self
    }

    pub fn event_bus(mut self, event_bus: Option<EventBus>) -> Self {
        self.event_bus = event_bus;
        self
    }

    pub fn build(self) -> GrpcStartParams {
        GrpcStartParams {
            pool: self.pool.expect("pool is required"),
            registry: self.registry.expect("registry is required"),
            hook_runner: self.hook_runner.expect("hook_runner is required"),
            jwt_secret: self.jwt_secret.expect("jwt_secret is required"),
            config: self.config.expect("config is required"),
            config_dir: self.config_dir.expect("config_dir is required"),
            event_bus: self.event_bus,
        }
    }
}

/// Start the gRPC server. Reflection is enabled by default but can be
/// disabled via `config.server.grpc_reflection`.
#[cfg(not(tarpaulin_include))]
pub async fn start_server(
    addr: &str,
    params: GrpcStartParams,
    shutdown: tokio_util::sync::CancellationToken,
) -> Result<()> {
    let addr = addr.parse()?;

    let email_renderer = Arc::new(EmailRenderer::new(&params.config_dir)?);
    let login_limiter = Arc::new(LoginRateLimiter::new(
        params.config.auth.max_login_attempts,
        params.config.auth.login_lockout_seconds,
    ));
    let forgot_password_limiter = Arc::new(LoginRateLimiter::new(
        params.config.auth.max_forgot_password_attempts,
        params.config.auth.forgot_password_window_seconds,
    ));

    let populate_cache_max_age = params.config.depth.populate_cache_max_age_secs;
    let grpc_rate_requests = params.config.server.grpc_rate_limit_requests;
    let grpc_rate_window = params.config.server.grpc_rate_limit_window;
    let grpc_reflection = params.config.server.grpc_reflection;
    let cors_layer = params.config.cors.build_layer();

    let content_service = service::ContentService::new(
        service::ContentServiceDeps::builder()
            .pool(params.pool)
            .registry(params.registry)
            .hook_runner(params.hook_runner)
            .jwt_secret(params.jwt_secret)
            .config(params.config)
            .config_dir(params.config_dir)
            .email_renderer(email_renderer)
            .event_bus(params.event_bus)
            .login_limiter(login_limiter)
            .forgot_password_limiter(forgot_password_limiter)
            .build(),
    );

    // Spawn periodic cache clear task for external DB mutation handling
    if populate_cache_max_age > 0
        && let Some(cache) = content_service.populate_cache_handle()
    {
        let interval_secs = populate_cache_max_age;
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(interval_secs));
            interval.tick().await; // skip first immediate tick
            loop {
                interval.tick().await;
                cache.clear();
            }
        });
    }

    let grpc_limiter = Arc::new(GrpcRateLimiter::new(grpc_rate_requests, grpc_rate_window));
    let rate_limit_layer = rate_limit::GrpcRateLimitLayer::new(grpc_limiter);

    let content_svc = content::content_api_server::ContentApiServer::new(content_service);

    // gRPC health service (grpc.health.v1.Health)
    let (health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter
        .set_serving::<content::content_api_server::ContentApiServer<service::ContentService>>()
        .await;

    let shutdown_signal = shutdown.cancelled_owned();

    let reflection_service = if grpc_reflection {
        Some(
            tonic_reflection::server::Builder::configure()
                .register_encoded_file_descriptor_set(content::FILE_DESCRIPTOR_SET)
                .build_v1()?,
        )
    } else {
        None
    };

    Server::builder()
        .layer(tower::util::option_layer(cors_layer))
        .layer(rate_limit_layer)
        .add_service(health_service)
        .add_optional_service(reflection_service)
        .add_service(content_svc)
        .serve_with_shutdown(addr, shutdown_signal)
        .await?;

    Ok(())
}
