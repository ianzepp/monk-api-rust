use crate::api::format::MetadataOptions;

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

/// Parse metadata options from query parameter
pub fn metadata_options_from_query(meta_param: Option<&str>) -> MetadataOptions {
    match meta_param {
        None => MetadataOptions::none(),
        Some("true") => MetadataOptions::all(),
        Some("false") | Some("") => MetadataOptions::none(),
        Some(param_value) => {
            // Parse comma-separated sections or dotted fields
            let mut opts = MetadataOptions::default();
            let mut specific = Vec::new();
            for part in param_value
                .split(',')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
            {
                match part {
                    "system" => opts.include_system = true,
                    "computed" => opts.include_computed = true,
                    "permissions" => opts.include_permissions = true,
                    "relationships" => opts.include_relationships = true,
                    "processing" => opts.include_processing = true,
                    other if other.contains('.') => specific.push(other.to_string()),
                    _ => {}
                }
            }
            if !specific.is_empty() {
                opts.specific_fields = Some(specific);
            }
            opts
        }
    }
}