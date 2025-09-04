mod common;

use anyhow::Result;
use reqwest::StatusCode;

#[tokio::test]
async fn health_endpoint_responds() -> Result<()> {
    let server = common::ensure_server().await?;
    let client = reqwest::Client::new();

    let res = client
        .get(format!("{}/health", server.base_url))
        .send()
        .await?;

    // We consider OK or SERVICE_UNAVAILABLE acceptable as a basic liveness check
    assert!(
        res.status() == StatusCode::OK || res.status() == StatusCode::SERVICE_UNAVAILABLE,
        "unexpected status: {}",
        res.status()
    );

    // Should be valid JSON
    let _body = res.json::<serde_json::Value>().await?;
    Ok(())
}

