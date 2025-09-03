# Monk API Rust

## Executive Summary

**🚨 DEVELOPMENT STATUS: NON-FUNCTIONAL REWRITE IN PROGRESS 🚨**

**PaaS Management API** - Rust rewrite of the Monk API backend, providing comprehensive REST API services for multi-tenant PaaS platform management with dynamic schema support and JWT authentication.

### Project Status
This is a **brand new rewrite** of the existing [monk-cli](https://github.com/ianzepp/monk-cli) TypeScript API backend. The project is currently **non-functional** and under active development.

**Current Implementation Status:**
- ❌ **Authentication**: JWT authentication system (not implemented)
- ❌ **Data Operations**: Dynamic CRUD operations on tenant schemas (not implemented) 
- ❌ **Schema Management**: JSON Schema to PostgreSQL DDL generation (not implemented)
- ❌ **Multi-tenant Support**: Tenant isolation and management (not implemented)
- ✅ **Basic Server**: Axum web server with placeholder routes (functional)

### Project Overview
- **Language**: Rust with Axum web framework
- **Purpose**: 100% API-compatible rewrite of Monk API PaaS platform backend
- **Architecture**: Async REST API with PostgreSQL backend and multi-tenant support
- **Database Compatibility**: Uses identical PostgreSQL schema as original monk-api
- **CLI Compatibility**: Designed to work seamlessly with existing [monk-cli](https://github.com/ianzepp/monk-cli) tools

### Target Features (To Be Implemented)
- **Multi-Tenant REST API**: Complete HTTP API matching original monk-api endpoints
- **Dynamic Schema Management**: JSON Schema definitions that auto-generate PostgreSQL tables
- **JWT Authentication**: Secure tenant-scoped authentication with session management
- **CRUD Operations**: Full create, read, update, delete operations on dynamic schemas
- **PostgreSQL Integration**: Direct compatibility with existing monk-api database
- **Cross-Platform Support**: macOS and Linux development environments

### Technical Architecture (Planned)
- **Web Framework**: Axum (async, performant, type-safe)
- **Database**: SQLx with PostgreSQL (compile-time query validation)
- **Authentication**: JWT with RS256/HS256 algorithms
- **Serialization**: Serde with JSON/YAML support
- **Schema Management**: Custom PostgreSQL DDL generator
- **Multi-tenancy**: Database-per-tenant isolation

### API Endpoints (Planned Implementation)

#### **Authentication** (`/api/auth/*`)
- `POST /api/auth/login` - JWT authentication with tenant scope
- `GET /api/auth/status` - Current authentication status and token info
- `GET /api/auth/info` - Detailed JWT token information
- `POST /api/auth/logout` - Clear authentication session

#### **Data Operations** (`/api/data/:schema/*`)
- `GET /api/data/:schema` - List records in schema with query support
- `GET /api/data/:schema/:id` - Get specific record by ID
- `POST /api/data/:schema` - Create new records (single or bulk)
- `PUT /api/data/:schema/:id` - Update record by ID
- `DELETE /api/data/:schema/:id` - Delete record by ID

#### **Schema Management** (`/api/meta/schema/*`)
- `GET /api/meta/schema/:name` - Get JSON Schema definition
- `POST /api/meta/schema` - Create new schema and generate PostgreSQL table
- `PUT /api/meta/schema/:name` - Update schema and alter PostgreSQL table
- `DELETE /api/meta/schema/:name` - Soft delete schema

## Installation & Development

### Prerequisites
- **Rust 1.70+** with cargo
- **PostgreSQL 15+** 
- **SQLx CLI** for database migrations

### Development Setup
```bash
# Clone repository
git clone https://github.com/ianzepp/monk-api-rust
cd monk-api-rust

# Install dependencies
cargo build

# Run development server (placeholder functionality only)
cargo run

# The server will start on http://localhost:3000
```

### Testing Current Implementation
```bash
# Basic server health check
curl http://localhost:3000/health

# API root information
curl http://localhost:3000/

# Test placeholder endpoints (returns 501 Not Implemented)
curl -X POST http://localhost:3000/api/auth/login
curl http://localhost:3000/api/data/users
curl http://localhost:3000/api/meta/schema/users
```

## Original Monk CLI Integration

This API is designed for **100% compatibility** with the existing [monk-cli](https://github.com/ianzepp/monk-cli) command-line interface:

```bash
# Once functional, monk-cli should work identically against this Rust backend
monk server add rust-local localhost:3000 --description "Rust API Backend"
monk server use rust-local
monk tenant add my-app "My Application"
monk tenant use my-app
monk auth login my-app admin
monk data select users
```

### Database Compatibility
The Rust implementation will use **identical PostgreSQL schemas** as the original TypeScript monk-api, ensuring:
- Seamless database migration between implementations
- Shared data access across both API versions  
- No data conversion or migration required
- Drop-in replacement capability

## Architecture Comparison

### Original monk-api (TypeScript)
- **Framework**: Express.js with TypeScript
- **Database**: PostgreSQL with custom ORM
- **Authentication**: JWT with passport.js
- **Status**: Production-ready, feature-complete

### monk-api-rust (This Project)
- **Framework**: Axum with native async Rust
- **Database**: PostgreSQL with SQLx (compile-time verification)
- **Authentication**: JWT with jsonwebtoken crate
- **Status**: Development, non-functional rewrite

## Development Roadmap

### Phase 1: Core Infrastructure ⏳
- [ ] Basic Axum server setup (✅ Complete)
- [ ] PostgreSQL connection and configuration  
- [ ] Environment configuration and error handling
- [ ] Request/response middleware stack

### Phase 2: Authentication System
- [ ] JWT token generation and validation
- [ ] Multi-tenant authentication scopes
- [ ] Session management and persistence
- [ ] Auth middleware and route protection

### Phase 3: Data Operations
- [ ] Dynamic schema registry
- [ ] CRUD operation handlers
- [ ] Query parameter processing
- [ ] Input validation and sanitization

### Phase 4: Schema Management  
- [ ] JSON Schema validation
- [ ] PostgreSQL DDL generation
- [ ] Schema migration system
- [ ] Backwards compatibility handling

### Phase 5: Integration & Testing
- [ ] monk-cli compatibility testing
- [ ] Database migration scripts
- [ ] Performance optimization
- [ ] Production deployment preparation

## Learning Objectives

This rewrite serves as a **Rust learning project** while creating production-quality software:

- **Async Web Development**: Modern async/await patterns with Axum
- **Database Integration**: Type-safe SQL with compile-time verification
- **Authentication Systems**: JWT implementation and security practices
- **API Design**: RESTful API architecture and error handling
- **PostgreSQL**: Advanced database operations and schema management
- **Production Deployment**: Real-world Rust application deployment

## Contributing

This project is primarily a learning exercise, but contributions are welcome once basic functionality is implemented.

## License

MIT License - See [LICENSE](LICENSE) file for details.

---

**⚠️ Important**: This project is under active development and is **not ready for production use**. For a functional Monk API implementation, please use the original [monk-cli](https://github.com/ianzepp/monk-cli) project.