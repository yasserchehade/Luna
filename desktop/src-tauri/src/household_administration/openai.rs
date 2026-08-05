use std::{io::Read, sync::Arc, time::Duration};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use reqwest::StatusCode;

use crate::{
    household_work::{
        HouseholdWorkKind, HouseholdWorkStatus, ProposedActionKind, WorkFact, WorkFactKey,
    },
    HouseholdActionProposal, HouseholdAdministrationRequest, HouseholdClarification,
    HouseholdWorkProposal, IntelligenceUsage, UntrustedHouseholdAdministrationResult,
    MAX_HOUSEHOLD_EXTRACTED_TEXT_CHARS, MAX_MVP_DOCUMENT_BYTES,
};

use super::{HouseholdAdministrationReasoning, ReasoningPortError};

pub const HOUSEHOLD_ADMINISTRATION_CONTRACT_VERSION: &str = "household-administration.v1";
const OPENAI_PROVIDER_ID: &str = "openai";
const OPENAI_RESPONSES_ENDPOINT: &str = "https://api.openai.com/v1/responses";

trait OpenAiTransport: Send + Sync {
    fn post(
        &self,
        api_key: &str,
        request_id: &str,
        timeout: Duration,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, OpenAiTransportFailure>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum OpenAiTransportFailure {
    Status {
        status: StatusCode,
        error_code: Option<String>,
    },
    TimedOut,
    NetworkFailure,
    InvalidResponse,
}

struct ReqwestOpenAiTransport;

impl OpenAiTransport for ReqwestOpenAiTransport {
    fn post(
        &self,
        api_key: &str,
        request_id: &str,
        timeout: Duration,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, OpenAiTransportFailure> {
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(timeout)
            .build()
            .map_err(|_| OpenAiTransportFailure::NetworkFailure)?;
        let response = client
            .post(OPENAI_RESPONSES_ENDPOINT)
            .bearer_auth(api_key)
            .header("X-Client-Request-Id", request_id)
            .json(body)
            .send()
            .map_err(|error| {
                if error.is_timeout() {
                    OpenAiTransportFailure::TimedOut
                } else {
                    OpenAiTransportFailure::NetworkFailure
                }
            })?;
        if !response.status().is_success() {
            let status = response.status();
            let error_code = read_bounded_openai_error_code(response);
            return Err(OpenAiTransportFailure::Status { status, error_code });
        }
        response
            .json()
            .map_err(|_| OpenAiTransportFailure::InvalidResponse)
    }
}

pub struct OpenAiHouseholdAdministrationReasoningAdapter {
    api_key: Vec<u8>,
    model: String,
    transport: Arc<dyn OpenAiTransport>,
}

impl OpenAiHouseholdAdministrationReasoningAdapter {
    pub fn from_env() -> Result<Self, ReasoningPortError> {
        let api_key = std::env::var("OPENAI_API_KEY")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .ok_or(ReasoningPortError::MissingApiKey)?;
        let model = std::env::var("LUNA_OPENAI_MODEL")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .ok_or(ReasoningPortError::ModelUnavailable)?;
        Self::new(api_key, model)
    }

    pub fn new(
        api_key: impl Into<String>,
        model: impl Into<String>,
    ) -> Result<Self, ReasoningPortError> {
        Self::with_transport(api_key, model, Arc::new(ReqwestOpenAiTransport))
    }

    fn with_transport(
        api_key: impl Into<String>,
        model: impl Into<String>,
        transport: Arc<dyn OpenAiTransport>,
    ) -> Result<Self, ReasoningPortError> {
        let api_key = api_key.into();
        if api_key.trim().is_empty() {
            return Err(ReasoningPortError::MissingApiKey);
        }
        let model = model.into();
        if model.trim().is_empty() {
            return Err(ReasoningPortError::ModelUnavailable);
        }
        Ok(Self {
            api_key: api_key.into_bytes(),
            model: model.trim().to_owned(),
            transport,
        })
    }
}

impl HouseholdAdministrationReasoning for OpenAiHouseholdAdministrationReasoningAdapter {
    fn reason(
        &self,
        request: &HouseholdAdministrationRequest,
    ) -> Result<UntrustedHouseholdAdministrationResult, ReasoningPortError> {
        if request.response_schema_version != HOUSEHOLD_ADMINISTRATION_CONTRACT_VERSION {
            return Err(ReasoningPortError::OpenAiContractMismatch);
        }
        let body = openai_household_administration_request(request, &self.model)?;
        let api_key = std::str::from_utf8(&self.api_key)
            .ok()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or(ReasoningPortError::MissingApiKey)?;
        let response = self
            .transport
            .post(
                api_key,
                &request.request_id,
                Duration::from_millis(request.constraints.timeout_ms.max(1)),
                &body,
            )
            .map_err(map_transport_failure)?;
        parse_openai_household_administration_response(&response, request, &self.model)
    }
}

fn openai_household_administration_request(
    request: &HouseholdAdministrationRequest,
    model: &str,
) -> Result<serde_json::Value, ReasoningPortError> {
    let mut bounded_request = request.clone();
    if let Some(source) = bounded_request.source.as_mut() {
        if source.original_size_bytes > MAX_MVP_DOCUMENT_BYTES {
            return Err(ReasoningPortError::RequestTooLarge);
        }
        if source
            .extracted_text
            .as_ref()
            .is_some_and(|text| text.chars().count() > MAX_HOUSEHOLD_EXTRACTED_TEXT_CHARS)
        {
            return Err(ReasoningPortError::RequestTooLarge);
        }
        validate_bounded_original(source)?;
    }
    let mut content = vec![serde_json::json!({
        "type": "input_text",
        "text": serde_json::to_string(&bounded_request)
            .map_err(|_| ReasoningPortError::OpenAiContractMismatch)?
    })];
    if let Some(source) = request.source.as_ref() {
        match source.media_type.as_str() {
            "application/pdf" => content.push(serde_json::json!({
                "type": "input_file",
                "filename": source.filename,
                "file_data": format!("data:{};base64,{}", source.media_type, source.original_base64)
            })),
            "image/jpeg" | "image/png" => content.push(serde_json::json!({
                "type": "input_image",
                "image_url": format!("data:{};base64,{}", source.media_type, source.original_base64),
                "detail": "auto"
            })),
            _ => return Err(ReasoningPortError::UnsupportedMedia),
        }
    }
    Ok(serde_json::json!({
        "model": model,
        "store": false,
        "max_output_tokens": request.constraints.max_output_tokens,
        "input": [
            {
                "role": "system",
                "content": [{
                    "type": "input_text",
                    "text": "You are Luna's household-administration reasoning engine. Use the supplied source, relevant conversation and authorised household context. Return only the requested structured proposal. Never claim authority, approve an action, execute a tool, invent evidence or ask for information already supplied. Ask at most one focused clarification. Corrections and clarifications must patch only the relevant facts and preserve unrelated Household Work state."
                }]
            },
            {"role": "user", "content": content}
        ],
        "text": {
            "format": {
                "type": "json_schema",
                "name": "luna_household_administration_result",
                "strict": true,
                "schema": household_administration_response_schema()
            }
        }
    }))
}

fn validate_bounded_original(
    source: &crate::HouseholdAdministrationSource,
) -> Result<(), ReasoningPortError> {
    if !matches!(
        source.media_type.as_str(),
        "application/pdf" | "image/jpeg" | "image/png"
    ) {
        return Err(ReasoningPortError::UnsupportedMedia);
    }
    let max_encoded_len = (MAX_MVP_DOCUMENT_BYTES.div_ceil(3) * 4) as usize;
    if source.original_base64.len() > max_encoded_len {
        return Err(ReasoningPortError::RequestTooLarge);
    }
    let original = BASE64
        .decode(source.original_base64.as_bytes())
        .map_err(|_| ReasoningPortError::UnsupportedMedia)?;
    if original.len() as u64 > MAX_MVP_DOCUMENT_BYTES {
        return Err(ReasoningPortError::RequestTooLarge);
    }
    if original.len() as u64 != source.original_size_bytes {
        return Err(ReasoningPortError::UnsupportedMedia);
    }
    Ok(())
}

fn map_transport_failure(failure: OpenAiTransportFailure) -> ReasoningPortError {
    match failure {
        OpenAiTransportFailure::TimedOut => ReasoningPortError::Timeout,
        OpenAiTransportFailure::NetworkFailure => ReasoningPortError::NetworkFailure,
        OpenAiTransportFailure::InvalidResponse => ReasoningPortError::StructuredResponseInvalid,
        OpenAiTransportFailure::Status { status, error_code } => {
            if status == StatusCode::UNAUTHORIZED
                || status == StatusCode::FORBIDDEN
                || error_code.as_deref() == Some("invalid_api_key")
            {
                ReasoningPortError::InvalidApiKey
            } else if status == StatusCode::NOT_FOUND
                || error_code.as_deref() == Some("model_not_found")
            {
                ReasoningPortError::ModelUnavailable
            } else if status == StatusCode::TOO_MANY_REQUESTS {
                ReasoningPortError::RateLimited
            } else if status == StatusCode::PAYLOAD_TOO_LARGE
                || error_code.as_deref() == Some("context_length_exceeded")
            {
                ReasoningPortError::RequestTooLarge
            } else if status == StatusCode::UNSUPPORTED_MEDIA_TYPE {
                ReasoningPortError::UnsupportedMedia
            } else if status == StatusCode::REQUEST_TIMEOUT || status == StatusCode::GATEWAY_TIMEOUT
            {
                ReasoningPortError::Timeout
            } else {
                ReasoningPortError::Unavailable
            }
        }
    }
}

fn parse_openai_household_administration_response(
    response: &serde_json::Value,
    request: &HouseholdAdministrationRequest,
    configured_model: &str,
) -> Result<UntrustedHouseholdAdministrationResult, ReasoningPortError> {
    if response
        .get("id")
        .and_then(serde_json::Value::as_str)
        .is_none()
    {
        return Err(ReasoningPortError::OpenAiContractMismatch);
    }
    let response_model = response
        .get("model")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(configured_model);
    let content =
        response_output_text(response).ok_or(ReasoningPortError::StructuredResponseInvalid)?;
    let result: OpenAiHouseholdStructuredResult =
        serde_json::from_str(content).map_err(|_| ReasoningPortError::StructuredResponseInvalid)?;
    Ok(UntrustedHouseholdAdministrationResult {
        request_id: request.request_id.clone(),
        provider_id: OPENAI_PROVIDER_ID.to_owned(),
        model_id: response_model.to_owned(),
        reply: result.reply,
        work: result.work.into_domain(),
        clarification: result.clarification.0.map(|item| item.into_domain()),
        proposed_actions: result
            .proposed_actions
            .into_iter()
            .map(OpenAiHouseholdActionProposal::into_domain)
            .collect(),
        usage: IntelligenceUsage {
            input_tokens: response
                .pointer("/usage/input_tokens")
                .and_then(serde_json::Value::as_u64),
            output_tokens: response
                .pointer("/usage/output_tokens")
                .and_then(serde_json::Value::as_u64),
            estimated_cost_usd: None,
        },
    })
}

fn response_output_text(response: &serde_json::Value) -> Option<&str> {
    response
        .get("output_text")
        .and_then(serde_json::Value::as_str)
        .or_else(|| {
            response
                .get("output")
                .and_then(serde_json::Value::as_array)
                .and_then(|output| {
                    output.iter().find_map(|item| {
                        (item.get("type").and_then(serde_json::Value::as_str) == Some("message"))
                            .then(|| {
                                item.get("content")
                                    .and_then(serde_json::Value::as_array)
                                    .and_then(|content| {
                                        content.iter().find_map(|part| {
                                            (part.get("type").and_then(serde_json::Value::as_str)
                                                == Some("output_text"))
                                            .then(|| {
                                                part.get("text").and_then(serde_json::Value::as_str)
                                            })
                                            .flatten()
                                        })
                                    })
                            })
                            .flatten()
                    })
                })
        })
}

pub(crate) fn household_administration_response_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["reply", "work", "clarification", "proposedActions"],
        "properties": {
            "reply": {"type": "string", "minLength": 1, "maxLength": 4000},
            "work": {
                "type": "object",
                "additionalProperties": false,
                "required": ["operation", "workId", "kind", "summary", "status", "facts", "dueAt", "urgency"],
                "properties": {
                    "operation": {"type": "string", "enum": ["none", "create", "update"]},
                    "workId": {"type": ["string", "null"]},
                    "kind": {"type": ["string", "null"], "enum": ["bill", "renewal", "request", "appointment", "other", null]},
                    "summary": {"type": ["string", "null"], "maxLength": 1024},
                    "status": {"type": ["string", "null"], "enum": ["active", "needsClarification", "awaitingApproval", "inProgress", "monitoring", "completed", "dismissed", "noLongerRelevant", "blocked", null]},
                    "facts": {
                        "type": "array",
                        "maxItems": 16,
                        "items": {
                            "type": "object",
                            "additionalProperties": false,
                            "required": ["key", "value", "evidenceRefs", "certainty"],
                            "properties": {
                                "key": {"type": "string", "enum": ["provider", "property", "account", "amount", "dueDate", "requiredAction", "urgency", "other"]},
                                "value": {"type": "string", "minLength": 1, "maxLength": 1024},
                                "evidenceRefs": {"type": "array", "minItems": 1, "maxItems": 32, "items": {"type": "string", "maxLength": 256}},
                                "certainty": {"type": "string", "enum": ["confirmed", "likely", "unknown"]}
                            }
                        }
                    },
                    "dueAt": {"type": ["string", "null"]},
                    "urgency": {"type": ["string", "null"], "enum": ["low", "normal", "high", null]}
                }
            },
            "clarification": {
                "type": ["object", "null"],
                "additionalProperties": false,
                "required": ["question", "reason", "field"],
                "properties": {
                    "question": {"type": "string", "minLength": 1, "maxLength": 500},
                    "reason": {"type": ["string", "null"], "maxLength": 1024},
                    "field": {"type": ["string", "null"], "enum": ["provider", "property", "account", "amount", "dueDate", "requiredAction", "urgency", "other", null]}
                }
            },
            "proposedActions": {
                "type": "array",
                "maxItems": 4,
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["kind", "summary", "arguments", "approvalRequired"],
                    "properties": {
                        "kind": {"type": "string", "enum": ["draftReply", "reminder"]},
                        "summary": {"type": "string", "minLength": 1, "maxLength": 500},
                        "arguments": {
                            "type": "object",
                            "additionalProperties": false,
                            "required": ["recipient", "subject", "body", "remindAt", "message"],
                            "properties": {
                                "recipient": {"type": ["string", "null"], "maxLength": 1024},
                                "subject": {"type": ["string", "null"], "maxLength": 1024},
                                "body": {"type": ["string", "null"], "maxLength": 1024},
                                "remindAt": {"type": ["string", "null"], "maxLength": 1024},
                                "message": {"type": ["string", "null"], "maxLength": 1024}
                            }
                        },
                        "approvalRequired": {"type": "boolean"}
                    }
                }
            }
        }
    })
}

#[derive(serde::Deserialize)]
#[serde(transparent)]
struct RequiredNullable<T>(Option<T>);

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OpenAiHouseholdStructuredResult {
    reply: String,
    work: OpenAiHouseholdWorkProposal,
    clarification: RequiredNullable<OpenAiHouseholdClarification>,
    proposed_actions: Vec<OpenAiHouseholdActionProposal>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OpenAiHouseholdWorkProposal {
    operation: crate::HouseholdWorkOperation,
    work_id: RequiredNullable<String>,
    kind: RequiredNullable<HouseholdWorkKind>,
    summary: RequiredNullable<String>,
    status: RequiredNullable<HouseholdWorkStatus>,
    facts: Vec<WorkFact>,
    due_at: RequiredNullable<String>,
    urgency: RequiredNullable<String>,
}

impl OpenAiHouseholdWorkProposal {
    fn into_domain(self) -> HouseholdWorkProposal {
        HouseholdWorkProposal {
            operation: self.operation,
            work_id: self.work_id.0,
            kind: self.kind.0,
            summary: self.summary.0,
            status: self.status.0,
            facts: self.facts,
            due_at: self.due_at.0,
            urgency: self.urgency.0,
        }
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OpenAiHouseholdClarification {
    question: String,
    reason: RequiredNullable<String>,
    field: RequiredNullable<WorkFactKey>,
}

impl OpenAiHouseholdClarification {
    fn into_domain(self) -> HouseholdClarification {
        HouseholdClarification {
            question: self.question,
            reason: self.reason.0,
            field: self.field.0,
        }
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OpenAiHouseholdActionProposal {
    kind: ProposedActionKind,
    summary: String,
    arguments: OpenAiHouseholdActionArguments,
    approval_required: bool,
}

impl OpenAiHouseholdActionProposal {
    fn into_domain(self) -> HouseholdActionProposal {
        let arguments = [
            ("recipient", self.arguments.recipient.0),
            ("subject", self.arguments.subject.0),
            ("body", self.arguments.body.0),
            ("remindAt", self.arguments.remind_at.0),
            ("message", self.arguments.message.0),
        ]
        .into_iter()
        .filter_map(|(key, value)| value.map(|value| (key.to_owned(), value)))
        .collect();
        HouseholdActionProposal {
            kind: self.kind,
            summary: self.summary,
            arguments,
            approval_required: self.approval_required,
        }
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OpenAiHouseholdActionArguments {
    recipient: RequiredNullable<String>,
    subject: RequiredNullable<String>,
    body: RequiredNullable<String>,
    remind_at: RequiredNullable<String>,
    message: RequiredNullable<String>,
}

fn read_bounded_openai_error_code(response: reqwest::blocking::Response) -> Option<String> {
    const MAX_ERROR_BODY_BYTES: u64 = 8 * 1024;
    let mut body = Vec::new();
    response
        .take(MAX_ERROR_BODY_BYTES)
        .read_to_end(&mut body)
        .ok()?;
    let error = serde_json::from_slice::<serde_json::Value>(&body).ok()?;
    error
        .pointer("/error/code")
        .or_else(|| error.pointer("/error/type"))?
        .as_str()
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use serde_json::json;

    use crate::{
        household_administration::{HouseholdAdministrationReasoning, ReasoningPortError},
        HouseholdAdministrationRequest, HouseholdAdministrationSource,
        IntelligenceExecutionConstraints,
    };

    use super::{
        OpenAiHouseholdAdministrationReasoningAdapter, OpenAiTransport, OpenAiTransportFailure,
    };

    struct FixtureTransport {
        response: Result<serde_json::Value, OpenAiTransportFailure>,
        requests: Mutex<Vec<serde_json::Value>>,
    }

    impl OpenAiTransport for FixtureTransport {
        fn post(
            &self,
            _api_key: &str,
            _request_id: &str,
            _timeout: std::time::Duration,
            body: &serde_json::Value,
        ) -> Result<serde_json::Value, OpenAiTransportFailure> {
            self.requests.lock().expect("requests").push(body.clone());
            self.response.clone()
        }
    }

    fn request() -> HouseholdAdministrationRequest {
        HouseholdAdministrationRequest {
            request_id: "luna-request-41".to_owned(),
            conversation_id: 41,
            current_message: "Take care of this.".to_owned(),
            relevant_conversation: Vec::new(),
            source: Some(HouseholdAdministrationSource {
                reference: "document-41".to_owned(),
                filename: "sanitised-bill.png".to_owned(),
                media_type: "image/png".to_owned(),
                original_base64: "c2FuaXRpc2VkLWltYWdl".to_owned(),
                extracted_text: None,
                original_size_bytes: 15,
                extracted_text_truncated: false,
            }),
            household_context: Vec::new(),
            active_household_work: Vec::new(),
            source_linked_household_work: None,
            available_tools: Vec::new(),
            authority_and_approval_constraints: "Luna validates authority.".to_owned(),
            response_schema_version: "household-administration.v1".to_owned(),
            constraints: IntelligenceExecutionConstraints {
                timeout_ms: 30_000,
                max_output_tokens: 1_200,
            },
        }
    }

    #[test]
    fn direct_openai_adapter_uses_strict_output_and_luna_owned_envelope_metadata() {
        let transport = Arc::new(FixtureTransport {
            response: Ok(json!({
                "id": "resp_openai_41",
                "model": "gpt-5.6",
                "output_text": serde_json::to_string(&json!({
                    "reply": "I found a bill and created Household Work for it.",
                    "work": {
                        "operation": "create",
                        "workId": null,
                        "kind": "bill",
                        "summary": "Pay the sanitised household bill",
                        "status": "active",
                        "facts": [],
                        "dueAt": null,
                        "urgency": "normal"
                    },
                    "clarification": null,
                    "proposedActions": []
                })).expect("structured result"),
                "usage": {"input_tokens": 321, "output_tokens": 87}
            })),
            requests: Mutex::new(Vec::new()),
        });
        let adapter = OpenAiHouseholdAdministrationReasoningAdapter::with_transport(
            "server-secret",
            "gpt-5.6",
            transport.clone(),
        )
        .expect("configured adapter");

        let result = adapter.reason(&request()).expect("valid OpenAI result");

        assert_eq!(result.request_id, "luna-request-41");
        assert_eq!(result.provider_id, "openai");
        assert_eq!(result.model_id, "gpt-5.6");
        assert_eq!(result.usage.input_tokens, Some(321));
        assert_eq!(result.usage.output_tokens, Some(87));

        let requests = transport.requests.lock().expect("requests");
        let body = &requests[0];
        assert_eq!(body["model"], "gpt-5.6");
        assert_eq!(body["store"], false);
        assert_eq!(body["text"]["format"]["type"], "json_schema");
        assert_eq!(body["text"]["format"]["strict"], true);
        let schema = &body["text"]["format"]["schema"];
        assert!(schema["properties"].get("requestId").is_none());
        assert!(schema["properties"].get("providerId").is_none());
        assert!(schema["properties"].get("modelId").is_none());
        assert!(schema["properties"].get("usage").is_none());
        let user_content = body["input"][1]["content"].as_array().expect("content");
        assert_eq!(user_content[1]["type"], "input_image");
        assert!(user_content[1]["image_url"]
            .as_str()
            .expect("image data URL")
            .starts_with("data:image/png;base64,"));
        assert!(!user_content[0]["text"]
            .as_str()
            .expect("bounded request")
            .contains("c2FuaXRpc2VkLWltYWdl"));
    }

    #[test]
    fn direct_openai_adapter_rejects_missing_model_owned_fields() {
        let transport = Arc::new(FixtureTransport {
            response: Ok(json!({
                "id": "resp_openai_42",
                "model": "gpt-5.6",
                "output_text": "{\"reply\":\"Incomplete\"}",
                "usage": {"input_tokens": 1, "output_tokens": 1}
            })),
            requests: Mutex::new(Vec::new()),
        });
        let adapter = OpenAiHouseholdAdministrationReasoningAdapter::with_transport(
            "server-secret",
            "gpt-5.6",
            transport,
        )
        .expect("configured adapter");

        assert_eq!(
            adapter.reason(&request()),
            Err(ReasoningPortError::StructuredResponseInvalid)
        );
    }

    #[test]
    fn direct_openai_adapter_maps_provider_failures_to_specific_categories() {
        let cases = [
            (
                OpenAiTransportFailure::Status {
                    status: reqwest::StatusCode::UNAUTHORIZED,
                    error_code: Some("invalid_api_key".to_owned()),
                },
                ReasoningPortError::InvalidApiKey,
            ),
            (
                OpenAiTransportFailure::Status {
                    status: reqwest::StatusCode::NOT_FOUND,
                    error_code: Some("model_not_found".to_owned()),
                },
                ReasoningPortError::ModelUnavailable,
            ),
            (
                OpenAiTransportFailure::Status {
                    status: reqwest::StatusCode::TOO_MANY_REQUESTS,
                    error_code: Some("rate_limit_exceeded".to_owned()),
                },
                ReasoningPortError::RateLimited,
            ),
            (
                OpenAiTransportFailure::Status {
                    status: reqwest::StatusCode::PAYLOAD_TOO_LARGE,
                    error_code: None,
                },
                ReasoningPortError::RequestTooLarge,
            ),
            (
                OpenAiTransportFailure::Status {
                    status: reqwest::StatusCode::UNSUPPORTED_MEDIA_TYPE,
                    error_code: None,
                },
                ReasoningPortError::UnsupportedMedia,
            ),
            (
                OpenAiTransportFailure::TimedOut,
                ReasoningPortError::Timeout,
            ),
            (
                OpenAiTransportFailure::NetworkFailure,
                ReasoningPortError::NetworkFailure,
            ),
        ];

        for (transport_failure, expected) in cases {
            let adapter = OpenAiHouseholdAdministrationReasoningAdapter::with_transport(
                "server-secret",
                "gpt-5.6",
                Arc::new(FixtureTransport {
                    response: Err(transport_failure),
                    requests: Mutex::new(Vec::new()),
                }),
            )
            .expect("configured adapter");
            assert_eq!(adapter.reason(&request()), Err(expected));
        }
    }

    #[test]
    fn direct_openai_adapter_sends_pdf_as_a_file_part() {
        let transport = Arc::new(FixtureTransport {
            response: Ok(json!({
                "id": "resp_openai_pdf",
                "model": "gpt-5.6",
                "output_text": serde_json::to_string(&json!({
                    "reply": "I read the PDF.",
                    "work": {
                        "operation": "none",
                        "workId": null,
                        "kind": null,
                        "summary": null,
                        "status": null,
                        "facts": [],
                        "dueAt": null,
                        "urgency": null
                    },
                    "clarification": null,
                    "proposedActions": []
                })).expect("structured result"),
                "usage": {"input_tokens": 20, "output_tokens": 8}
            })),
            requests: Mutex::new(Vec::new()),
        });
        let adapter = OpenAiHouseholdAdministrationReasoningAdapter::with_transport(
            "server-secret",
            "gpt-5.6",
            transport.clone(),
        )
        .expect("configured adapter");
        let mut pdf_request = request();
        let source = pdf_request.source.as_mut().expect("source");
        source.filename = "sanitised.pdf".to_owned();
        source.media_type = "application/pdf".to_owned();

        adapter.reason(&pdf_request).expect("valid PDF request");

        let requests = transport.requests.lock().expect("requests");
        let content = requests[0]["input"][1]["content"]
            .as_array()
            .expect("content");
        assert_eq!(content[1]["type"], "input_file");
        assert_eq!(content[1]["filename"], "sanitised.pdf");
        assert!(content[1]["file_data"]
            .as_str()
            .expect("file data URL")
            .starts_with("data:application/pdf;base64,"));
    }

    #[test]
    fn direct_openai_adapter_fails_closed_for_configuration_and_contract_mismatch() {
        assert!(matches!(
            OpenAiHouseholdAdministrationReasoningAdapter::new("", "gpt-5.6"),
            Err(ReasoningPortError::MissingApiKey)
        ));
        assert!(matches!(
            OpenAiHouseholdAdministrationReasoningAdapter::new("server-secret", ""),
            Err(ReasoningPortError::ModelUnavailable)
        ));
        let adapter = OpenAiHouseholdAdministrationReasoningAdapter::with_transport(
            "server-secret",
            "gpt-5.6",
            Arc::new(FixtureTransport {
                response: Err(OpenAiTransportFailure::NetworkFailure),
                requests: Mutex::new(Vec::new()),
            }),
        )
        .expect("configured adapter");
        let mut incompatible_request = request();
        incompatible_request.response_schema_version = "household-administration.v2".to_owned();
        assert_eq!(
            adapter.reason(&incompatible_request),
            Err(ReasoningPortError::OpenAiContractMismatch)
        );
    }
}
