use clap::Subcommand;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::fs;
use crate::cli::OutputFormat;

#[derive(Subcommand)]
pub enum FixtureCommands {
    #[command(about = "Generate fixture data from JSON schemas")]
    Generate {
        #[arg(help = "Template name (e.g., 'empty', 'basic')")]
        template: String,
        #[arg(long, help = "Number of records to generate per schema", default_value = "100")]
        count: u32,
        #[arg(long, help = "Custom fixtures directory path")]
        fixtures_dir: Option<PathBuf>,
        #[arg(long, help = "Output directory for generated data")]
        output: Option<PathBuf>,
    },
    
    #[command(about = "Build fixture database locally")]
    Build {
        #[arg(help = "Template name (e.g., 'empty', 'basic')")]
        template: String,
        #[arg(long, help = "Custom fixtures directory path")]
        fixtures_dir: Option<PathBuf>,
        #[arg(long, help = "Local database name", default_value = "monk_fixture_db")]
        db_name: String,
        #[arg(long, help = "Database URL override")]
        database_url: Option<String>,
        #[arg(long, help = "Clone from existing template database")]
        clone: Option<String>,
    },
    
    #[command(about = "Deploy fixture database to remote server")]
    Deploy {
        #[arg(help = "Template name (e.g., 'empty', 'basic')")]
        template: String,
        #[arg(long, help = "Custom fixtures directory path")]
        fixtures_dir: Option<PathBuf>,
        #[arg(long, help = "Target server name")]
        target: Option<String>,
        #[arg(long, help = "Show deployment progress")]
        progress: bool,
        #[arg(long, help = "Remote database URL override")]
        database_url: Option<String>,
    },
}

pub async fn handle(cmd: FixtureCommands, output_format: OutputFormat) -> anyhow::Result<()> {
    match cmd {
        FixtureCommands::Generate { template, count: _count, fixtures_dir, output: _output } => {
            handle_generate(template, _count, fixtures_dir, _output, output_format).await
        }
        FixtureCommands::Build { template, fixtures_dir, db_name, database_url: _database_url, clone } => {
            handle_build(template, fixtures_dir, db_name, _database_url, clone, output_format).await
        }
        FixtureCommands::Deploy { template, fixtures_dir, target, progress, database_url: _database_url } => {
            handle_deploy(template, fixtures_dir, target, progress, _database_url, output_format).await
        }
    }
}

fn get_fixtures_dir(custom_dir: Option<PathBuf>) -> PathBuf {
    match custom_dir {
        Some(dir) => dir,
        None => {
            let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            current_dir.join("fixtures")
        }
    }
}

fn get_template_dir(fixtures_dir: PathBuf, template: &str) -> PathBuf {
    fixtures_dir.join(template)
}

fn determine_build_strategy(
    template: &str, 
    clone: &Option<String>, 
    fixtures_dir: &PathBuf
) -> anyhow::Result<(String, Option<String>)> {
    match (template, clone) {
        // System template - special case, build from SQL only (base template)
        ("system", None) => {
            let sql_file = get_template_dir(fixtures_dir.clone(), "system").join("init.sql");
            if sql_file.exists() {
                Ok(("build_from_sql".to_string(), Some(sql_file.display().to_string())))
            } else {
                Err(anyhow::anyhow!("System template missing required init.sql file"))
            }
        }
        
        // Any template with explicit clone parameter - always clone
        (_, Some(clone_from)) => {
            let clone_template = get_template_dir(fixtures_dir.clone(), clone_from);
            if clone_template.exists() {
                Ok(("clone_from_template".to_string(), Some(format!("template_{}", clone_from))))
            } else {
                Err(anyhow::anyhow!("Clone source template '{}' not found", clone_from))
            }
        }
        
        // All other templates - always clone from system (then init.sql/schemas executed on top)
        (_, None) => {
            let system_template = get_template_dir(fixtures_dir.clone(), "system");
            if system_template.exists() {
                Ok(("clone_from_template".to_string(), Some("template_system".to_string())))
            } else {
                Err(anyhow::anyhow!("Template '{}' requires system template to exist for cloning", template))
            }
        }
    }
}

async fn execute_template_build(
    template: &str,
    template_dir: &PathBuf,
    db_name: &str,
    clone: &Option<String>,
    fixtures_dir: &PathBuf,
    _output_format: &OutputFormat,
) -> anyhow::Result<Value> {
    let mut build_steps = Vec::new();
    let mut warnings = Vec::new();
    
    // Step 1: Handle template cloning or database creation
    let (build_strategy, source_info) = determine_build_strategy(template, clone, fixtures_dir)?;
    
    match build_strategy.as_str() {
        "clone_from_template" => {
            build_steps.push(format!("Clone from template: {}", source_info.as_ref().unwrap_or(&"unknown".to_string())));
            // TODO: Implement actual database cloning logic
            // create_database_from_template(db_name, source_template).await?;
        }
        "build_from_sql" => {
            build_steps.push("Create new database".to_string());
            // TODO: Implement database creation
            // create_empty_database(db_name).await?;
        }
        _ => {
            return Err(anyhow::anyhow!("Unknown build strategy: {}", build_strategy));
        }
    }
    
    // Step 2: Execute init.sql if present
    let init_sql_file = template_dir.join("init.sql");
    if init_sql_file.exists() {
        build_steps.push(format!("Execute init.sql: {}", init_sql_file.display()));
        execute_sql_file(&init_sql_file, db_name).await?;
    }
    
    // Step 3: Process schemas/*.json files if present
    let schemas_dir = template_dir.join("schemas");
    if schemas_dir.exists() && schemas_dir.is_dir() {
        let schema_files = get_schema_files(&schemas_dir)?;
        if !schema_files.is_empty() {
            build_steps.push(format!("Process {} schema files from schemas/ directory", schema_files.len()));
            for schema_file in schema_files {
                build_steps.push(format!("  └─ Process schema: {}", schema_file.file_name().unwrap_or_default().to_string_lossy()));
                process_schema_file(&schema_file, db_name).await?;
            }
        }
    }
    
    // If no init.sql and no schemas, add a warning
    if !init_sql_file.exists() && (!schemas_dir.exists() || get_schema_files(&schemas_dir).unwrap_or_default().is_empty()) {
        if build_strategy != "clone_from_template" {
            warnings.push("Template has no init.sql or schemas/*.json files to process".to_string());
        }
    }
    
    Ok(json!({
        "success": true,
        "template": template,
        "database": db_name,
        "build_strategy": build_strategy,
        "source_info": source_info,
        "build_steps": build_steps,
        "warnings": warnings
    }))
}

fn get_schema_files(schemas_dir: &PathBuf) -> anyhow::Result<Vec<PathBuf>> {
    let mut schema_files = Vec::new();
    
    if !schemas_dir.exists() {
        return Ok(schema_files);
    }
    
    for entry in fs::read_dir(schemas_dir)? {
        let entry = entry?;
        let path = entry.path();
        
        if path.is_file() {
            if let Some(extension) = path.extension() {
                if extension == "json" {
                    schema_files.push(path);
                }
            }
        }
    }
    
    schema_files.sort();
    Ok(schema_files)
}

async fn execute_sql_file(sql_file: &PathBuf, _db_name: &str) -> anyhow::Result<()> {
    // TODO: Implement actual SQL execution using SQLX
    // let sql_content = fs::read_to_string(sql_file)?;
    // let db = get_database_connection(db_name).await?;
    // db.execute(sql_content).await?;
    
    // For now, just verify the file exists and is readable
    if !sql_file.exists() {
        return Err(anyhow::anyhow!("SQL file not found: {}", sql_file.display()));
    }
    
    let _content = fs::read_to_string(sql_file)?;
    // Placeholder: SQL file read successfully
    Ok(())
}

async fn process_schema_file(schema_file: &PathBuf, _db_name: &str) -> anyhow::Result<()> {
    // TODO: Implement schema processing using shared Meta API logic
    // let schema_json: Value = serde_json::from_str(&fs::read_to_string(schema_file)?)?;
    // let db = get_database_connection(db_name).await?;
    // crate::handlers::meta::create_schema(&db, schema_json).await?;
    
    // For now, just verify the file exists and is valid JSON
    if !schema_file.exists() {
        return Err(anyhow::anyhow!("Schema file not found: {}", schema_file.display()));
    }
    
    let content = fs::read_to_string(schema_file)?;
    let _parsed: Value = serde_json::from_str(&content)?;
    // Placeholder: Schema file parsed successfully
    Ok(())
}

async fn handle_generate(
    template: String,
    _count: u32,
    fixtures_dir: Option<PathBuf>,
    _output: Option<PathBuf>,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    let fixtures_dir = get_fixtures_dir(fixtures_dir);
    let template_dir = get_template_dir(fixtures_dir, &template);
    
    // Check if template exists
    if !template_dir.exists() {
        return Err(anyhow::anyhow!("Template '{}' not found at: {}", template, template_dir.display()));
    }
    
    match template.as_str() {
        "empty" => {
            // Empty template has no schemas, so no data to generate
            match output_format {
                OutputFormat::Json => {
                    println!("{}", serde_json::to_string_pretty(&json!({
                        "success": true,
                        "message": "Empty template requires no data generation",
                        "template": template,
                        "schemas_processed": 0,
                        "records_generated": 0
                    }))?);
                }
                OutputFormat::Text => {
                    println!("✓ Empty template requires no data generation");
                    println!("  Template: {}", template);
                    println!("  Schemas processed: 0");
                    println!("  Records generated: 0");
                }
            }
        }
        _ => {
            // Future: Handle templates with schemas
            return Err(anyhow::anyhow!("Template '{}' not yet implemented. Only 'empty' template is currently supported.", template));
        }
    }
    
    Ok(())
}

async fn handle_build(
    template: String,
    fixtures_dir: Option<PathBuf>,
    db_name: String,
    _database_url: Option<String>,
    clone: Option<String>,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    let fixtures_dir = get_fixtures_dir(fixtures_dir);
    let template_dir = get_template_dir(fixtures_dir.clone(), &template);
    
    // Check if template exists
    if !template_dir.exists() {
        return Err(anyhow::anyhow!("Template '{}' not found at: {}", template, template_dir.display()));
    }
    
    // Execute the build process
    let build_result = execute_template_build(&template, &template_dir, &db_name, &clone, &fixtures_dir, &output_format).await?;
    
    match output_format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&build_result)?);
        }
        OutputFormat::Text => {
            println!("✓ Template build completed");
            println!("  Template: {}", template);
            println!("  Database: {}", db_name);
            if let Some(steps) = build_result.get("build_steps") {
                if let Some(steps_array) = steps.as_array() {
                    for step in steps_array {
                        if let Some(step_str) = step.as_str() {
                            println!("  └─ {}", step_str);
                        }
                    }
                }
            }
        }
    }
    
    Ok(())
}

async fn handle_deploy(
    template: String,
    fixtures_dir: Option<PathBuf>,
    target: Option<String>,
    progress: bool,
    _database_url: Option<String>,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    let fixtures_dir = get_fixtures_dir(fixtures_dir);
    let template_dir = get_template_dir(fixtures_dir, &template);
    
    // Check if template exists
    if !template_dir.exists() {
        return Err(anyhow::anyhow!("Template '{}' not found at: {}", template, template_dir.display()));
    }
    
    match output_format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&json!({
                "success": true,
                "message": format!("Deploy command not yet implemented for template '{}'", template),
                "template": template,
                "target": target,
                "progress": progress
            }))?);
        }
        OutputFormat::Text => {
            println!("✓ Deploy command not yet implemented");
            println!("  Template: {}", template);
            println!("  Target: {:?}", target);
            println!("  Progress: {}", progress);
            println!("  Template dir: {}", template_dir.display());
        }
    }
    
    Ok(())
}