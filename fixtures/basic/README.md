# Basic Fixtures Template

This template creates a tenant database by cloning the system template and adding demo schemas for basic application functionality.

## Template Hierarchy

```
system (base template)
└── basic (clones system + demo schemas)
```

## Contents

### SQL Initialization
- *None* - Clones from system template

### Schemas (2 files)
- `account.json` - User account management
- `project.json` - Project management with account relationships

### Data (0 files)  
- No sample data - schemas create empty tables

## Core Tables Inherited + Added

**Inherited from system template:**
- **`schemas`** - Schema registry
- **`columns`** - Column metadata  
- **`users`** - User accounts
- **`pings`** - Health check table

**Added by this template:**
- **`accounts`** - Account management (from account.json)
- **`projects`** - Project management (from project.json)

## Usage

```bash
# Build basic template (clones system + processes schemas)
monk fixture build basic

# This clones "template_system" and adds account + project tables
```

## Database Output

- **Database Name**: `template_basic` (or custom via --db-name)
- **Clone Source**: `template_system`
- **Purpose**: Demo application with accounts and projects
- **Use Case**: Starting point for applications with user/project management