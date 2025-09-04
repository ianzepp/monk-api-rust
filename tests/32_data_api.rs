mod common;

use anyhow::Result;
use reqwest::StatusCode;

#[tokio::test]
async fn list_users_basic() -> Result<()> {
    let server = common::ensure_server().await?;
    let client = reqwest::Client::new();

    // Rely on server-side MONK_TENANT_DB from .env
    let res = client
        .get(format!("{}/api/data/users", server.base_url))
        .send()
        .await?;

    assert_eq!(res.status(), StatusCode::OK, "expected 200 OK, got {}", res.status());

    let body = res.json::<serde_json::Value>().await?;
    assert!(body.get("success").and_then(|v| v.as_bool()).unwrap_or(false), "success flag false or missing: {}", body);
    assert!(body.get("data").is_some(), "missing data field: {}", body);
    assert!(body.get("data").unwrap().is_array(), "data should be an array: {}", body);

    Ok(())
}

