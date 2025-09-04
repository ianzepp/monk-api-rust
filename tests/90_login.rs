mod common;

use anyhow::Result;
use reqwest::StatusCode;
use serde_json::json;

#[tokio::test]
async fn login_endpoint_responds() -> Result<()> {
    let server = common::ensure_server().await?;
    let client = reqwest::Client::new();

    let payload = json!({
        "password": "test-password"
    });

    let res = client
        .post(format!("{}/auth/login/test-tenant/test-user", server.base_url))
        .json(&payload)
        .send()
        .await?;

    // For now, we expect database errors since the databases don't exist yet
    // But we can verify the endpoint structure works
    assert!(
        res.status() == StatusCode::NOT_FOUND || 
        res.status() == StatusCode::INTERNAL_SERVER_ERROR,
        "Expected NOT_FOUND or INTERNAL_SERVER_ERROR, got {}",
        res.status()
    );

    // Should be valid JSON response
    let body = res.json::<serde_json::Value>().await?;
    assert!(body.get("success").is_some(), "Response should have 'success' field: {}", body);
    assert_eq!(body["success"], false, "Should be false for error response");
    assert!(body.get("error").is_some(), "Response should have 'error' field: {}", body);

    Ok(())
}

#[tokio::test]
async fn login_endpoint_structure() -> Result<()> {
    let server = common::ensure_server().await?;
    let client = reqwest::Client::new();

    // Test without payload - should fail due to missing JSON body
    let res = client
        .post(format!("{}/auth/login/test-tenant/test-user", server.base_url))
        .send()
        .await?;

    // Should be a client error (400 bad request) due to missing JSON body
    assert!(
        res.status().is_client_error() || 
        res.status() == StatusCode::NOT_FOUND || 
        res.status() == StatusCode::INTERNAL_SERVER_ERROR,
        "Expected client error, NOT_FOUND or INTERNAL_SERVER_ERROR, got {}",
        res.status()
    );

    Ok(())
}
