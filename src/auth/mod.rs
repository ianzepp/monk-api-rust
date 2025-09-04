use chrono::{Duration, Utc};
use jsonwebtoken::{encode, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::config;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub tenant: String,
    pub user: String,
    pub database: String,
    pub access: String,
    pub user_id: Uuid,
    pub exp: i64,
    pub iat: i64,
}

impl Claims {
    pub fn new(tenant: String, user: String, database: String, access: String, user_id: Uuid) -> Self {
        let now = Utc::now();
        let expiry_hours = config::config().security.jwt_expiry_hours;
        let exp = (now + Duration::hours(expiry_hours as i64)).timestamp();
        
        Self {
            tenant,
            user,
            database,
            access,
            user_id,
            exp,
            iat: now.timestamp(),
        }
    }
}

#[derive(Debug)]
pub enum JwtError {
    TokenGeneration(String),
    InvalidSecret,
}

impl std::fmt::Display for JwtError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JwtError::TokenGeneration(msg) => write!(f, "JWT generation error: {}", msg),
            JwtError::InvalidSecret => write!(f, "Invalid JWT secret"),
        }
    }
}

impl std::error::Error for JwtError {}

pub fn generate_jwt(claims: Claims) -> Result<String, JwtError> {
    let secret = &config::config().security.jwt_secret;
    
    if secret.is_empty() {
        return Err(JwtError::InvalidSecret);
    }
    
    let encoding_key = EncodingKey::from_secret(secret.as_bytes());
    let header = Header::default();
    
    encode(&header, &claims, &encoding_key)
        .map_err(|e| JwtError::TokenGeneration(e.to_string()))
}
