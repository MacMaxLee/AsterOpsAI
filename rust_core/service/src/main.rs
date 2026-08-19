use std::sync::Arc;

use service::{api, self_metrics, state::AppState};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let platform: Arc<dyn platform::PlatformAdapter> =
        Arc::from(platform::current_platform_adapter());
    let self_metrics = self_metrics::spawn(platform.clone());
    let state = AppState::new(platform, self_metrics);
    let app = api::router(state);

    #[cfg(unix)]
    {
        let socket_path = service::transport::unix::resolve_socket_path()?;
        service::transport::unix::serve(app, &socket_path).await?;
    }

    #[cfg(windows)]
    {
        service::transport::windows::serve(app).await?;
    }

    Ok(())
}
