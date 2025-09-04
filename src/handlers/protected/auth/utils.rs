use serde::{Deserialize, Serialize};

/// Authenticated user information extracted from JWT token
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticatedUser {
    pub id: String,
    pub username: String,
    pub email: Option<String>,
    pub tenant: String,
    pub database: String,
    pub role: String,
    pub permissions: Vec<String>,
    pub session_type: SessionType,
    pub token_expires: chrono::DateTime<chrono::Utc>,
}

/// Session type for tracking privilege levels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SessionType {
    /// Standard user session
    Standard,
    /// Elevated admin session with sudo privileges
    Elevated,
    /// System/service account session
    System,
}

/// Permission levels for authorization checks
#[derive(Debug, Clone, PartialEq)]
pub enum Permission {
    Read,
    Write,
    Admin,
    Sudo,
    Root,
    Custom(String),
}

impl Permission {
    /// Convert permission to string representation
    pub fn as_str(&self) -> &str {
        match self {
            Permission::Read => "read",
            Permission::Write => "write", 
            Permission::Admin => "admin",
            Permission::Sudo => "sudo",
            Permission::Root => "root",
            Permission::Custom(s) => s,
        }
    }

    /// Parse permission from string
    pub fn from_str(s: &str) -> Self {
        match s {
            "read" => Permission::Read,
            "write" => Permission::Write,
            "admin" => Permission::Admin,
            "sudo" => Permission::Sudo,
            "root" => Permission::Root,
            other => Permission::Custom(other.to_string()),
        }
    }
}

impl AuthenticatedUser {
    /// Check if user has a specific permission
    pub fn has_permission(&self, permission: &Permission) -> bool {
        self.permissions.contains(&permission.as_str().to_string())
    }

    /// Check if user has any of the specified permissions
    pub fn has_any_permission(&self, permissions: &[Permission]) -> bool {
        permissions.iter().any(|p| self.has_permission(p))
    }

    /// Check if user has all of the specified permissions
    pub fn has_all_permissions(&self, permissions: &[Permission]) -> bool {
        permissions.iter().all(|p| self.has_permission(p))
    }

    /// Check if user can elevate to sudo
    pub fn can_sudo(&self) -> bool {
        self.has_permission(&Permission::Admin) || self.has_permission(&Permission::Sudo)
    }

    /// Check if session is elevated
    pub fn is_elevated(&self) -> bool {
        self.session_type == SessionType::Elevated
    }

    /// Get token expiration time
    pub fn token_expires_in(&self) -> chrono::Duration {
        self.token_expires - chrono::Utc::now()
    }

    /// Check if token is expired or expiring soon
    pub fn is_token_expired(&self, buffer_minutes: i64) -> bool {
        let buffer = chrono::Duration::minutes(buffer_minutes);
        chrono::Utc::now() + buffer >= self.token_expires
    }
}

/// Extract Bearer token from Authorization header
/// 
/// Parses "Bearer <token>" format and returns the token portion.
/// 
/// # Arguments
/// * `auth_header` - Authorization header value
/// 
/// # Returns
/// * `Result<String, String>` - Extracted token or error message
pub fn extract_bearer_token(auth_header: &str) -> Result<String, String> {
    const BEARER_PREFIX: &str = "Bearer ";
    
    if !auth_header.starts_with(BEARER_PREFIX) {
        return Err("Authorization header must start with 'Bearer '".to_string());
    }
    
    let token = auth_header[BEARER_PREFIX.len()..].trim();
    
    if token.is_empty() {
        return Err("Bearer token is empty".to_string());
    }
    
    Ok(token.to_string())
}

/// Validate that user has required permissions for an operation
/// 
/// Helper function for authorization checks in protected endpoints.
/// 
/// # Arguments
/// * `user` - Authenticated user to check
/// * `required_permissions` - Permissions required for the operation
/// * `require_elevated` - Whether elevated session is required
/// 
/// # Returns
/// * `Result<(), String>` - Success or authorization error message
pub fn authorize_user(
    user: &AuthenticatedUser,
    required_permissions: &[Permission],
    require_elevated: bool,
) -> Result<(), String> {
    // Check if elevated session is required
    if require_elevated && !user.is_elevated() {
        return Err("Elevated session required. Please use sudo to elevate privileges.".to_string());
    }
    
    // Check if user has any of the required permissions
    if !required_permissions.is_empty() && !user.has_any_permission(required_permissions) {
        let perm_names: Vec<&str> = required_permissions.iter().map(|p| p.as_str()).collect();
        return Err(format!("Insufficient permissions. Required: {}", perm_names.join(", ")));
    }
    
    // Check if token is expired or expiring soon (5 minute buffer)
    if user.is_token_expired(5) {
        return Err("Token expired or expiring soon. Please refresh your session.".to_string());
    }
    
    Ok(())
}

/// Generate elevated permissions list for sudo operations
/// 
/// Creates an appropriate permissions list for elevated sessions
/// based on the user's current role and requested permissions.
/// 
/// # Arguments
/// * `user` - Current authenticated user
/// * `requested_permissions` - Specific permissions being requested
/// 
/// # Returns
/// * `Vec<String>` - List of permissions for elevated session
pub fn generate_elevated_permissions(
    user: &AuthenticatedUser,
    requested_permissions: Option<&[String]>,
) -> Vec<String> {
    let mut permissions = user.permissions.clone();
    
    // Add standard elevated permissions
    let elevated_perms = ["sudo", "admin"];
    for perm in &elevated_perms {
        if !permissions.contains(&perm.to_string()) {
            permissions.push(perm.to_string());
        }
    }
    
    // Add specifically requested permissions if user is allowed
    if let Some(requested) = requested_permissions {
        for perm in requested {
            // Only add if user already has admin/sudo rights
            if user.has_permission(&Permission::Admin) && !permissions.contains(perm) {
                permissions.push(perm.clone());
            }
        }
    }
    
    permissions
}

/// Session duration constants for different session types
pub mod session_duration {
    use chrono::Duration;
    
    /// Standard session duration (24 hours)
    pub const STANDARD: Duration = Duration::hours(24);
    
    /// Elevated session duration (30 minutes for security)
    pub const ELEVATED: Duration = Duration::minutes(30);
    
    /// System session duration (1 hour)
    pub const SYSTEM: Duration = Duration::hours(1);
    
    /// Refresh token buffer (5 minutes)
    pub const REFRESH_BUFFER: Duration = Duration::minutes(5);
}

/// Authorization helper macros for common permission checks
#[macro_export]
macro_rules! require_permission {
    ($user:expr, $perm:expr) => {
        if !$user.has_permission(&$perm) {
            return Err(AppError::Forbidden(format!(
                "Permission '{}' required", 
                $perm.as_str()
            )));
        }
    };
}

#[macro_export]
macro_rules! require_elevated {
    ($user:expr) => {
        if !$user.is_elevated() {
            return Err(AppError::Forbidden(
                "Elevated session required. Use sudo to elevate privileges.".to_string()
            ));
        }
    };
}

#[macro_export]
macro_rules! require_admin {
    ($user:expr) => {
        require_permission!($user, crate::handlers::protected::auth::utils::Permission::Admin);
    };
}

/*
PROTECTED AUTH UTILITIES OVERVIEW:

This module provides utilities specifically for protected authentication
endpoints and middleware. It complements the public auth utilities but
focuses on authenticated user sessions and authorization.

KEY FEATURES:

1. **User Context Management**:
   - AuthenticatedUser struct with comprehensive user info
   - Permission checking methods
   - Session type tracking
   - Token expiration management

2. **Authorization Helpers**:
   - Permission validation functions
   - Elevation requirement checking
   - Token freshness validation
   - Macro shortcuts for common checks

3. **Token Management**:
   - Bearer token extraction from headers
   - Elevated permission generation
   - Session duration constants
   - Expiration checking with buffers

4. **Security Features**:
   - Session type enforcement (standard vs elevated)
   - Permission hierarchy validation
   - Token expiration monitoring
   - Authorization logging support

USAGE PATTERNS:

1. **In Middleware**:
   ```rust
   let token = extract_bearer_token(&auth_header)?;
   let user = validate_and_extract_user(&token)?;
   request.extensions_mut().insert(user);
   ```

2. **In Protected Endpoints**:
   ```rust
   pub async fn admin_operation(
       Extension(user): Extension<AuthenticatedUser>
   ) -> Result<Json<Value>, AppError> {
       authorize_user(&user, &[Permission::Admin], false)?;
       // ... operation logic
   }
   ```

3. **For Sudo Operations**:
   ```rust
   require_elevated!(user);
   require_admin!(user);
   ```

INTEGRATION:

- Works with JWT middleware for token validation
- Provides user context for all protected endpoints
- Supports audit logging through user tracking
- Enables fine-grained permission control

This utilities module makes authorization checks clean, consistent,
and secure across all protected API endpoints.
*/