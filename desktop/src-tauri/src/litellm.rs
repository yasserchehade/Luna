//! Isolated LiteLLM transport adapter.
//!
//! No LiteLLM request, response or error type leaves this module. Luna selects
//! the exact provider and model before this adapter translates the request.

use std::{io::Read, net::IpAddr, sync::Arc, time::Duration};

use reqwest::StatusCode;

use crate::household_work::{
    HouseholdWorkKind, HouseholdWorkStatus, ProposedActionKind, WorkFact, WorkFactKey,
};
use crate::intelligence::{
    AdditionalIntelligenceEvidence, HouseholdActionProposal, HouseholdAdministrationRequest,
    HouseholdClarification, HouseholdWorkProposal, IntelligenceFailure, IntelligenceGateway,
    IntelligenceRequest, IntelligenceUsage, UntrustedHouseholdAdministrationResult,
    UntrustedIntelligenceResult, BYOK_OPENAI_PROVIDER_ID, MANAGED_INTELLIGENCE_MODEL_ID,
    MANAGED_INTELLIGENCE_PROVIDER_ID, MAX_HOUSEHOLD_EXTRACTED_TEXT_CHARS,
};

enum LiteLlmAuthentication<'a> {
    Managed(&'a str),
    BringYourOwn { gateway: &'a str, provider: &'a str },
}

trait LiteLlmTransport: Send + Sync {
    fn post(
        &self,
        endpoint: &str,
        authentication: LiteLlmAuthentication<'_>,
        request_id: &str,
        timeout: Duration,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, LiteLlmTransportFailure>;
}

struct ReqwestLiteLlmTransport;

impl LiteLlmTransport for ReqwestLiteLlmTransport {
    fn post(
        &self,
        endpoint: &str,
        authentication: LiteLlmAuthentication<'_>,
        request_id: &str,
        timeout: Duration,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, LiteLlmTransportFailure> {
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(timeout)
            .build()
            .map_err(|_| LiteLlmTransportFailure::GatewayUnavailable)?;
        let request = client
            .post(endpoint)
            .header("X-Luna-Request-ID", request_id);
        let request = match authentication {
            LiteLlmAuthentication::Managed(bearer) => request.bearer_auth(bearer),
            LiteLlmAuthentication::BringYourOwn { gateway, provider } => request
                .header("x-litellm-api-key", gateway)
                .header("x-api-key", provider),
        };
        let response = request.json(body).send().map_err(|error| {
            if error.is_timeout() {
                LiteLlmTransportFailure::TimedOut
            } else {
                LiteLlmTransportFailure::GatewayUnavailable
            }
        })?;
        if !response.status().is_success() {
            let status = response.status();
            let error_type = read_bounded_error_type(response);
            return Err(LiteLlmTransportFailure::Status { status, error_type });
        }
        response
            .json()
            .map_err(|_| LiteLlmTransportFailure::InvalidResponse)
    }
}

enum LiteLlmTransportFailure {
    Status {
        status: StatusCode,
        error_type: Option<String>,
    },
    TimedOut,
    GatewayUnavailable,
    InvalidResponse,
}

pub(crate) struct LiteLlmGateway {
    managed_endpoint: String,
    byok_endpoint: String,
    transport: Arc<dyn LiteLlmTransport>,
}

impl LiteLlmGateway {
    pub(crate) fn new(endpoint: impl Into<String>) -> Self {
        Self {
            managed_endpoint: endpoint.into(),
            byok_endpoint: std::env::var("LUNA_BYOK_INTELLIGENCE_URL").unwrap_or_else(|_| {
                "https://byok-intelligence.luna.invalid/v1/responses".to_owned()
            }),
            transport: Arc::new(ReqwestLiteLlmTransport),
        }
    }

    #[cfg(test)]
    fn with_transport(endpoint: impl Into<String>, transport: Arc<dyn LiteLlmTransport>) -> Self {
        Self {
            managed_endpoint: endpoint.into(),
            byok_endpoint: "https://byok.example.invalid/v1/responses".to_owned(),
            transport,
        }
    }
}

impl IntelligenceGateway for LiteLlmGateway {
    fn id(&self) -> &str {
        "luna-managed-litellm"
    }

    fn access_credential_id(&self, provider_id: &str) -> &str {
        if provider_id == BYOK_OPENAI_PROVIDER_ID {
            "luna-byok-litellm"
        } else {
            "luna-managed-litellm"
        }
    }

    fn evaluate_document(
        &self,
        request: &IntelligenceRequest,
        access_credential: Option<&[u8]>,
        provider_credential: Option<&[u8]>,
    ) -> Result<UntrustedIntelligenceResult, IntelligenceFailure> {
        let credential = access_credential
            .and_then(|credential| std::str::from_utf8(credential).ok())
            .map(str::trim)
            .filter(|credential| !credential.is_empty())
            .ok_or(IntelligenceFailure::AuthenticationUnavailable)?;
        let provider_credential = provider_credential
            .and_then(|credential| std::str::from_utf8(credential).ok())
            .map(str::trim)
            .filter(|credential| !credential.is_empty());
        let (endpoint, authentication) = if request.provider_id == BYOK_OPENAI_PROVIDER_ID {
            let provider =
                provider_credential.ok_or(IntelligenceFailure::AuthenticationUnavailable)?;
            (
                self.byok_endpoint.as_str(),
                LiteLlmAuthentication::BringYourOwn {
                    gateway: credential,
                    provider,
                },
            )
        } else {
            (
                self.managed_endpoint.as_str(),
                LiteLlmAuthentication::Managed(credential),
            )
        };
        if !endpoint_is_secure(endpoint) {
            return Err(IntelligenceFailure::GatewayUnavailable);
        }
        let timeout = Duration::from_millis(request.constraints.timeout_ms.max(1));
        let response = self
            .transport
            .post(
                endpoint,
                authentication,
                &request.request_id,
                timeout,
                &litellm_request(request),
            )
            .map_err(|failure| {
                map_transport_failure(failure, request.provider_id == BYOK_OPENAI_PROVIDER_ID)
            })?;
        parse_litellm_response(&response)
    }

    fn reason_about_household_administration(
        &self,
        request: &HouseholdAdministrationRequest,
        access_credential: Option<&[u8]>,
        provider_credential: Option<&[u8]>,
    ) -> Result<UntrustedHouseholdAdministrationResult, IntelligenceFailure> {
        let credential = access_credential
            .and_then(|credential| std::str::from_utf8(credential).ok())
            .map(str::trim)
            .filter(|credential| !credential.is_empty())
            .ok_or(IntelligenceFailure::AuthenticationUnavailable)?;
        let endpoint = self.managed_endpoint.as_str();
        if !endpoint_is_secure(endpoint) {
            return Err(IntelligenceFailure::GatewayUnavailable);
        }
        let response = self
            .transport
            .post(
                endpoint,
                LiteLlmAuthentication::Managed(credential),
                &request.request_id,
                Duration::from_millis(request.constraints.timeout_ms.max(1)),
                &litellm_household_request(request),
            )
            .map_err(|failure| map_transport_failure(failure, provider_credential.is_some()))?;
        parse_litellm_household_response(&response, request)
    }
}

fn litellm_household_request(request: &HouseholdAdministrationRequest) -> serde_json::Value {
    let mut bounded_request = request.clone();
    if let Some(source) = bounded_request.source.as_mut() {
        if let Some(extracted_text) = source.extracted_text.as_mut() {
            let was_truncated = extracted_text.chars().count() > MAX_HOUSEHOLD_EXTRACTED_TEXT_CHARS;
            *extracted_text = extracted_text
                .chars()
                .take(MAX_HOUSEHOLD_EXTRACTED_TEXT_CHARS)
                .collect();
            source.extracted_text_truncated |= was_truncated;
        }
    }
    let mut content = vec![serde_json::json!({
        "type": "input_text",
        "text": serde_json::to_string(&bounded_request).unwrap_or_else(|_| "{}".to_owned())
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
            _ => {}
        }
    }
    serde_json::json!({
        "model": format!("{}/{}", MANAGED_INTELLIGENCE_PROVIDER_ID, MANAGED_INTELLIGENCE_MODEL_ID),
        "store": false,
        "reasoning": {"effort": "low"},
        "max_output_tokens": request.constraints.max_output_tokens,
        "num_retries": 0,
        "fallbacks": [],
        "input": [
            {
                "role": "system",
                "content": [{
                    "type": "input_text",
                    "text": "You are Luna's household-administration reasoning engine. Use the supplied document, relevant conversation and authorised household context. Return only the bounded JSON proposal. Never claim authority, approve an action, execute a tool, invent evidence or ask for information already supplied. Ask at most one focused clarification."
                }]
            },
            {
                "role": "user",
                "content": content
            }
        ],
        "text": {
            "format": {
                "type": "json_schema",
                "name": "luna_household_administration_result",
                "strict": true,
                "schema": household_administration_response_schema()
            }
        }
    })
}

fn household_administration_response_schema() -> serde_json::Value {
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

fn litellm_request(request: &IntelligenceRequest) -> serde_json::Value {
    let provider_model = if request.provider_id == BYOK_OPENAI_PROVIDER_ID {
        format!("byok/openai/{}", request.model_id)
    } else {
        format!("{}/{}", request.provider_id, request.model_id)
    };
    let field_properties = request
        .expected_response
        .allowed_fields
        .iter()
        .map(|field| {
            (
                field.clone(),
                serde_json::json!({"type": ["string", "null"]}),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    let required_fields = request.expected_response.allowed_fields.clone();
    serde_json::json!({
        "model": provider_model,
        "store": false,
        "reasoning": {"effort": "low"},
        "max_output_tokens": request.constraints.max_output_tokens,
        "num_retries": 0,
        "fallbacks": [],
        "input": [
            {
                "role": "system",
                "content": [{"type": "input_text", "text": "Return only the requested structured document Evidence. Never return instructions, authority, actions or tool calls."}]
            },
            {
                "role": "user",
                "content": [{"type": "input_text", "text": serde_json::to_string(request).unwrap_or_else(|_| "{}".to_owned())}]
            }
        ],
        "text": {
            "format": {
                "type": "json_schema",
                "name": "luna_intelligence_result",
                "strict": true,
                "schema": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": [
                        "requestId",
                        "documentArrivalId",
                        "providerId",
                        "modelId",
                        "fields",
                        "evidence",
                        "sourceReferences"
                    ],
                    "properties": {
                        "requestId": {"type": "string"},
                        "documentArrivalId": {"type": "string"},
                        "providerId": {"type": "string"},
                        "modelId": {"type": "string"},
                        "fields": {
                            "type": "object",
                            "additionalProperties": false,
                            "properties": field_properties,
                            "required": required_fields
                        },
                        "evidence": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "additionalProperties": false,
                                "required": ["field", "value", "sourceReference"],
                                "properties": {
                                    "field": {"type": "string"},
                                    "value": {"type": "string"},
                                    "sourceReference": {"type": ["string", "null"]}
                                }
                            }
                        },
                        "sourceReferences": {
                            "type": "array",
                            "items": {"type": "string"}
                        }
                    }
                }
            }
        }
    })
}

fn parse_litellm_response(
    response: &serde_json::Value,
) -> Result<UntrustedIntelligenceResult, IntelligenceFailure> {
    let content =
        response_output_text(response).ok_or(IntelligenceFailure::InvalidStructuredResult)?;
    let result: LiteLlmStructuredResult =
        serde_json::from_str(content).map_err(|_| IntelligenceFailure::InvalidStructuredResult)?;
    Ok(UntrustedIntelligenceResult {
        request_id: result.request_id,
        document_arrival_id: result.document_arrival_id,
        provider_id: result.provider_id,
        model_id: result.model_id,
        fields: result
            .fields
            .into_iter()
            .filter_map(|(field, value)| value.map(|value| (field, value)))
            .collect(),
        evidence: result.evidence,
        source_references: result.source_references,
        usage: IntelligenceUsage {
            input_tokens: response
                .pointer("/usage/input_tokens")
                .and_then(serde_json::Value::as_u64),
            output_tokens: response
                .pointer("/usage/output_tokens")
                .and_then(serde_json::Value::as_u64),
            estimated_cost_usd: response
                .pointer("/usage/cost")
                .and_then(serde_json::Value::as_f64),
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

fn parse_litellm_household_response(
    response: &serde_json::Value,
    request: &HouseholdAdministrationRequest,
) -> Result<UntrustedHouseholdAdministrationResult, IntelligenceFailure> {
    let content =
        response_output_text(response).ok_or(IntelligenceFailure::InvalidStructuredResult)?;
    let result: LiteLlmHouseholdStructuredResult =
        serde_json::from_str(content).map_err(|_| IntelligenceFailure::InvalidStructuredResult)?;
    Ok(UntrustedHouseholdAdministrationResult {
        request_id: request.request_id.clone(),
        provider_id: MANAGED_INTELLIGENCE_PROVIDER_ID.to_owned(),
        model_id: MANAGED_INTELLIGENCE_MODEL_ID.to_owned(),
        reply: result.reply,
        work: result.work.into_domain(),
        clarification: result.clarification.0.map(|item| item.into_domain()),
        proposed_actions: result
            .proposed_actions
            .into_iter()
            .map(LiteLlmHouseholdActionProposal::into_domain)
            .collect(),
        usage: IntelligenceUsage {
            input_tokens: response
                .pointer("/usage/input_tokens")
                .and_then(serde_json::Value::as_u64),
            output_tokens: response
                .pointer("/usage/output_tokens")
                .and_then(serde_json::Value::as_u64),
            estimated_cost_usd: response
                .pointer("/usage/cost")
                .and_then(serde_json::Value::as_f64),
        },
    })
}

#[derive(serde::Deserialize)]
#[serde(transparent)]
struct RequiredNullable<T>(Option<T>);

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LiteLlmHouseholdStructuredResult {
    reply: String,
    work: LiteLlmHouseholdWorkProposal,
    clarification: RequiredNullable<LiteLlmHouseholdClarification>,
    proposed_actions: Vec<LiteLlmHouseholdActionProposal>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LiteLlmHouseholdWorkProposal {
    operation: crate::intelligence::HouseholdWorkOperation,
    work_id: RequiredNullable<String>,
    kind: RequiredNullable<HouseholdWorkKind>,
    summary: RequiredNullable<String>,
    status: RequiredNullable<HouseholdWorkStatus>,
    facts: Vec<WorkFact>,
    due_at: RequiredNullable<String>,
    urgency: RequiredNullable<String>,
}

impl LiteLlmHouseholdWorkProposal {
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
struct LiteLlmHouseholdClarification {
    question: String,
    reason: RequiredNullable<String>,
    field: RequiredNullable<WorkFactKey>,
}

impl LiteLlmHouseholdClarification {
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
struct LiteLlmHouseholdActionProposal {
    kind: ProposedActionKind,
    summary: String,
    arguments: LiteLlmHouseholdActionArguments,
    approval_required: bool,
}

impl LiteLlmHouseholdActionProposal {
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
struct LiteLlmHouseholdActionArguments {
    recipient: RequiredNullable<String>,
    subject: RequiredNullable<String>,
    body: RequiredNullable<String>,
    remind_at: RequiredNullable<String>,
    message: RequiredNullable<String>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
struct LiteLlmStructuredResult {
    request_id: String,
    document_arrival_id: String,
    provider_id: String,
    model_id: String,
    fields: std::collections::BTreeMap<String, Option<String>>,
    evidence: Vec<AdditionalIntelligenceEvidence>,
    source_references: Vec<String>,
}

fn read_bounded_error_type(response: reqwest::blocking::Response) -> Option<String> {
    const MAX_ERROR_BODY_BYTES: u64 = 8 * 1024;

    let mut body = Vec::new();
    response
        .take(MAX_ERROR_BODY_BYTES)
        .read_to_end(&mut body)
        .ok()?;
    serde_json::from_slice::<serde_json::Value>(&body)
        .ok()?
        .pointer("/error/type")?
        .as_str()
        .map(str::to_owned)
}

fn endpoint_is_secure(endpoint: &str) -> bool {
    let Ok(endpoint) = reqwest::Url::parse(endpoint) else {
        return false;
    };
    let Some(host) = endpoint.host_str() else {
        return false;
    };
    if endpoint.scheme() == "https" {
        return true;
    }
    let ip_host = host
        .strip_prefix('[')
        .and_then(|host| host.strip_suffix(']'))
        .unwrap_or(host);
    endpoint.scheme() == "http"
        && (host.eq_ignore_ascii_case("localhost")
            || ip_host
                .parse::<IpAddr>()
                .is_ok_and(|address| address.is_loopback()))
}

fn is_gateway_auth_error(error_type: Option<&str>) -> bool {
    matches!(
        error_type,
        Some(
            "auth_error"
                | "budget_exceeded"
                | "expired_key"
                | "key_model_access_denied"
                | "no_db_connection"
                | "org_model_access_denied"
                | "project_model_access_denied"
                | "team_model_access_denied"
                | "token_not_found_in_db"
                | "user_model_access_denied"
        )
    )
}

fn map_transport_failure(failure: LiteLlmTransportFailure, byok: bool) -> IntelligenceFailure {
    match failure {
        LiteLlmTransportFailure::Status { status, error_type }
            if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN =>
        {
            if byok && !is_gateway_auth_error(error_type.as_deref()) {
                IntelligenceFailure::ProviderAuthenticationUnavailable
            } else {
                IntelligenceFailure::AuthenticationUnavailable
            }
        }
        LiteLlmTransportFailure::Status {
            status: StatusCode::TOO_MANY_REQUESTS,
            ..
        } => IntelligenceFailure::RateLimited,
        LiteLlmTransportFailure::Status { status, .. } if status.is_client_error() => {
            IntelligenceFailure::ProviderRejectedRequest
        }
        LiteLlmTransportFailure::Status { .. } | LiteLlmTransportFailure::GatewayUnavailable => {
            IntelligenceFailure::GatewayUnavailable
        }
        LiteLlmTransportFailure::TimedOut => IntelligenceFailure::TimedOut,
        LiteLlmTransportFailure::InvalidResponse => IntelligenceFailure::InvalidStructuredResult,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intelligence::{
        AvailableHouseholdTool, DocumentContentExcerpt, HouseholdAdministrationMessage,
        HouseholdAdministrationSource, HouseholdContextItem, IntelligenceCapability,
        IntelligenceExecutionConstraints, IntelligenceResponseSchema,
    };
    use std::sync::Mutex;

    struct RecordingTransport {
        body: Mutex<Option<serde_json::Value>>,
        response: serde_json::Value,
    }

    struct ByokRecordingTransport {
        observation: Mutex<Option<(String, String, String, serde_json::Value)>>,
        response: serde_json::Value,
    }

    impl LiteLlmTransport for ByokRecordingTransport {
        fn post(
            &self,
            endpoint: &str,
            authentication: LiteLlmAuthentication<'_>,
            _request_id: &str,
            _timeout: Duration,
            body: &serde_json::Value,
        ) -> Result<serde_json::Value, LiteLlmTransportFailure> {
            let LiteLlmAuthentication::BringYourOwn { gateway, provider } = authentication else {
                panic!("expected separate BYOK authentication");
            };
            *self.observation.lock().expect("BYOK observation lock") = Some((
                endpoint.to_owned(),
                gateway.to_owned(),
                provider.to_owned(),
                body.clone(),
            ));
            Ok(self.response.clone())
        }
    }

    impl LiteLlmTransport for RecordingTransport {
        fn post(
            &self,
            _endpoint: &str,
            _authentication: LiteLlmAuthentication<'_>,
            _request_id: &str,
            _timeout: Duration,
            body: &serde_json::Value,
        ) -> Result<serde_json::Value, LiteLlmTransportFailure> {
            *self.body.lock().expect("recording transport lock") = Some(body.clone());
            Ok(self.response.clone())
        }
    }

    fn household_request(
        media_type: &str,
        original_base64: &str,
        extracted_text: Option<String>,
    ) -> HouseholdAdministrationRequest {
        HouseholdAdministrationRequest {
            request_id: "household-request-1".to_owned(),
            conversation_id: 42,
            current_message: "What does this say?".to_owned(),
            relevant_conversation: vec![HouseholdAdministrationMessage {
                author: "member".to_owned(),
                body: "What does this say?".to_owned(),
            }],
            source: Some(HouseholdAdministrationSource {
                reference: "document-7".to_owned(),
                filename: if media_type == "application/pdf" {
                    "notice.pdf".to_owned()
                } else {
                    "notice.png".to_owned()
                },
                media_type: media_type.to_owned(),
                original_base64: original_base64.to_owned(),
                extracted_text,
                original_size_bytes: 6,
                extracted_text_truncated: false,
            }),
            household_context: vec![HouseholdContextItem {
                category: "property".to_owned(),
                value: "12 Seabreeze Avenue".to_owned(),
                source_reference: "document-7".to_owned(),
            }],
            active_household_work: Vec::new(),
            source_linked_household_work: None,
            available_tools: vec![AvailableHouseholdTool {
                name: "reminder".to_owned(),
                description: "Propose a reminder".to_owned(),
            }],
            authority_and_approval_constraints: "Luna validates proposals.".to_owned(),
            response_schema_version: "household-administration.v1".to_owned(),
            constraints: IntelligenceExecutionConstraints {
                timeout_ms: 10_000,
                max_output_tokens: 1_200,
            },
        }
    }

    fn conforming_household_content() -> serde_json::Value {
        serde_json::json!({
            "reply": "The bill is due on 15 August.",
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
        })
    }

    fn household_response(content: serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "model": "openai/gpt-5.6-luna",
            "output": [{
                "type": "message",
                "content": [{"type": "output_text", "text": content.to_string()}]
            }],
            "usage": {"input_tokens": 37, "output_tokens": 19, "cost": 0.002}
        })
    }

    #[test]
    fn household_request_uses_the_strict_accepted_output_schema() {
        let request =
            household_request("application/pdf", "QklOQVJZ", Some("Bill text".to_owned()));
        let body = litellm_household_request(&request);
        assert_eq!(body["text"]["format"]["type"], "json_schema");
        assert_eq!(body["text"]["format"]["strict"], true);
        let required = body["text"]["format"]["schema"]["required"]
            .as_array()
            .expect("required fields");
        assert_eq!(
            required,
            &vec![
                serde_json::json!("reply"),
                serde_json::json!("work"),
                serde_json::json!("clarification"),
                serde_json::json!("proposedActions"),
            ]
        );
        assert!(body["text"]["format"]["schema"]["properties"]
            .get("usage")
            .is_none());
        assert!(body["text"]["format"]["schema"]["properties"]
            .get("requestId")
            .is_none());
    }

    #[test]
    fn household_request_uses_the_live_managed_responses_route_contract() {
        let request =
            household_request("application/pdf", "QklOQVJZ", Some("Bill text".to_owned()));
        let body = litellm_household_request(&request);

        assert_eq!(body["model"], "openai/gpt-5.6-luna");
        assert!(body.get("input").is_some());
        assert_eq!(body["text"]["format"]["type"], "json_schema");
        assert!(body.get("messages").is_none());
        assert!(body.get("response_format").is_none());
    }

    #[test]
    fn conforming_household_response_parses_successfully() {
        let request = household_request("application/pdf", "QklOQVJZ", None);
        let result = parse_litellm_household_response(
            &household_response(conforming_household_content()),
            &request,
        )
        .expect("parse conforming household result");
        assert_eq!(result.reply, "The bill is due on 15 August.");
        assert_eq!(result.work.operation, crate::HouseholdWorkOperation::None);
    }

    #[test]
    fn household_response_missing_a_required_field_is_rejected_cleanly() {
        let request = household_request("application/pdf", "QklOQVJZ", None);
        let mut content = conforming_household_content();
        content
            .as_object_mut()
            .expect("household result object")
            .remove("work");
        assert_eq!(
            parse_litellm_household_response(&household_response(content), &request),
            Err(IntelligenceFailure::InvalidStructuredResult)
        );
    }

    #[test]
    fn household_transport_metadata_comes_from_luna_and_the_response_envelope() {
        let request = household_request("application/pdf", "QklOQVJZ", None);
        let result = parse_litellm_household_response(
            &household_response(conforming_household_content()),
            &request,
        )
        .expect("parse household result without model-owned metadata");
        assert_eq!(result.request_id, request.request_id);
        assert_eq!(result.provider_id, MANAGED_INTELLIGENCE_PROVIDER_ID);
        assert_eq!(result.model_id, MANAGED_INTELLIGENCE_MODEL_ID);
        assert_eq!(result.usage.input_tokens, Some(37));
        assert_eq!(result.usage.output_tokens, Some(19));
        assert_eq!(result.usage.estimated_cost_usd, Some(0.002));
    }

    #[test]
    fn text_pdf_input_is_bounded_and_binary_is_a_file_part() {
        let request = household_request(
            "application/pdf",
            "QklOQVJZ",
            Some("x".repeat(MAX_HOUSEHOLD_EXTRACTED_TEXT_CHARS + 100)),
        );
        let body = litellm_household_request(&request);
        let content = body["input"][1]["content"]
            .as_array()
            .expect("multimodal content");
        let text = content[0]["text"].as_str().expect("request text");
        assert!(!text.contains("QklOQVJZ"));
        let request_json: serde_json::Value = serde_json::from_str(text).expect("request JSON");
        assert_eq!(
            request_json["source"]["extractedText"]
                .as_str()
                .expect("bounded extracted text")
                .chars()
                .count(),
            MAX_HOUSEHOLD_EXTRACTED_TEXT_CHARS
        );
        assert_eq!(request_json["source"]["extractedTextTruncated"], true);
        assert_eq!(content[1]["type"], "input_file");
        assert_eq!(
            content[1]["file_data"],
            "data:application/pdf;base64,QklOQVJZ"
        );
    }

    #[test]
    fn image_only_pdf_and_image_use_supported_non_text_content_parts() {
        let pdf_body = litellm_household_request(&household_request(
            "application/pdf",
            "U0NBTk5FRFBERg==",
            None,
        ));
        let pdf_content = pdf_body["input"][1]["content"]
            .as_array()
            .expect("PDF content");
        assert_eq!(pdf_content[1]["type"], "input_file");
        assert_eq!(
            pdf_content[1]["file_data"],
            "data:application/pdf;base64,U0NBTk5FRFBERg=="
        );

        let image_body =
            litellm_household_request(&household_request("image/png", "UE5HREFUQQ==", None));
        let image_content = image_body["input"][1]["content"]
            .as_array()
            .expect("image content");
        assert_eq!(image_content[1]["type"], "input_image");
        assert_eq!(
            image_content[1]["image_url"],
            "data:image/png;base64,UE5HREFUQQ=="
        );
        assert!(!image_content[0]["text"]
            .as_str()
            .expect("request text")
            .contains("UE5HREFUQQ=="));
    }

    #[test]
    fn adapter_passes_one_exact_model_and_disables_litellm_fallbacks() {
        let transport = Arc::new(RecordingTransport {
            body: Mutex::new(None),
            response: serde_json::json!({
                "model": "openai/gpt-5.6-luna",
                "output": [{"type": "message", "content": [{"type": "output_text", "text": "{\"requestId\":\"request-1\",\"documentArrivalId\":\"arrival-1\",\"providerId\":\"openai\",\"modelId\":\"gpt-5.6-luna\",\"fields\":{},\"evidence\":[],\"sourceReferences\":[]}"}]}],
                "usage": {"input_tokens": 10, "output_tokens": 5}
            }),
        });
        let gateway = LiteLlmGateway::with_transport("https://example.invalid", transport.clone());
        let request = IntelligenceRequest {
            request_id: "request-1".to_owned(),
            document_arrival_id: "arrival-1".to_owned(),
            capability: IntelligenceCapability::DirectionInterpretation,
            provider_id: "openai".to_owned(),
            model_id: "gpt-5.6-luna".to_owned(),
            evidence: Vec::new(),
            content_excerpts: vec![DocumentContentExcerpt {
                source: "local excerpt".to_owned(),
                text: "Bill".to_owned(),
            }],
            expected_response: IntelligenceResponseSchema {
                allowed_fields: Vec::new(),
                allow_candidate_direction: true,
            },
            consent_grant_id: Some(1),
            constraints: IntelligenceExecutionConstraints {
                timeout_ms: 10_000,
                max_output_tokens: 128,
            },
        };

        gateway
            .evaluate_document(&request, Some(b"narrow-gateway-token"), None)
            .expect("evaluate through adapter");

        let body = transport
            .body
            .lock()
            .expect("recording transport lock")
            .clone()
            .expect("recorded body");
        assert_eq!(body["model"], "openai/gpt-5.6-luna");
        assert_eq!(body["num_retries"], 0);
        assert_eq!(body["fallbacks"], serde_json::json!([]));
        assert!(body.get("api_key").is_none());
        assert!(body.get("tools").is_none());
    }

    #[test]
    fn byok_adapter_separates_gateway_and_provider_credentials_on_the_isolated_route() {
        let transport = Arc::new(ByokRecordingTransport {
            observation: Mutex::new(None),
            response: serde_json::json!({
                "model": "byok/openai/gpt-5.6-luna",
                "output": [{"type": "message", "content": [{"type": "output_text", "text": "{\"requestId\":\"request-byok\",\"documentArrivalId\":\"synthetic-provider-connection-test\",\"providerId\":\"openai-byok\",\"modelId\":\"gpt-5.6-luna\",\"fields\":{},\"evidence\":[],\"sourceReferences\":[]}"}]}],
                "usage": {"input_tokens": 10, "output_tokens": 5}
            }),
        });
        let gateway =
            LiteLlmGateway::with_transport("https://managed.example.invalid", transport.clone());
        let request = IntelligenceRequest {
            request_id: "request-byok".to_owned(),
            document_arrival_id: "synthetic-provider-connection-test".to_owned(),
            capability: IntelligenceCapability::DirectionInterpretation,
            provider_id: BYOK_OPENAI_PROVIDER_ID.to_owned(),
            model_id: "gpt-5.6-luna".to_owned(),
            evidence: Vec::new(),
            content_excerpts: Vec::new(),
            expected_response: IntelligenceResponseSchema {
                allowed_fields: Vec::new(),
                allow_candidate_direction: false,
            },
            consent_grant_id: None,
            constraints: IntelligenceExecutionConstraints {
                timeout_ms: 10_000,
                max_output_tokens: 128,
            },
        };

        gateway
            .evaluate_document(
                &request,
                Some(b"narrow-byok-gateway-token"),
                Some(b"customer-provider-token"),
            )
            .expect("evaluate through isolated BYOK adapter");

        let (endpoint, gateway_key, provider_key, body) = transport
            .observation
            .lock()
            .expect("BYOK observation lock")
            .clone()
            .expect("BYOK observation");
        assert_eq!(endpoint, "https://byok.example.invalid/v1/responses");
        assert_eq!(gateway_key, "narrow-byok-gateway-token");
        assert_eq!(provider_key, "customer-provider-token");
        assert_eq!(body["model"], "byok/openai/gpt-5.6-luna");
        assert_eq!(body["num_retries"], 0);
        assert_eq!(body["fallbacks"], serde_json::json!([]));
        assert!(body.get("api_key").is_none());
    }

    #[test]
    fn cleartext_gateway_endpoints_are_allowed_only_on_loopback() {
        assert!(endpoint_is_secure("http://127.0.0.1:4001/v1/responses"));
        assert!(endpoint_is_secure("http://[::1]:4001/v1/responses"));
        assert!(endpoint_is_secure("http://localhost:4001/v1/responses"));
        assert!(endpoint_is_secure(
            "https://byok-intelligence.luna.example/v1/responses"
        ));
        assert!(!endpoint_is_secure(
            "http://byok-intelligence.luna.example/v1/responses"
        ));
        assert!(!endpoint_is_secure("not a URL"));
    }

    #[test]
    fn byok_gateway_authentication_rejection_is_not_blamed_on_the_provider_key() {
        assert_eq!(
            map_transport_failure(
                LiteLlmTransportFailure::Status {
                    status: StatusCode::UNAUTHORIZED,
                    error_type: Some("token_not_found_in_db".to_owned()),
                },
                true,
            ),
            IntelligenceFailure::AuthenticationUnavailable
        );
        assert_eq!(
            map_transport_failure(
                LiteLlmTransportFailure::Status {
                    status: StatusCode::UNAUTHORIZED,
                    error_type: Some("invalid_request_error".to_owned()),
                },
                true,
            ),
            IntelligenceFailure::ProviderAuthenticationUnavailable
        );
    }
}
