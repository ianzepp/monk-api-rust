# Empty Fixtures Template

This template creates a minimal tenant database by **cloning** the system template. It provides the essential system infrastructure without any additional schemas or sample data.

## Purpose

This is the simplest template that creates a tenant database with only the core system tables required by the Monk API. It's perfect for starting with a clean slate.

## Template Hierarchy

```
system (base template)
└── empty (clones system + no additions)
```

## Contents

### SQL Initialization
- *None* - Clones from system template

### Schemas (0 files)
- No custom schemas - inherits system infrastructure only

### Data (0 files)  
- No sample data - inherits essential system records only

## Core Tables Inherited

When this template is built, it clones the system template which includes:

- **`schemas`** - Schema registry (contains self-references for system tables)
- **`columns`** - Column metadata (empty initially)  
- **`users`** - User accounts (empty initially, populated by API)
- **`pings`** - Health check table (empty initially)

## Usage

```bash
# Build empty template (clones from system template)
monk fixture build empty --clone system

# This creates a database that clones "template_system" 
```

## Database Output

- **Database Name**: `template_empty` (or custom via --db-name)
- **Clone Source**: `template_system`
- **Purpose**: Minimal tenant database
- **Use Case**: Clean starting point for new tenants