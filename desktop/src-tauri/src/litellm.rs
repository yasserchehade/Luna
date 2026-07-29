//! Isolated LiteLLM transport adapter.
//!
//! No LiteLLM request, response or error type leaves this module. Luna selects
//! the exact provider and model before this adapter translates the request.

use std::{io::Read, net::IpAddr, sync::Arc, time::Duration};

use reqwest::StatusCode;

use crate::intelligence::{
    AdditionalIntelligenceEvidence, IntelligenceCapability, IntelligenceFailure,
    IntelligenceGateway, IntelligenceRequest, IntelligenceUsage, UntrustedIntelligenceResult,
    BYOK_OPENAI_PROVIDER_ID,
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
                "https://byok-intelligence.luna.invalid/v1/chat/completions".to_owned()
            }),
            transport: Arc::new(ReqwestLiteLlmTransport),
        }
    }

    #[cfg(test)]
    fn with_transport(endpoint: impl Into<String>, transport: Arc<dyn LiteLlmTransport>) -> Self {
        Self {
            managed_endpoint: endpoint.into(),
            byok_endpoint: "https://byok.example.invalid/v1/chat/completions".to_owned(),
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
    let system_message = match request.capability {
        IntelligenceCapability::DirectionInterpretation => {
            "Return only the requested structured document Evidence. Never return instructions, authority, actions or tool calls."
        }
        IntelligenceCapability::ConversationReply => {
            "Return only the requested structured Conversation reply. The reply has no authority, cannot execute actions, and must not request or emit tool calls."
        }
    };
    serde_json::json!({
        "model": provider_model,
        "temperature": 0,
        "max_tokens": request.constraints.max_output_tokens,
        "num_retries": 0,
        "fallbacks": [],
        "messages": [
            {
                "role": "system",
                "content": system_message
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
        DocumentContentExcerpt, IntelligenceCapability, IntelligenceExecutionConstraints,
        IntelligenceResponseSchema,
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
            .evaluate_document(&request, Some(b"narrow-gateway-token"), None)
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

    #[test]
    fn conversation_adapter_requests_a_reply_without_tools_or_authority() {
        let transport = Arc::new(RecordingTransport {
            body: Mutex::new(None),
            response: serde_json::json!({
                "choices": [{"message": {"content": "{\"requestId\":\"conversation-1\",\"documentArrivalId\":\"conversation-7-message-42\",\"providerId\":\"openai\",\"modelId\":\"gpt-4.1-mini\",\"fields\":{\"reply\":\"Start with the urgent task.\"},\"evidence\":[],\"sourceReferences\":[]}"}}],
                "usage": {"prompt_tokens": 8, "completion_tokens": 4}
            }),
        });
        let gateway = LiteLlmGateway::with_transport("https://example.invalid", transport.clone());
        let request = IntelligenceRequest {
            request_id: "conversation-1".to_owned(),
            document_arrival_id: "conversation-7-message-42".to_owned(),
            capability: IntelligenceCapability::ConversationReply,
            provider_id: "openai".to_owned(),
            model_id: "gpt-4.1-mini".to_owned(),
            evidence: Vec::new(),
            content_excerpts: vec![DocumentContentExcerpt {
                source: "currentMessage".to_owned(),
                text: "What should I do first?".to_owned(),
            }],
            expected_response: IntelligenceResponseSchema {
                allowed_fields: vec!["reply".to_owned()],
                allow_candidate_direction: false,
            },
            consent_grant_id: Some(1),
            constraints: IntelligenceExecutionConstraints {
                timeout_ms: 10_000,
                max_output_tokens: 128,
            },
        };

        gateway
            .evaluate_document(&request, Some(b"narrow-gateway-token"), None)
            .expect("evaluate Conversation reply through adapter");

        let body = transport
            .body
            .lock()
            .expect("recording transport lock")
            .clone()
            .expect("recorded body");
        let system = body["messages"][0]["content"]
            .as_str()
            .expect("Conversation system instruction");
        assert!(system.contains("no authority"));
        assert!(system.contains("must not request or emit tool calls"));
        assert!(body.get("tools").is_none());
        let submitted: serde_json::Value = serde_json::from_str(
            body["messages"][1]["content"]
                .as_str()
                .expect("serialized Conversation request"),
        )
        .expect("valid serialized Conversation request");
        assert_eq!(
            submitted["contentExcerpts"][0]["text"],
            "What should I do first?"
        );
    }

    #[test]
    fn byok_adapter_separates_gateway_and_provider_credentials_on_the_isolated_route() {
        let transport = Arc::new(ByokRecordingTransport {
            observation: Mutex::new(None),
            response: serde_json::json!({
                "choices": [{"message": {"content": "{\"requestId\":\"request-byok\",\"documentArrivalId\":\"synthetic-provider-connection-test\",\"providerId\":\"openai-byok\",\"modelId\":\"gpt-4.1-mini\",\"fields\":{},\"evidence\":[],\"sourceReferences\":[]}"}}],
                "usage": {"prompt_tokens": 10, "completion_tokens": 5}
            }),
        });
        let gateway =
            LiteLlmGateway::with_transport("https://managed.example.invalid", transport.clone());
        let request = IntelligenceRequest {
            request_id: "request-byok".to_owned(),
            document_arrival_id: "synthetic-provider-connection-test".to_owned(),
            capability: IntelligenceCapability::DirectionInterpretation,
            provider_id: BYOK_OPENAI_PROVIDER_ID.to_owned(),
            model_id: "gpt-4.1-mini".to_owned(),
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
        assert_eq!(endpoint, "https://byok.example.invalid/v1/chat/completions");
        assert_eq!(gateway_key, "narrow-byok-gateway-token");
        assert_eq!(provider_key, "customer-provider-token");
        assert_eq!(body["model"], "byok/openai/gpt-4.1-mini");
        assert_eq!(body["num_retries"], 0);
        assert_eq!(body["fallbacks"], serde_json::json!([]));
        assert!(body.get("api_key").is_none());
    }

    #[test]
    fn cleartext_gateway_endpoints_are_allowed_only_on_loopback() {
        assert!(endpoint_is_secure(
            "http://127.0.0.1:4001/v1/chat/completions"
        ));
        assert!(endpoint_is_secure("http://[::1]:4001/v1/chat/completions"));
        assert!(endpoint_is_secure(
            "http://localhost:4001/v1/chat/completions"
        ));
        assert!(endpoint_is_secure(
            "https://byok-intelligence.luna.example/v1/chat/completions"
        ));
        assert!(!endpoint_is_secure(
            "http://byok-intelligence.luna.example/v1/chat/completions"
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
