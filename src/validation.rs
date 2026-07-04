use jsonschema::JSONSchema;
use serde_json::Value;

/// Holds context when a response fails validation, used to prompt the model with corrections.
#[derive(Debug, Clone)]
pub struct Correction {
    /// The original malformed output returned by the model.
    pub malformed_output: String,
    /// A description of the validation failure (e.g. schema violations).
    pub error_description: String,
}

/// A validator that compiles a JSON Schema and validates JSON strings against it.
pub struct SchemaValidator {
    schema: JSONSchema,
}

impl SchemaValidator {
    /// Compiles a JSON Schema from a `serde_json::Value`.
    pub fn new(schema_json: Value) -> Result<Self, String> {
        let schema = JSONSchema::compile(&schema_json).map_err(|e| e.to_string())?;
        Ok(Self { schema })
    }

    /// Validates a raw JSON string against the compiled schema.
    pub fn validate(&self, json_str: &str) -> Result<(), String> {
        let value: Value =
            serde_json::from_str(json_str).map_err(|e| format!("Invalid JSON syntax: {}", e))?;

        self.schema.validate(&value).map_err(|errs| {
            let err_msgs: Vec<String> = errs.map(|e| e.to_string()).collect();
            err_msgs.join(", ")
        })
    }
}
