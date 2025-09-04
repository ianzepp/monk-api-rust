use crate::observer::stateful_record::{
    AccessLevel, ProcessingMetadata, RecordResponseMetadata, RelationshipMetadata, StatefulRecord,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use uuid::Uuid;

/// Options that control which metadata categories are included in the API response
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetadataOptions {
    pub include_system: bool,
    pub include_computed: bool,
    pub include_permissions: bool,
    pub include_relationships: bool,
    pub include_processing: bool,
    /// Specific fields to include (dot notation: "system.created_at", "permissions.can_edit")
    pub specific_fields: Option<Vec<String>>,
}

impl MetadataOptions {
    pub fn all() -> Self {
        Self {
            include_system: true,
            include_computed: true,
            include_permissions: true,
            include_relationships: true,
            include_processing: false,
            specific_fields: None,
        }
    }

    pub fn none() -> Self { Self::default() }

    pub fn should_include_any(&self) -> bool {
        self.include_system
            || self.include_computed
            || self.include_permissions
            || self.include_relationships
            || self.include_processing
    }
}

/// Convert a StatefulRecord into the public wire format
/// { id, type, attributes, relationships, meta, links }
pub fn record_to_api_value(
    record: &StatefulRecord,
    schema_type: &str,
    options: &MetadataOptions,
) -> Value {
    // Build attributes from record.modified, excluding system fields and id
    let attributes = build_attributes(record);

    // Build relationships from response metadata
    let relationships = if options.include_relationships {
        Some(build_relationships(&record.response_metadata.relationships))
    } else {
        None
    };

    // Build meta from response metadata according to options
    let mut meta_value = Value::Null;
    if options.should_include_any() {
        meta_value = build_meta(&record.response_metadata, options);
    }

    // Build links (self link if id present)
    let links = record.id.map(|id| {
        json!({
            "self": format!("/api/data/{}/{}", schema_type, id)
        })
    });

    let mut obj = Map::new();
    if let Some(id) = record.id { obj.insert("id".into(), Value::String(id.to_string())); }
    obj.insert("type".into(), Value::String(schema_type.to_string()));
    obj.insert("attributes".into(), Value::Object(attributes));
    if let Some(rel) = relationships { obj.insert("relationships".into(), rel); }
    if !meta_value.is_null() { obj.insert("meta".into(), meta_value); }
    if let Some(lnk) = links { obj.insert("links".into(), lnk); }

    Value::Object(obj)
}

/// Convert a list of records to API values
pub fn records_to_api_values(
    records: &[StatefulRecord],
    schema_type: &str,
    options: &MetadataOptions,
) -> Vec<Value> {
    records
        .iter()
        .map(|r| record_to_api_value(r, schema_type, options))
        .collect()
}

fn build_attributes(record: &StatefulRecord) -> Map<String, Value> {
    const SYSTEM_FIELDS: &[&str] = &[
        "id",
        "created_at",
        "updated_at",
        "deleted_at",
        "trashed_at",
        "access_read",
        "access_edit",
        "access_full",
        "access_deny",
        "version",
        "tenant_id",
    ];

    let mut attrs = Map::new();
    for (k, v) in &record.modified {
        if SYSTEM_FIELDS.contains(&k.as_str()) { continue; }
        attrs.insert(k.clone(), v.clone());
    }
    attrs
}

fn build_relationships(rel: &RelationshipMetadata) -> Value {
    let mut rel_obj = Map::new();

    for (schema, ids) in &rel.relationships {
        let data = Value::Array(
            ids.iter()
                .map(|id| json!({ "type": schema, "id": id }))
                .collect(),
        );
        let mut entry = Map::new();
        entry.insert("data".into(), data);
        if let Some(count) = rel.related_counts.get(schema) {
            entry.insert("meta".into(), json!({ "count": count }));
        }
        rel_obj.insert(schema.clone(), Value::Object(entry));
    }

    Value::Object(rel_obj)
}

fn build_meta(meta: &RecordResponseMetadata, options: &MetadataOptions) -> Value {
    let mut m = Map::new();

    if options.include_system {
        m.insert(
            "system".into(),
            json!({
                "created_at": meta.system.created_at.map(|dt| dt.to_rfc3339()),
                "updated_at": meta.system.updated_at.map(|dt| dt.to_rfc3339()),
                "trashed_at": meta.system.trashed_at.map(|dt| dt.to_rfc3339()),
                "deleted_at": meta.system.deleted_at.map(|dt| dt.to_rfc3339()),
                "access_read": meta.system.access_read,
                "access_edit": meta.system.access_edit,
                "access_full": meta.system.access_full,
                "access_deny": meta.system.access_deny,
                "version": meta.system.version,
                "tenant_id": meta.system.tenant_id,
            }),
        );
    }

    if options.include_computed {
m.insert("computed".into(), Value::Object(serde_json::Map::from_iter(meta.computed.clone())));
    }

    if options.include_permissions {
        m.insert(
            "permissions".into(),
            json!({
                "can_read": meta.permissions.can_read,
                "can_edit": meta.permissions.can_edit,
                "can_delete": meta.permissions.can_delete,
                "can_share": meta.permissions.can_share,
                "effective_access_level": format!("{:?}", meta.permissions.effective_access_level),
                "permission_source": meta.permissions.permission_source,
            }),
        );
    }

    if options.include_relationships {
        m.insert(
            "relationships".into(),
            json!({
                "related_counts": meta.relationships.related_counts,
            }),
        );
    }

    if options.include_processing {
        if let Some(stats) = &meta.processing.query_stats {
            m.insert(
                "processing".into(),
                json!({
                    "enriched_by": meta.processing.enriched_by,
                    "processing_time_ms": meta.processing.processing_time_ms,
                    "cache_hit": meta.processing.cache_hit,
                    "query_stats": {
                        "execution_time_ms": stats.execution_time_ms,
                        "rows_examined": stats.rows_examined,
                        "index_used": stats.index_used,
                    }
                }),
            );
        } else {
            m.insert(
                "processing".into(),
                json!({
                    "enriched_by": meta.processing.enriched_by,
                    "processing_time_ms": meta.processing.processing_time_ms,
                    "cache_hit": meta.processing.cache_hit,
                }),
            );
        }
    }

    let mut meta_value = Value::Object(m);

    // Apply field-specific filters if requested
    if let Some(fields) = &options.specific_fields {
        meta_value = filter_metadata_fields(meta_value, fields);
    }

    meta_value
}

fn filter_metadata_fields(metadata: Value, fields: &[String]) -> Value {
    if metadata.is_null() { return metadata; }
    let mut filtered = json!({});
    for field in fields {
        let parts: Vec<&str> = field.split('.').collect();
        if parts.len() == 2 {
            let category = parts[0];
            let field_name = parts[1];
            if let Some(category_data) = metadata.get(category) {
                if let Some(field_value) = category_data.get(field_name) {
                    if filtered.get(category).is_none() {
                        filtered[category] = json!({});
                    }
                    filtered[category][field_name] = field_value.clone();
                }
            }
        }
    }
    filtered
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::observer::stateful_record::{RecordOperation, RecordResponseMetadata, SystemMetadata};

    fn map(pairs: Vec<(&str, Value)>) -> Map<String, Value> {
        let mut m = Map::new();
        for (k, v) in pairs { m.insert(k.to_string(), v); }
        m
    }

    #[test]
    fn attributes_excludes_system_fields() {
        let mut rec = StatefulRecord::create(map(vec![
            ("id", Value::String("11111111-1111-1111-1111-111111111111".into())),
            ("name", Value::String("Alice".into())),
            ("created_at", Value::String("2024-01-01T00:00:00Z".into())),
        ]));
        rec.extract_system_metadata();
        let attrs = super::build_attributes(&rec);
        assert!(attrs.get("name").is_some());
        assert!(attrs.get("id").is_none());
        assert!(attrs.get("created_at").is_none());
    }

    #[test]
    fn meta_respects_options() {
        let mut rec = StatefulRecord::create(map(vec![
            ("name", Value::String("Alice".into())),
            ("created_at", Value::String("2024-01-01T00:00:00Z".into())),
        ]));
        rec.extract_system_metadata();
        rec.response_metadata.permissions.can_edit = true;
        rec.response_metadata.computed.insert("age_days".into(), Value::from(10));

        let mut opts = MetadataOptions::default();
        opts.include_system = true;
        opts.include_permissions = true;
        let v = record_to_api_value(&rec, "users", &opts);
        let meta = v.get("meta").unwrap();
        assert!(meta.get("system").is_some());
        assert!(meta.get("permissions").is_some());
        assert!(meta.get("computed").is_none());
    }
}
