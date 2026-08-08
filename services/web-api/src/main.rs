use std::{env, path::PathBuf};

use luna_web_api::{app, WebConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .json()
        .init();

    let config = WebConfig {
        data_dir: env::var_os("LUNA_WEB_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(".luna/web")),
        household_id: env_or("LUNA_DEV_HOUSEHOLD_ID", "local-household"),
        member_id: env_or("LUNA_DEV_MEMBER_ID", "local-member"),
        member_display_name: env_or("LUNA_DEV_MEMBER_NAME", "Luna member"),
        household_name: env_or("LUNA_DEV_HOUSEHOLD_NAME", "Luna household"),
    };
    let router = app(config)?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:8787").await?;
    tracing::info!(address = %listener.local_addr()?, "Luna web backend is ready");
    axum::serve(listener, router).await?;
    Ok(())
}

fn env_or(name: &str, fallback: &str) -> String {
    env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| fallback.to_owned())
}
