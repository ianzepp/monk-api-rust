# Tenant Provisioning and Templates

This document specifies the template-driven tenant provisioning workflow and naming conventions used by monk-api-rust. It formalizes how template databases are created and how new tenant databases are cloned from those templates.

## Overview

- Templates are real PostgreSQL databases pre-populated with core system tables and baseline data (fixtures).
- New tenants are created by cloning a selected template database on the same PostgreSQL host.
- Each tenant receives its own isolated database named `tenant_<hash>`, where `<hash>` is a stable, fixed-length hex string derived from the tenant name.
- The system database ("main") tracks templates and tenants in its `monk_main.tenants` table.

## Naming Conventions

- System database: `monk_main`
  - Future work: allow override via environment (e.g., `MONK_SYSTEM_DB_NAME`).
- Template databases: `monk_template_<template_key>`
  - Examples: `monk_template_basic`, `monk_template_crm`, `monk_template_demo`.
- Tenant databases: `tenant_<hash>`
  - `<hash>`: 16 hexadecimal characters derived deterministically from the requested tenant name (e.g., SHA-based or similar). Example: `tenant_007314608dd04169`.

## Template Build Process

Template creation is performed once per template key.

1) Create the template database
   - Create a fresh PostgreSQL database for the template key: `monk_template_<template_key>`.

2) Install core system objects
   - Create system tables present in every tenant DB:
     - `users` (authentication/authorization)
     - `schemas` (dynamic schema registry)
     - `columns` (dynamic column metadata)
   - Apply any required core indexes and constraints.

3) Load baseline schemas and data (fixtures)
   - Import baseline JSON Schemas (e.g., `accounts`, `contacts`, etc.).
   - Generate and migrate dynamic tables as needed.
   - Load seed records for demos or default use.

4) Finalize template
   - Ensure the template is consistent and passes health checks.
   - Mark template ready for cloning (operational convention; no special DB flag required in PostgreSQL).

Notes:
- For PostgreSQL `CREATE DATABASE ... TEMPLATE ...` cloning, the template must not have any active sessions when cloning.
- Consider operational conventions to avoid opening long-lived connections to template databases.

## Tenant Provisioning Flow

Given a requested tenant name and chosen template key:

1) Resolve template database name
   - `template_db = "monk_template_<template_key>"`

2) Compute tenant database name
   - `hash = stable_hex_16(tenant_name)`
   - `tenant_db = format!("tenant_{}", hash)`
   - The hash must be deterministic and stable. A 16-hex prefix of a cryptographic hash (e.g., SHA family) is sufficient.

3) Clone tenant database from template (same PostgreSQL host)

```sql
-- Ensure no active connections to the template database first
CREATE DATABASE "tenant_007314608dd04169" TEMPLATE "monk_template_basic";
```

4) Post-clone initialization (optional)
   - Run tenant-specific migrations if required (e.g., versioned migrations since the template was created).
   - Create tenant-specific roles or extensions if applicable.

5) Register tenant in system database

```sql
INSERT INTO monk_main.tenants (
  name, database, host, is_active, tenant_type, access_read, access_edit, access_full, access_deny
) VALUES (
  $1,     -- tenant name requested (e.g., 'acme_corp')
  $2,     -- tenant database name (e.g., 'tenant_007314608dd04169')
  'localhost',
  true,
  'normal',
  '{}', '{}', '{}', '{}'
);
```

6) Return provisioning response
   - Include the `tenant_db` and any other metadata required by the caller.

## Deprovisioning (High-Level)

- Disable tenant (`is_active = false`).
- Optionally archive/export data.
- Drop tenant database (irreversible): `DROP DATABASE "tenant_<hash>";`
- Remove registry entry from `monk_main.tenants` or mark as deleted.

## Operational Considerations

- Template availability
  - Cloning requires the template DB to be free of active sessions. Avoid background tasks or users connected to the template DBs.
- Same-host assumption (current)
  - `CREATE DATABASE ... TEMPLATE ...` works only when source and destination reside on the same PostgreSQL instance.
  - Future work: cross-host cloning (pg_dump/restore or logical replication) when templates and tenants are distributed.
- Isolation guarantees
  - Per-tenant database isolation eliminates cross-tenant queries and minimizes blast radius.
- Performance
  - Database-level cloning is very fast for templates with modest size. For very large templates, consider streaming restore options.

## Hashing Strategy

- Use a deterministic hash of the requested tenant name to produce a collision-resistant 16-hex suffix.
- Keep the chosen algorithm internal to the service to allow future changes without breaking external contracts.
- Examples (illustrative only): SHA-1/256 truncated to 16 hex, BLAKE3 truncated to 16 hex.
- Ensure database name validation accepts only `[A-Za-z0-9_]+`.

## Future Enhancements

- Configurable system DB name via environment (e.g., `MONK_SYSTEM_DB_NAME`).
- Template registry in `monk_main` (track template versions, health, and build provenance).
- Cross-host provisioning support (dump/restore or replication-based cloning).
- Post-provision observers (audit, notification, welcome data, etc.).
- Automated cleanup of stale templates or retired versions.

