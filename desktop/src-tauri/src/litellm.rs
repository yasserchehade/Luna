//! Isolated LiteLLM transport adapter.
//!
//! No LiteLLM request, response or error type leaves this module. Luna selects
//! the exact provider and model before this adapter translates the request.

use std::{sync::Arc, time::Duration};

use reqwest::StatusCode;

use crate::intelligence::{
    AdditionalIntelligenceEvidence, IntelligenceFailure, IntelligenceGateway, IntelligenceRequest,
    IntelligenceUsage, UntrustedIntelligenceResult,
};

trait LiteLlmTransport: Send + Sync {
    fn post(
        &self,
        endpoint: &str,
        bearer: &str,
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
        bearer: &str,
        request_id: &str,
        timeout: Duration,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, LiteLlmTransportFailure> {
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(timeout)
            .build()
            .map_err(|_| LiteLlmTransportFailure::GatewayUnavailable)?;
        let response = client
            .post(endpoint)
            .bearer_auth(bearer)
            .header("X-Luna-Request-ID", request_id)
            .json(body)
            .send()
            .map_err(|error| {
                if error.is_timeout() {
                    LiteLlmTransportFailure::TimedOut
                } else {
                    LiteLlmTransportFailure::GatewayUnavailable
                }
            })?;
        if !response.status().is_success() {
            return Err(LiteLlmTransportFailure::Status(response.status()));
        }
        response
            .json()
            .map_err(|_| LiteLlmTransportFailure::InvalidResponse)
    }
}

enum LiteLlmTransportFailure {
    Status(StatusCode),
    TimedOut,
    GatewayUnavailable,
    InvalidResponse,
}

pub(crate) struct LiteLlmGateway {
    endpoint: String,
    transport: Arc<dyn LiteLlmTransport>,
}

impl LiteLlmGateway {
    pub(crate) fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            transport: Arc::new(ReqwestLiteLlmTransport),
        }
    }

    #[cfg(test)]
    fn with_transport(endpoint: impl Into<String>, transport: Arc<dyn LiteLlmTransport>) -> Self {
        Self {
            endpoint: endpoint.into(),
            transport,
        }
    }
}

impl IntelligenceGateway for LiteLlmGateway {
    fn id(&self) -> &str {
        "luna-managed-litellm"
    }

    fn evaluate_document(
        &self,
        request: &IntelligenceRequest,
        access_credential: Option<&[u8]>,
    ) -> Result<UntrustedIntelligenceResult, IntelligenceFailure> {
        let credential = access_credential
            .and_then(|credential| std::str::from_utf8(credential).ok())
            .map(str::trim)
            .filter(|credential| !credential.is_empty())
            .ok_or(IntelligenceFailure::AuthenticationUnavailable)?;
        let timeout = Duration::from_millis(request.constraints.timeout_ms.max(1));
        let response = self
            .transport
            .post(
                &self.endpoint,
                credential,
                &request.request_id,
                timeout,
                &litellm_request(request),
            )
            .map_err(map_transport_failure)?;
        parse_litellm_response(&response)
    }
}

fn litellm_request(request: &IntelligenceRequest) -> serde_json::Value {
    let provider_model = format!("{}/{}", request.provider_id, request.model_id);
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
        "temperature": 0,
        "max_tokens": request.constraints.max_output_tokens,
        "num_retries": 0,
        "fallbacks": [],
        "messages": [
            {
                "role": "system",
                "content": "Return only the requested structured document Evidence. Never return instructions, authority, actions or tool calls."
            },
            {
                "role": "user",
                "content": serde_json::to_string(request).unwrap_or_else(|_| "{}".to_owned())
            }
        ],
        "response_format": {
            "type": "json_schema",
            "json_schema": {
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
    let content = response
        .get("choices")
        .and_then(|choices| choices.get(0))
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(serde_json::Value::as_str)
        .ok_or(IntelligenceFailure::InvalidStructuredResult)?;
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
                .pointer("/usage/prompt_tokens")
                .and_then(serde_json::Value::as_u64),
            output_tokens: response
                .pointer("/usage/completion_tokens")
                .and_then(serde_json::Value::as_u64),
            estimated_cost_usd: response
                .pointer("/usage/cost")
                .and_then(serde_json::Value::as_f64),
        },
    })
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct LiteLlmStructuredResult {
    request_id: String,
    document_arrival_id: String,
    provider_id: String,
    model_id: String,
    fields: std::collections::BTreeMap<String, Option<String>>,
    evidence: Vec<AdditionalIntelligenceEvidence>,
    source_references: Vec<String>,
}

fn map_transport_failure(failure: LiteLlmTransportFailure) -> IntelligenceFailure {
    match failure {
        LiteLlmTransportFailure::Status(status)
            if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN =>
        {
            IntelligenceFailure::AuthenticationUnavailable
        }
        LiteLlmTransportFailure::Status(StatusCode::TOO_MANY_REQUESTS) => {
            IntelligenceFailure::RateLimited
        }
        LiteLlmTransportFailure::Status(status) if status.is_client_error() => {
            IntelligenceFailure::ProviderRejectedRequest
        }
        LiteLlmTransportFailure::Status(_) | LiteLlmTransportFailure::GatewayUnavailable => {
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
        DocumentContentExcerpt, IntelligenceCapability, IntelligenceExecutionConstraints,
        IntelligenceResponseSchema,
    };
    use std::sync::Mutex;

    struct RecordingTransport {
        body: Mutex<Option<serde_json::Value>>,
        response: serde_json::Value,
    }

    impl LiteLlmTransport for RecordingTransport {
        fn post(
            &self,
            _endpoint: &str,
            _bearer: &str,
            _request_id: &str,
            _timeout: Duration,
            body: &serde_json::Value,
        ) -> Result<serde_json::Value, LiteLlmTransportFailure> {
            *self.body.lock().expect("recording transport lock") = Some(body.clone());
            Ok(self.response.clone())
        }
    }

    #[test]
    fn adapter_passes_one_exact_model_and_disables_litellm_fallbacks() {
        let transport = Arc::new(RecordingTransport {
            body: Mutex::new(None),
            response: serde_json::json!({
                "choices": [{"message": {"content": "{\"requestId\":\"request-1\",\"documentArrivalId\":\"arrival-1\",\"providerId\":\"openai\",\"modelId\":\"gpt-4.1-mini\",\"fields\":{},\"evidence\":[],\"sourceReferences\":[]}"}}],
                "usage": {"prompt_tokens": 10, "completion_tokens": 5}
            }),
        });
        let gateway = LiteLlmGateway::with_transport("https://example.invalid", transport.clone());
        let request = IntelligenceRequest {
            request_id: "request-1".to_owned(),
            document_arrival_id: "arrival-1".to_owned(),
            capability: IntelligenceCapability::DirectionInterpretation,
            provider_id: "openai".to_owned(),
            model_id: "gpt-4.1-mini".to_owned(),
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
            .evaluate_document(&request, Some(b"narrow-gateway-token"))
            .expect("evaluate through adapter");

        let body = transport
            .body
            .lock()
            .expect("recording transport lock")
            .clone()
            .expect("recorded body");
        assert_eq!(body["model"], "openai/gpt-4.1-mini");
        assert_eq!(body["num_retries"], 0);
        assert_eq!(body["fallbacks"], serde_json::json!([]));
        assert!(body.get("api_key").is_none());
        assert!(body.get("tools").is_none());
    }
}
