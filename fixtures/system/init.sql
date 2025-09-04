-- Monk API Required Schema Tables
-- These tables are required for the Hono API to function correctly

-- Schema registry table to store JSON Schema definitions
CREATE TABLE "schemas" (
	"id" uuid PRIMARY KEY DEFAULT gen_random_uuid() NOT NULL,
	"access_read" uuid[] DEFAULT '{}'::uuid[],
	"access_edit" uuid[] DEFAULT '{}'::uuid[],
	"access_full" uuid[] DEFAULT '{}'::uuid[],
	"access_deny" uuid[] DEFAULT '{}'::uuid[],
	"created_at" timestamp DEFAULT now() NOT NULL,
	"updated_at" timestamp DEFAULT now() NOT NULL,
	"trashed_at" timestamp,
	"deleted_at" timestamp,
	"name" text NOT NULL,
	"table_name" text NOT NULL,
	"status" text DEFAULT 'pending' NOT NULL,
	"definition" jsonb NOT NULL,
	"field_count" text NOT NULL,
	"json_checksum" text,
	CONSTRAINT "schema_name_unique" UNIQUE("name"),
	CONSTRAINT "schema_table_name_unique" UNIQUE("table_name")
);

-- Column registry table to store individual field metadata
CREATE TABLE "columns" (
	"id" uuid PRIMARY KEY DEFAULT gen_random_uuid() NOT NULL,
	"access_read" uuid[] DEFAULT '{}'::uuid[],
	"access_edit" uuid[] DEFAULT '{}'::uuid[],
	"access_full" uuid[] DEFAULT '{}'::uuid[],
	"access_deny" uuid[] DEFAULT '{}'::uuid[],
	"created_at" timestamp DEFAULT now() NOT NULL,
	"updated_at" timestamp DEFAULT now() NOT NULL,
	"trashed_at" timestamp,
	"deleted_at" timestamp,
	"schema_name" text NOT NULL,
	"column_name" text NOT NULL,
	"pg_type" text NOT NULL,
	"is_required" text DEFAULT 'false' NOT NULL,
	"default_value" text,
	"relationship_type" text,
	"related_schema" text,
	"related_column" text,
	"relationship_name" text,
	"cascade_delete" boolean DEFAULT false,
	"required_relationship" boolean DEFAULT false,
	"minimum" numeric,
	"maximum" numeric,
	"pattern_regex" text,
	"enum_values" text[],
	"is_array" boolean DEFAULT false,
	"description" text
);

-- Add foreign key constraint
ALTER TABLE "columns" ADD CONSTRAINT "columns_schemas_name_schema_name_fk"
    FOREIGN KEY ("schema_name") REFERENCES "public"."schemas"("name")
    ON DELETE no action ON UPDATE no action;

-- Add unique index for schema+column combination
CREATE UNIQUE INDEX "idx_columns_schema_column" 
    ON "columns" ("schema_name", "column_name");

-- Users table to store tenant users and their access levels (1-db-per-tenant)
CREATE TABLE "users" (
	"id" uuid PRIMARY KEY DEFAULT gen_random_uuid() NOT NULL,
	"name" text NOT NULL,
	"auth" text NOT NULL,
	"access" text CHECK ("access" IN ('root', 'full', 'edit', 'read', 'deny')) NOT NULL,
	"access_read" uuid[] DEFAULT '{}'::uuid[],
	"access_edit" uuid[] DEFAULT '{}'::uuid[],
	"access_full" uuid[] DEFAULT '{}'::uuid[],
	"access_deny" uuid[] DEFAULT '{}'::uuid[],
	"created_at" timestamp DEFAULT now() NOT NULL,
	"updated_at" timestamp DEFAULT now() NOT NULL,
	"trashed_at" timestamp,
	"deleted_at" timestamp,
	CONSTRAINT "users_auth_unique" UNIQUE("auth")
);

-- Ping logging table to record all ping requests
CREATE TABLE "pings" (
    "id" uuid PRIMARY KEY DEFAULT gen_random_uuid() NOT NULL,
    "timestamp" timestamp DEFAULT now() NOT NULL,
    "client_ip" inet,
    "user_agent" text,
    "request_id" text,
    "response_time_ms" integer,
    "jwt_tenant" text,
    "jwt_user_id" uuid,
    "jwt_access" text,
    "server_version" text,
    "database_status" text,
    "created_at" timestamp DEFAULT now() NOT NULL
);

-- Insert self-reference row to enable recursive schema discovery via data API
-- This allows GET /api/data/schemas to work by querying the schema table itself
INSERT INTO "schemas" (name, table_name, status, definition, field_count, json_checksum)
VALUES (
    'schemas',
    'schemas',
    'system',
    '{
        "type": "object",
        "title": "Schemas",
        "description": "Schema registry table for meta API schema definitions",
        "properties": {
            "name": {
                "type": "string",
                "minLength": 1,
                "maxLength": 100,
                "description": "Unique schema name",
                "example": "account"
            },
            "table_name": {
                "type": "string",
                "minLength": 1,
                "maxLength": 100,
                "description": "Database table name",
                "example": "accounts"
            },
            "status": {
                "type": "string",
                "enum": ["pending", "active", "disabled", "system"],
                "default": "pending",
                "description": "Schema status"
            },
            "definition": {
                "type": "object",
                "description": "JSON Schema definition object",
                "additionalProperties": true
            },
            "field_count": {
                "type": "string",
                "pattern": "^[0-9]+$",
                "description": "Number of fields in schema",
                "example": "5"
            },
            "json_checksum": {
                "type": "string",
                "pattern": "^[a-f0-9]{64}$",
                "description": "SHA256 checksum of original JSON",
                "example": "a1b2c3d4..."
            }
        },
        "required": ["name", "table_name", "status", "definition", "field_count"],
        "additionalProperties": false
    }',
    '6',
    null
);

-- Insert user schema registration to enable user API access
-- This allows GET /api/data/users and GET /api/meta/users to work
INSERT INTO "schemas" (name, table_name, status, definition, field_count, json_checksum)
VALUES (
    'users',
    'users',
    'system',
    '{
        "type": "object",
        "title": "Users",
        "description": "User management schema for tenant databases",
        "properties": {
            "name": {
                "type": "string",
                "minLength": 2,
                "maxLength": 100,
                "description": "Human-readable display name for the user",
                "example": "Jane Smith"
            },
            "auth": {
                "type": "string",
                "minLength": 2,
                "maxLength": 255,
                "description": "Authentication identifier (username, email, etc.)",
                "example": "jane@company.com"
            },
            "access": {
                "type": "string",
                "enum": ["root", "full", "edit", "read", "deny"],
                "description": "Access level for the user",
                "example": "full"
            }
        },
        "required": ["id", "name", "auth", "access"],
        "additionalProperties": false
    }',
    '3',
    null
);
