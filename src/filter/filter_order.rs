use serde_json::Value;

use super::types::{FilterOrderInfo, SortDirection};
use super::error::FilterError;

pub struct FilterOrder;

impl FilterOrder {
    pub fn validate_and_parse(order: &Value) -> Result<Vec<FilterOrderInfo>, FilterError> {
        match order {
            Value::String(s) => Self::parse_order_string(s),
            Value::Array(arr) => {
                // Expect array of strings like ["created_at desc", "name asc"]
                let mut out = Vec::new();
                for v in arr {
                    if let Value::String(s) = v { out.extend(Self::parse_order_string(s)?); }
                }
                Ok(out)
            }
            Value::Object(obj) => {
                // { "created_at": "desc", "name": "asc" }
                let mut out = Vec::new();
                for (k, v) in obj {
                    let sort = match v.as_str().unwrap_or("asc").to_ascii_lowercase().as_str() {
                        "desc" => SortDirection::Desc,
                        _ => SortDirection::Asc,
                    };
                    out.push(FilterOrderInfo { column: k.clone(), sort });
                }
                Ok(out)
            }
            _ => Ok(vec![]),
        }
    }

    fn parse_order_string(s: &str) -> Result<Vec<FilterOrderInfo>, FilterError> {
        // split on commas, then each token into column and direction
        let mut out = Vec::new();
        for part in s.split(',') {
            let trimmed = part.trim();
            if trimmed.is_empty() { continue; }
            let mut it = trimmed.split_whitespace();
            if let Some(col) = it.next() {
                let dir = it.next().unwrap_or("asc");
                let sort = if dir.eq_ignore_ascii_case("desc") { SortDirection::Desc } else { SortDirection::Asc };
                out.push(FilterOrderInfo { column: col.to_string(), sort });
            }
        }
        Ok(out)
    }

    pub fn generate(infos: &[FilterOrderInfo]) -> Result<String, FilterError> {
        if infos.is_empty() { return Ok(String::new()); }
        let parts: Vec<String> = infos
            .iter()
            .map(|i| format!("\"{}\" {}", i.column, i.sort.to_sql()))
            .collect();
        Ok(format!("ORDER BY {}", parts.join(", ")))
    }
}
