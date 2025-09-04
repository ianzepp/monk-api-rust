-- Advanced template init.sql
-- This runs AFTER cloning from system template

-- Add some additional system configuration
INSERT INTO "schemas" (name, table_name, status, definition, field_count, json_checksum)
VALUES (
    'settings',
    'settings',
    'system',
    '{
        "type": "object",
        "title": "Settings",
        "description": "Application configuration settings",
        "properties": {
            "key": {
                "type": "string",
                "minLength": 1,
                "maxLength": 100,
                "description": "Setting key"
            },
            "value": {
                "type": "string",
                "description": "Setting value"
            }
        },
        "required": ["key", "value"],
        "additionalProperties": false
    }',
    '2',
    null
);

-- Create the actual settings table
CREATE TABLE "settings" (
    "id" uuid PRIMARY KEY DEFAULT gen_random_uuid() NOT NULL,
    "access_read" uuid[] DEFAULT '{}'::uuid[],
    "access_edit" uuid[] DEFAULT '{}'::uuid[],
    "access_full" uuid[] DEFAULT '{}'::uuid[],
    "access_deny" uuid[] DEFAULT '{}'::uuid[],
    "created_at" timestamp DEFAULT now() NOT NULL,
    "updated_at" timestamp DEFAULT now() NOT NULL,
    "trashed_at" timestamp,
    "deleted_at" timestamp,
    "key" text NOT NULL,
    "value" text NOT NULL,
    CONSTRAINT "settings_key_unique" UNIQUE("key")
);