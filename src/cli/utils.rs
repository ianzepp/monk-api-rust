use serde_json::{json, Value};
use crate::cli::OutputFormat;

/// Output a success message in the appropriate format
pub fn output_success(
    output_format: &OutputFormat,
    message: &str,
    data: Option<Value>,
) -> anyhow::Result<()> {
    match output_format {
        OutputFormat::Json => {
            let mut response = json!({
                "success": true,
                "message": message
            });
            
            if let Some(data_value) = data {
                response.as_object_mut().unwrap().extend(
                    data_value.as_object().unwrap().clone()
                );
            }
            
            println!("{}", serde_json::to_string_pretty(&response)?);
        }
        OutputFormat::Text => {
            println!("✓ {}", message);
        }
    }
    Ok(())
}

/// Output an error message in the appropriate format  
pub fn output_error(
    output_format: &OutputFormat,
    message: &str,
    error_code: Option<&str>,
) -> anyhow::Result<()> {
    match output_format {
        OutputFormat::Json => {
            let mut response = json!({
                "success": false,
                "error": message
            });
            
            if let Some(code) = error_code {
                response["error_code"] = json!(code);
            }
            
            println!("{}", serde_json::to_string_pretty(&response)?);
        }
        OutputFormat::Text => {
            eprintln!("Error: {}", message);
        }
    }
    Ok(())
}

/// Output an empty collection in the appropriate format
pub fn output_empty_collection(
    output_format: &OutputFormat,
    collection_name: &str,
    message: &str,
) -> anyhow::Result<()> {
    match output_format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&json!({
                collection_name: []
            }))?);
        }
        OutputFormat::Text => {
            println!("{}", message);
        }
    }
    Ok(())
}

/// Output current item information in the appropriate format
pub fn output_current_item(
    output_format: &OutputFormat,
    item_type: &str,
    name: &str,
    details: Value,
) -> anyhow::Result<()> {
    match output_format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&json!({
                format!("current_{}", item_type): details
            }))?);
        }
        OutputFormat::Text => {
            println!("Current {}: {}", item_type, name);
            // Extract and display key details for text format
            if let Some(url) = details.get("url") {
                if let Some(url_str) = url.as_str() {
                    println!("URL: {}", url_str);
                }
            }
            if let Some(server) = details.get("server") {
                if let Some(server_str) = server.as_str() {
                    println!("Server: {}", server_str);
                }
            }
            if let Some(desc) = details.get("description") {
                if let Some(desc_str) = desc.as_str() {
                    if !desc_str.is_empty() {
                        println!("Description: {}", desc_str);
                    }
                }
            }
        }
    }
    Ok(())
}

/// Output "no current item" message in the appropriate format
pub fn output_no_current_item(
    output_format: &OutputFormat,
    item_type: &str,
) -> anyhow::Result<()> {
    match output_format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&json!({
                format!("current_{}", item_type): null
            }))?);
        }
        OutputFormat::Text => {
            println!("No current {} set", item_type);
        }
    }
    Ok(())
}

/// Generic function to handle switching between items (server/tenant)
pub fn switch_current_item<F, G>(
    item_name: &str,
    item_type: &str,
    check_exists: F,
    update_current: G,
    output_format: &OutputFormat,
) -> anyhow::Result<()>
where
    F: Fn(&str) -> anyhow::Result<bool>,
    G: Fn(&str) -> anyhow::Result<()>,
{
    if !check_exists(item_name)? {
        return Err(anyhow::anyhow!("{} '{}' not found", 
            item_type.chars().next().unwrap().to_uppercase().collect::<String>() + &item_type[1..],
            item_name
        ));
    }
    
    update_current(item_name)?;
    
    output_success(
        output_format,
        &format!("Switched to {} '{}'", item_type, item_name),
        Some(json!({ format!("current_{}", item_type): item_name })),
    )?;
    
    Ok(())
}

/// Generic function to handle deleting items and clearing current if needed
pub fn delete_item_with_current_check<F, G, H>(
    item_name: &str,
    item_type: &str,
    check_exists: F,
    remove_item: G,
    clear_if_current: H,
    output_format: &OutputFormat,
) -> anyhow::Result<()>
where
    F: Fn(&str) -> anyhow::Result<bool>,
    G: Fn(&str) -> anyhow::Result<()>,
    H: Fn(&str) -> anyhow::Result<()>,
{
    if !check_exists(item_name)? {
        return Err(anyhow::anyhow!("{} '{}' not found",
            item_type.chars().next().unwrap().to_uppercase().collect::<String>() + &item_type[1..],
            item_name
        ));
    }
    
    remove_item(item_name)?;
    clear_if_current(item_name)?;
    
    output_success(
        output_format,
        &format!("{} '{}' deleted successfully",
            item_type.chars().next().unwrap().to_uppercase().collect::<String>() + &item_type[1..],
            item_name
        ),
        None,
    )?;
    
    Ok(())
}

/// Extract target item name from optional parameter or use current
pub fn resolve_target_item(
    provided_name: Option<String>,
    current_getter: impl Fn() -> anyhow::Result<Option<String>>,
    item_type: &str,
) -> anyhow::Result<String> {
    match provided_name {
        Some(name) => Ok(name),
        None => {
            match current_getter()? {
                Some(current) => Ok(current),
                None => Err(anyhow::anyhow!("No current {} set", item_type)),
            }
        }
    }
}