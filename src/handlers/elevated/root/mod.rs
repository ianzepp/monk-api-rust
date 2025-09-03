// handlers/elevated/root/mod.rs - Root administrative handlers
//
// Root-level administrative operations that require elevated JWT tokens.
// These handlers provide system-wide management capabilities.

// Root operation modules
pub mod tenant;  // Multi-tenant management operations

// Re-export tenant management handlers
pub use tenant::*;

/*
ROOT HANDLER ORGANIZATION:

This module contains handlers for /api/root/ endpoints that require
elevated privileges obtained through the sudo elevation process.

Current Modules:

1. **Tenant Management** (/api/root/tenant/):
   - Complete tenant lifecycle management
   - Database provisioning and health monitoring  
   - Cross-tenant administrative operations

Future Modules:
- System configuration management
- Platform-wide analytics and reporting
- User management across tenants
- Backup and disaster recovery operations

All root operations are subject to:
- Comprehensive audit logging
- IP-based access restrictions (future)
- Multi-factor authentication requirements (future)
- Approval workflows for destructive operations (future)
*/