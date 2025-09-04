mod common;

use anyhow::Result;
use reqwest::StatusCode;

// These tests verify basic filter surface: select, order, limit
// They rely on the built-in `users` table in the tenant specified by MONK_TENANT_DB.

#[tokio::test]
async fn select_id_only_returns_empty_attributes() -> Result<()> {
    let server = common::ensure_server().await?;
    let client = reqwest::Client::new();

    let body = serde_json::json!({
        "select": ["id"],
        // no where/order/limit
    });

    let res = client
        .post(format!("{}/api/find/users?meta=system", server.base_url))
        .json(&body)
        .send()
        .await?;

    assert_eq!(res.status(), StatusCode::OK, "unexpected status: {}", res.status());

    let payload = res.json::<serde_json::Value>().await?;
    assert!(payload["success"].as_bool().unwrap_or(false), "success=false: {}", payload);
    let data = payload["data"].as_array().cloned().unwrap_or_default();

    // For any returned records, attributes should be empty because only id was selected (id is top-level, not in attributes)
    for rec in data.iter() {
        assert!(rec.get("id").is_some(), "record missing id: {}", rec);
        let attrs = rec.get("attributes").and_then(|v| v.as_object()).cloned().unwrap_or_default();
        assert!(attrs.is_empty(), "expected empty attributes; got: {:?}", attrs);
    }

    Ok(())
}

#[tokio::test]
async fn order_by_created_at_desc() -> Result<()> {
    let server = common::ensure_server().await?;
    let client = reqwest::Client::new();

    // Request system metadata so we can check created_at ordering
    let body = serde_json::json!({
        "order": "created_at desc",
        "limit": 10
    });

    let res = client
        .post(format!("{}/api/find/users?meta=system", server.base_url))
        .json(&body)
        .send()
        .await?;

    assert_eq!(res.status(), StatusCode::OK, "unexpected status: {}", res.status());

    let payload = res.json::<serde_json::Value>().await?;
    assert!(payload["success"].as_bool().unwrap_or(false), "success=false: {}", payload);
    let data = payload["data"].as_array().cloned().unwrap_or_default();

    // If we have at least two records, verify non-increasing created_at order
    if data.len() >= 2 {
        let mut prev: Option<String> = None;
        for rec in data.iter() {
            let created = rec
                .get("meta")
                .and_then(|m| m.get("system"))
                .and_then(|s| s.get("created_at"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if let Some(p) = prev.as_ref() {
                // RFC3339 strings sort lexicographically in chronological order
                assert!(p >= &created, "expected descending created_at: prev={}, curr={}", p, created);
            }
            prev = Some(created);
        }
    }

    Ok(())
}

#[tokio::test]
async fn limit_two_records() -> Result<()> {
    let server = common::ensure_server().await?;
    let client = reqwest::Client::new();

    let body = serde_json::json!({
        "limit": 2
    });

    let res = client
        .post(format!("{}/api/find/users", server.base_url))
        .json(&body)
        .send()
        .await?;

    assert_eq!(res.status(), StatusCode::OK, "unexpected status: {}", res.status());

    let payload = res.json::<serde_json::Value>().await?;
    assert!(payload["success"].as_bool().unwrap_or(false), "success=false: {}", payload);
    let data = payload["data"].as_array().cloned().unwrap_or_default();
    assert!(data.len() <= 2, "expected <= 2 records, got {}", data.len());

    Ok(())
}

