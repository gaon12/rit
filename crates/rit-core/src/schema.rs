/// Stable JSON schema version for rit machine-readable command output.
pub const RIT_SCHEMA_VERSION: u32 = 1;

/// One named JSON schema exposed by rit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JsonSchemaDocument {
    /// CLI/API schema name.
    pub name: &'static str,
    /// Stable schema version.
    pub version: u32,
    /// JSON Schema document.
    pub json: &'static str,
}

const SCHEMAS: &[JsonSchemaDocument] = &[
    JsonSchemaDocument {
        name: "status",
        version: RIT_SCHEMA_VERSION,
        json: STATUS_SCHEMA,
    },
    JsonSchemaDocument {
        name: "diff",
        version: RIT_SCHEMA_VERSION,
        json: DIFF_SCHEMA,
    },
    JsonSchemaDocument {
        name: "doctor",
        version: RIT_SCHEMA_VERSION,
        json: DOCTOR_SCHEMA,
    },
    JsonSchemaDocument {
        name: "operations",
        version: RIT_SCHEMA_VERSION,
        json: OPERATIONS_SCHEMA,
    },
    JsonSchemaDocument {
        name: "impact",
        version: RIT_SCHEMA_VERSION,
        json: IMPACT_SCHEMA,
    },
    JsonSchemaDocument {
        name: "indexdb",
        version: RIT_SCHEMA_VERSION,
        json: INDEXDB_SCHEMA,
    },
];

/// Returns every stable JSON schema known to this build.
pub fn json_schemas() -> &'static [JsonSchemaDocument] {
    SCHEMAS
}

/// Looks up one stable JSON schema by name.
pub fn json_schema(name: &str) -> Option<JsonSchemaDocument> {
    SCHEMAS.iter().copied().find(|schema| schema.name == name)
}

const STATUS_SCHEMA: &str = r##"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://rit.dev/schemas/v1/status.json",
  "title": "rit status",
  "type": "object",
  "additionalProperties": false,
  "required": ["branch", "entries"],
  "properties": {
    "branch": {
      "type": ["object", "null"],
      "additionalProperties": false,
      "required": ["kind", "name"],
      "properties": {
        "kind": { "enum": ["branch", "initial_branch", "detached"] },
        "name": { "type": ["string", "null"] }
      }
    },
    "entries": {
      "type": "array",
      "items": {
        "type": "object",
        "additionalProperties": false,
        "required": ["index_status", "worktree_status", "path"],
        "properties": {
          "index_status": { "type": "string", "minLength": 1, "maxLength": 1 },
          "worktree_status": { "type": "string", "minLength": 1, "maxLength": 1 },
          "path": { "type": "string" }
        }
      }
    }
  }
}"##;

const DIFF_SCHEMA: &str = r##"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://rit.dev/schemas/v1/diff.json",
  "title": "rit diff",
  "type": "object",
  "additionalProperties": false,
  "required": ["files", "warnings"],
  "properties": {
    "files": {
      "type": "array",
      "items": {
        "type": "object",
        "additionalProperties": false,
        "required": ["status", "old_path", "path", "similarity_score", "insertions", "deletions", "binary", "old_size", "new_size"],
        "properties": {
          "status": { "type": "string", "minLength": 1, "maxLength": 1 },
          "old_path": { "type": ["string", "null"] },
          "path": { "type": "string" },
          "similarity_score": { "type": ["integer", "null"], "minimum": 0, "maximum": 100 },
          "insertions": { "type": "integer", "minimum": 0 },
          "deletions": { "type": "integer", "minimum": 0 },
          "binary": { "type": "boolean" },
          "old_size": { "type": "integer", "minimum": 0 },
          "new_size": { "type": "integer", "minimum": 0 }
        }
      }
    },
    "warnings": { "type": "array", "items": { "type": "string" } }
  }
}"##;

const DOCTOR_SCHEMA: &str = r##"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://rit.dev/schemas/v1/doctor.json",
  "title": "rit doctor",
  "type": "object",
  "additionalProperties": false,
  "required": ["worktree", "git_dir", "common_dir", "bare", "status", "checks"],
  "properties": {
    "worktree": { "type": ["string", "null"] },
    "git_dir": { "type": "string" },
    "common_dir": { "type": "string" },
    "bare": { "type": "boolean" },
    "status": { "enum": ["ok", "error"] },
    "checks": {
      "type": "array",
      "items": {
        "type": "object",
        "additionalProperties": false,
        "required": ["name", "severity", "detail"],
        "properties": {
          "name": { "type": "string" },
          "severity": { "enum": ["ok", "warning", "error"] },
          "detail": { "type": "string" }
        }
      }
    }
  }
}"##;

const OPERATIONS_SCHEMA: &str = r##"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://rit.dev/schemas/v1/operations.json",
  "title": "rit operations",
  "type": "object",
  "additionalProperties": false,
  "required": ["records", "warnings"],
  "properties": {
    "records": {
      "type": "array",
      "items": {
        "type": "object",
        "additionalProperties": false,
        "required": ["id", "command", "summary", "before", "after", "changed_paths", "created_object_ids"],
        "properties": {
          "id": { "type": "string" },
          "command": { "type": "string" },
          "summary": { "type": "string" },
          "before": { "$ref": "#/$defs/snapshot" },
          "after": { "$ref": "#/$defs/snapshot" },
          "changed_paths": { "type": "array", "items": { "type": "string" } },
          "created_object_ids": { "type": "array", "items": { "type": "string" } }
        }
      }
    },
    "warnings": {
      "type": "array",
      "items": {
        "type": "object",
        "additionalProperties": false,
        "required": ["line_number", "message"],
        "properties": {
          "line_number": { "type": "integer", "minimum": 1 },
          "message": { "type": "string" }
        }
      }
    }
  },
  "$defs": {
    "snapshot": {
      "type": "object",
      "additionalProperties": false,
      "required": ["head", "branch", "index_checksum"],
      "properties": {
        "head": { "type": ["string", "null"] },
        "branch": { "type": ["string", "null"] },
        "index_checksum": { "type": ["string", "null"] }
      }
    }
  }
}"##;

const IMPACT_SCHEMA: &str = r##"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://rit.dev/schemas/v1/impact.json",
  "title": "rit impact",
  "type": "object",
  "additionalProperties": false,
  "required": ["range", "base", "target", "changed_paths", "changed_packages", "affected_tests", "public_api_changes", "docs_only", "large_file_changes", "reviewer_hints", "semantic", "history_touched_paths", "indexdb_acceleration_available", "indexdb_acceleration_used"],
  "properties": {
    "range": { "type": "string" },
    "base": { "type": "string" },
    "target": { "type": "string" },
    "changed_paths": { "type": "array", "items": { "type": "string" } },
    "changed_packages": { "type": "array", "items": { "type": "string" } },
    "affected_tests": { "type": "array", "items": { "type": "string" } },
    "public_api_changes": { "type": "array", "items": { "type": "string" } },
    "docs_only": { "type": "boolean" },
    "large_file_changes": {
      "type": "array",
      "items": {
        "type": "object",
        "additionalProperties": false,
        "required": ["path", "size"],
        "properties": {
          "path": { "type": "string" },
          "size": { "type": "integer", "minimum": 0 }
        }
      }
    },
    "reviewer_hints": { "type": "array", "items": { "$ref": "#/$defs/hint" } },
    "semantic": { "$ref": "#/$defs/semantic" },
    "history_touched_paths": { "type": "array", "items": { "type": "string" } },
    "indexdb_acceleration_available": { "type": "boolean" },
    "indexdb_acceleration_used": { "type": "boolean" }
  },
  "$defs": {
    "hint": {
      "type": "object",
      "additionalProperties": false,
      "required": ["kind", "path", "detail"],
      "properties": {
        "kind": { "type": "string" },
        "path": { "type": "string" },
        "detail": { "type": "string" }
      }
    },
    "semantic": {
      "type": "object",
      "additionalProperties": false,
      "required": ["files"],
      "properties": {
        "files": {
          "type": "array",
          "items": {
            "type": "object",
            "additionalProperties": false,
            "required": ["path", "category"],
            "properties": {
              "path": { "type": "string" },
              "category": { "enum": ["code", "tests", "docs", "other"] }
            }
          }
        }
      }
    }
  }
}"##;

const INDEXDB_SCHEMA: &str = r##"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://rit.dev/schemas/v1/indexdb.json",
  "title": "rit indexdb",
  "type": "object",
  "additionalProperties": false,
  "required": ["storage", "exists", "schema_version", "healthy", "stale", "stale_reasons"],
  "properties": {
    "exists": { "type": "boolean" },
    "healthy": { "type": "boolean" },
    "stale": { "type": "boolean" },
    "schema_version": { "type": ["integer", "null"] },
    "stale_reasons": { "type": "array", "items": { "type": "string" } },
    "storage": {
      "type": "object",
      "additionalProperties": false,
      "required": ["directory", "database_path", "lock_path", "worktree_directory", "worktree_cache_path", "worktree_lock_path"],
      "properties": {
        "directory": { "type": "string" },
        "database_path": { "type": "string" },
        "lock_path": { "type": "string" },
        "worktree_directory": { "type": "string" },
        "worktree_cache_path": { "type": "string" },
        "worktree_lock_path": { "type": "string" }
      }
    }
  }
}"##;

#[cfg(test)]
mod tests {
    use super::{json_schema, json_schemas};

    #[test]
    fn schema_lookup_returns_named_documents() {
        let status = json_schema("status").expect("status schema should exist");

        assert_eq!(status.name, "status");
        assert!(
            status
                .json
                .contains("\"$id\": \"https://rit.dev/schemas/v1/status.json\"")
        );
        assert!(json_schema("missing").is_none());
    }

    #[test]
    fn schema_list_contains_stable_m23_names() {
        let names = json_schemas()
            .iter()
            .map(|schema| schema.name)
            .collect::<Vec<_>>();

        assert_eq!(
            names,
            vec![
                "status",
                "diff",
                "doctor",
                "operations",
                "impact",
                "indexdb"
            ]
        );
    }
}
