/// Resolve tenant database from query parameter or environment variable
pub fn resolve_tenant_db(param: &Option<String>) -> Result<String, String> {
    if let Some(db) = param {
        return Ok(db.clone());
    }
    if let Ok(env_db) = std::env::var("MONK_TENANT_DB") {
        return Ok(env_db);
    }
    Err("tenant database not specified; provide ?tenant=tenant_<hash> or set MONK_TENANT_DB".to_string())
}