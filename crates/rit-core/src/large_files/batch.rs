use crate::{LargeFileBackendKind, LargeFilePointer, Result, RitError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Git LFS Batch API media type.
pub const LFS_BATCH_MEDIA_TYPE: &str = "application/vnd.git-lfs+json";

/// Git LFS Batch API operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LfsBatchOperation {
    /// Request download actions.
    Download,
    /// Request upload actions.
    Upload,
}

/// Optional ref metadata sent to the Batch API.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LfsBatchRef {
    /// Fully-qualified server ref name.
    pub name: String,
}

impl LfsBatchRef {
    /// Creates a Batch API ref descriptor.
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

/// Object descriptor used in Batch API requests.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LfsBatchObject {
    /// Git LFS object SHA-256.
    pub oid: String,
    /// Object size in bytes.
    pub size: u64,
}

impl LfsBatchObject {
    /// Creates an object descriptor from a Git LFS pointer.
    pub fn from_pointer(pointer: &LargeFilePointer) -> Result<Self> {
        if pointer.backend != LargeFileBackendKind::Lfs {
            return Err(RitError::invalid_input(format!(
                "cannot use {} pointer in Git LFS batch request",
                pointer.backend.as_str()
            )));
        }
        Ok(Self {
            oid: pointer.object_id.clone(),
            size: pointer.size,
        })
    }
}

/// Git LFS Batch API request body.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LfsBatchRequest {
    /// Requested operation.
    pub operation: LfsBatchOperation,
    /// Supported transfer adapters.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub transfers: Vec<String>,
    /// Optional server ref context.
    #[serde(rename = "ref", skip_serializing_if = "Option::is_none")]
    pub ref_property: Option<LfsBatchRef>,
    /// Objects participating in the transfer.
    pub objects: Vec<LfsBatchObject>,
}

impl LfsBatchRequest {
    /// Creates a Batch API request using the basic transfer adapter.
    pub fn new(operation: LfsBatchOperation, objects: Vec<LfsBatchObject>) -> Self {
        Self {
            operation,
            transfers: vec!["basic".to_owned()],
            ref_property: None,
            objects,
        }
    }

    /// Sets the optional server ref context.
    pub fn with_ref(mut self, ref_name: impl Into<String>) -> Self {
        self.ref_property = Some(LfsBatchRef::new(ref_name));
        self
    }

    /// Encodes the request as Git LFS Batch API JSON.
    pub fn to_json(&self) -> Result<Vec<u8>> {
        serde_json::to_vec(self).map_err(|error| {
            RitError::invalid_input(format!("failed to encode Git LFS batch request: {error}"))
        })
    }
}

/// Git LFS Batch API response body.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
pub struct LfsBatchResponse {
    /// Server-selected transfer adapter. Missing means `basic`.
    #[serde(default)]
    pub transfer: Option<String>,
    /// Per-object transfer results.
    #[serde(default)]
    pub objects: Vec<LfsBatchObjectResponse>,
}

impl LfsBatchResponse {
    /// Parses a Git LFS Batch API JSON response.
    pub fn from_json(data: &[u8]) -> Result<Self> {
        serde_json::from_slice(data).map_err(|error| {
            RitError::invalid_input(format!("failed to parse Git LFS batch response: {error}"))
        })
    }

    /// Returns the selected transfer adapter, defaulting to `basic`.
    pub fn transfer_adapter(&self) -> &str {
        self.transfer.as_deref().unwrap_or("basic")
    }
}

/// One object entry in a Batch API response.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
pub struct LfsBatchObjectResponse {
    /// Git LFS object SHA-256.
    pub oid: String,
    /// Object size in bytes.
    pub size: u64,
    /// Whether this object's transfer URL is already authenticated.
    #[serde(default)]
    pub authenticated: bool,
    /// Transfer actions such as `download`, `upload`, or `verify`.
    #[serde(default)]
    pub actions: BTreeMap<String, LfsBatchAction>,
    /// Per-object error, when the server cannot transfer this object.
    #[serde(default)]
    pub error: Option<LfsBatchObjectError>,
}

/// One transfer action returned by the Batch API.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
pub struct LfsBatchAction {
    /// URL or adapter-specific location for the action.
    pub href: String,
    /// Optional request headers for the action.
    #[serde(default)]
    pub header: BTreeMap<String, String>,
    /// Relative expiration in seconds.
    #[serde(default)]
    pub expires_in: Option<i32>,
    /// Absolute RFC 3339 expiration timestamp.
    #[serde(default)]
    pub expires_at: Option<String>,
}

/// Per-object Batch API error.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
pub struct LfsBatchObjectError {
    /// Server error code.
    pub code: u16,
    /// Human-readable server message.
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_batch_download_request() {
        let pointer = LargeFilePointer::new(
            LargeFileBackendKind::Lfs,
            "4d7a214614ab2935c943f9e0ff69d22eadbb8f32b1258daaa5e2ca24d17e2393",
            123,
        );
        let object = LfsBatchObject::from_pointer(&pointer).expect("object should build");
        let request = LfsBatchRequest::new(LfsBatchOperation::Download, vec![object])
            .with_ref("refs/heads/main");

        let json = String::from_utf8(request.to_json().expect("request should encode"))
            .expect("json should be utf8");

        assert!(json.contains("\"operation\":\"download\""));
        assert!(json.contains("\"transfers\":[\"basic\"]"));
        assert!(json.contains("\"ref\":{\"name\":\"refs/heads/main\"}"));
        assert!(json.contains(
            "\"oid\":\"4d7a214614ab2935c943f9e0ff69d22eadbb8f32b1258daaa5e2ca24d17e2393\""
        ));
        assert!(json.contains("\"size\":123"));
    }

    #[test]
    fn parses_batch_response_actions_and_errors() {
        let response = LfsBatchResponse::from_json(
            br#"{
                "transfer": "basic",
                "objects": [
                    {
                        "oid": "1111111111111111111111111111111111111111111111111111111111111111",
                        "size": 10,
                        "authenticated": true,
                        "actions": {
                            "download": {
                                "href": "https://example.test/object",
                                "header": { "Authorization": "RemoteAuth token" },
                                "expires_in": 60
                            }
                        }
                    },
                    {
                        "oid": "2222222222222222222222222222222222222222222222222222222222222222",
                        "size": 20,
                        "error": { "code": 404, "message": "missing" }
                    }
                ]
            }"#,
        )
        .expect("response should parse");

        assert_eq!(response.transfer_adapter(), "basic");
        assert_eq!(response.objects.len(), 2);
        let action = response.objects[0]
            .actions
            .get("download")
            .expect("download action should exist");
        assert_eq!(action.href, "https://example.test/object");
        assert_eq!(
            action.header.get("Authorization").map(String::as_str),
            Some("RemoteAuth token")
        );
        assert_eq!(action.expires_in, Some(60));
        assert_eq!(
            response.objects[1].error,
            Some(LfsBatchObjectError {
                code: 404,
                message: "missing".to_owned(),
            })
        );
    }

    #[test]
    fn missing_response_transfer_defaults_to_basic() {
        let response =
            LfsBatchResponse::from_json(br#"{ "objects": [] }"#).expect("response should parse");

        assert_eq!(response.transfer_adapter(), "basic");
    }
}
