use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub environment: Environment,
    pub filter: FilterConfig,
    pub database: DatabaseConfig,
    pub api: ApiConfig,
    pub security: SecurityConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Environment {
    Development,
    Staging,
    Production,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterConfig {
    pub allow_raw_sql: bool,
    pub max_limit: Option<i32>,
    pub max_nested_depth: u32,
    pub enable_query_cache: bool,
    pub debug_logging: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub max_connections: u32,
    pub connection_timeout: u64,
    pub enable_query_logging: bool,
    pub enable_slow_query_warning: bool,
    pub slow_query_threshold_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    pub enable_rate_limiting: bool,
    pub rate_limit_requests: u32,
    pub rate_limit_window_secs: u64,
    pub enable_request_logging: bool,
    pub enable_response_compression: bool,
    pub max_request_size_bytes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub enable_cors: bool,
    pub cors_origins: Vec<String>,
    pub require_https: bool,
    pub enable_audit_logging: bool,
    pub jwt_expiry_hours: u64,
}

impl AppConfig {
    pub fn from_env() -> Self {
        let environment = match env::var("APP_ENV").as_deref() {
            Ok("production") | Ok("prod") => Environment::Production,
            Ok("staging") | Ok("stage") => Environment::Staging,
            _ => Environment::Development,
        };

        // Set defaults based on environment, then override with specific env vars
        match environment {
            Environment::Production => Self::production(),
            Environment::Staging => Self::staging(),
            Environment::Development => Self::development(),
        }
        .with_env_overrides()
    }

    fn with_env_overrides(mut self) -> Self {
        // Filter overrides
        if let Ok(v) = env::var("FILTER_ALLOW_RAW_SQL") {
            self.filter.allow_raw_sql = v.parse().unwrap_or(self.filter.allow_raw_sql);
        }
        if let Ok(v) = env::var("FILTER_MAX_LIMIT") {
            self.filter.max_limit = v.parse().ok();
        }
        if let Ok(v) = env::var("FILTER_MAX_NESTED_DEPTH") {
            self.filter.max_nested_depth = v.parse().unwrap_or(self.filter.max_nested_depth);
        }
        if let Ok(v) = env::var("FILTER_ENABLE_QUERY_CACHE") {
            self.filter.enable_query_cache = v.parse().unwrap_or(self.filter.enable_query_cache);
        }
        if let Ok(v) = env::var("FILTER_DEBUG_LOGGING") {
            self.filter.debug_logging = v.parse().unwrap_or(self.filter.debug_logging);
        }

        // Database overrides
        if let Ok(v) = env::var("DATABASE_MAX_CONNECTIONS") {
            self.database.max_connections = v.parse().unwrap_or(self.database.max_connections);
        }
        if let Ok(v) = env::var("DATABASE_CONNECTION_TIMEOUT") {
            self.database.connection_timeout = v.parse().unwrap_or(self.database.connection_timeout);
        }
        if let Ok(v) = env::var("DATABASE_ENABLE_QUERY_LOGGING") {
            self.database.enable_query_logging = v.parse().unwrap_or(self.database.enable_query_logging);
        }
        if let Ok(v) = env::var("DATABASE_ENABLE_SLOW_QUERY_WARNING") {
            self.database.enable_slow_query_warning = v.parse().unwrap_or(self.database.enable_slow_query_warning);
        }
        if let Ok(v) = env::var("DATABASE_SLOW_QUERY_THRESHOLD_MS") {
            self.database.slow_query_threshold_ms = v.parse().unwrap_or(self.database.slow_query_threshold_ms);
        }

        // API overrides
        if let Ok(v) = env::var("API_ENABLE_RATE_LIMITING") {
            self.api.enable_rate_limiting = v.parse().unwrap_or(self.api.enable_rate_limiting);
        }
        if let Ok(v) = env::var("API_RATE_LIMIT_REQUESTS") {
            self.api.rate_limit_requests = v.parse().unwrap_or(self.api.rate_limit_requests);
        }
        if let Ok(v) = env::var("API_RATE_LIMIT_WINDOW_SECS") {
            self.api.rate_limit_window_secs = v.parse().unwrap_or(self.api.rate_limit_window_secs);
        }
        if let Ok(v) = env::var("API_ENABLE_REQUEST_LOGGING") {
            self.api.enable_request_logging = v.parse().unwrap_or(self.api.enable_request_logging);
        }
        if let Ok(v) = env::var("API_ENABLE_RESPONSE_COMPRESSION") {
            self.api.enable_response_compression = v.parse().unwrap_or(self.api.enable_response_compression);
        }
        if let Ok(v) = env::var("API_MAX_REQUEST_SIZE_BYTES") {
            self.api.max_request_size_bytes = v.parse().unwrap_or(self.api.max_request_size_bytes);
        }

        // Security overrides
        if let Ok(v) = env::var("SECURITY_ENABLE_CORS") {
            self.security.enable_cors = v.parse().unwrap_or(self.security.enable_cors);
        }
        if let Ok(v) = env::var("SECURITY_CORS_ORIGINS") {
            self.security.cors_origins = v.split(',').map(|s| s.trim().to_string()).collect();
        }
        if let Ok(v) = env::var("SECURITY_REQUIRE_HTTPS") {
            self.security.require_https = v.parse().unwrap_or(self.security.require_https);
        }
        if let Ok(v) = env::var("SECURITY_ENABLE_AUDIT_LOGGING") {
            self.security.enable_audit_logging = v.parse().unwrap_or(self.security.enable_audit_logging);
        }
        if let Ok(v) = env::var("SECURITY_JWT_EXPIRY_HOURS") {
            self.security.jwt_expiry_hours = v.parse().unwrap_or(self.security.jwt_expiry_hours);
        }

        self
    }

    fn development() -> Self {
        Self {
            environment: Environment::Development,
            filter: FilterConfig {
                allow_raw_sql: true,
                max_limit: Some(1000),
                max_nested_depth: 10,
                enable_query_cache: false,
                debug_logging: true,
            },
            database: DatabaseConfig {
                max_connections: 10,
                connection_timeout: 30,
                enable_query_logging: true,
                enable_slow_query_warning: true,
                slow_query_threshold_ms: 100,
            },
            api: ApiConfig {
                enable_rate_limiting: false,
                rate_limit_requests: 1000,
                rate_limit_window_secs: 60,
                enable_request_logging: true,
                enable_response_compression: false,
                max_request_size_bytes: 10 * 1024 * 1024, // 10MB
            },
            security: SecurityConfig {
                enable_cors: true,
                cors_origins: vec!["http://localhost:3000".to_string(), "http://localhost:5173".to_string()],
                require_https: false,
                enable_audit_logging: false,
                jwt_expiry_hours: 24 * 7, // 1 week
            },
        }
    }

    fn staging() -> Self {
        Self {
            environment: Environment::Staging,
            filter: FilterConfig {
                allow_raw_sql: false,
                max_limit: Some(500),
                max_nested_depth: 5,
                enable_query_cache: true,
                debug_logging: false,
            },
            database: DatabaseConfig {
                max_connections: 20,
                connection_timeout: 10,
                enable_query_logging: true,
                enable_slow_query_warning: true,
                slow_query_threshold_ms: 500,
            },
            api: ApiConfig {
                enable_rate_limiting: true,
                rate_limit_requests: 100,
                rate_limit_window_secs: 60,
                enable_request_logging: true,
                enable_response_compression: true,
                max_request_size_bytes: 5 * 1024 * 1024, // 5MB
            },
            security: SecurityConfig {
                enable_cors: true,
                cors_origins: vec!["https://staging.example.com".to_string()],
                require_https: true,
                enable_audit_logging: true,
                jwt_expiry_hours: 24,
            },
        }
    }

    fn production() -> Self {
        Self {
            environment: Environment::Production,
            filter: FilterConfig {
                allow_raw_sql: false,
                max_limit: Some(100),
                max_nested_depth: 3,
                enable_query_cache: true,
                debug_logging: false,
            },
            database: DatabaseConfig {
                max_connections: 50,
                connection_timeout: 5,
                enable_query_logging: false,
                enable_slow_query_warning: true,
                slow_query_threshold_ms: 1000,
            },
            api: ApiConfig {
                enable_rate_limiting: true,
                rate_limit_requests: 60,
                rate_limit_window_secs: 60,
                enable_request_logging: false,
                enable_response_compression: true,
                max_request_size_bytes: 2 * 1024 * 1024, // 2MB
            },
            security: SecurityConfig {
                enable_cors: true,
                cors_origins: vec!["https://app.example.com".to_string()],
                require_https: true,
                enable_audit_logging: true,
                jwt_expiry_hours: 4,
            },
        }
    }
}

// Global singleton config - initialized once at startup
pub static CONFIG: Lazy<AppConfig> = Lazy::new(AppConfig::from_env);

// Convenience function for accessing config
pub fn config() -> &'static AppConfig {
    &CONFIG
}

// Helper macros for common checks
#[macro_export]
macro_rules! is_development {
    () => {
        matches!($crate::config::CONFIG.environment, $crate::config::Environment::Development)
    };
}

#[macro_export]
macro_rules! is_production {
    () => {
        matches!($crate::config::CONFIG.environment, $crate::config::Environment::Production)
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_development_config() {
        let config = AppConfig::development();
        assert!(config.filter.allow_raw_sql);
        assert_eq!(config.filter.max_limit, Some(1000));
        assert!(!config.api.enable_rate_limiting);
    }

    #[test]
    fn test_default_production_config() {
        let config = AppConfig::production();
        assert!(!config.filter.allow_raw_sql);
        assert_eq!(config.filter.max_limit, Some(100));
        assert!(config.api.enable_rate_limiting);
    }
}