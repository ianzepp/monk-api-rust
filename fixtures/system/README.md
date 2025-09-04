# System Fixtures Template

This is the **base template** that creates the core system tables required by all Monk tenants. All other templates clone from this foundation.

## Purpose

This template provides the essential infrastructure tables that every tenant database requires. Other templates (empty, basic, etc.) clone this template and add their own schemas and data on top.

## Contents

### SQL Initialization
- `init-tenant.sql` - Creates core system tables

### Schemas (0 files)
- No custom schemas - only system infrastructure

### Data (0 files)  
- No sample data - only essential system records

## Core Tables Created

When this template is built, it creates the essential infrastructure tables:

- **`schemas`** - Schema registry (contains self-references for system tables)
- **`columns`** - Column metadata (empty initially)  
- **`users`** - User accounts (empty initially, populated by API)
- **`pings`** - Health check table (empty initially)

## Usage

```bash
# Build system template (foundation for all other templates)
monk fixture build system

# This creates a database named "template_system" that other templates can clone
```

## Template Hierarchy

```
system (base template)
├── empty (system + no additions)
├── basic (system + demo schemas + small data)
└── basic_large (system + demo schemas + large data)
```

## Database Output

- **Database Name**: `template_system`
- **Purpose**: Foundation template for cloning
- **Use Case**: Base for all tenant database creation