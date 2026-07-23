//! Provider-neutral cloud assistance with explicit, inspectable consent.
//!
//! This module deliberately stops at a structured provider boundary. Luna owns the
//! request, consent and audit record; a provider only receives the structured
//! request and returns evidence-bearing fields. Provider credentials never enter
//! the database or the audit trail.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use rusqlite::{params, Connection, OptionalExtension};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use thiserror::Error;

use crate::trusted_device::{
    CredentialVault, ProtectedHouseholdState, TrustedDeviceError, TrustedDeviceManager, VaultError,
};

const MANAGED_PROVIDER_ID: &str = "luna-managed";
const MANAGED_PROVIDER_NAME: &str = "Luna-managed provider";
const OPENAI_PROVIDER_ID: &str = "openai";
const ANTHROPIC_PROVIDER_ID: &str = "anthropic";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntelligenceProviderDescriptor {
    pub id: String,
    pub name: String,
    pub description: String,
    pub auth_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntelligenceProviderStatus {
    pub descriptor: IntelligenceProviderDescriptor,
    pub configured: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CloudConsentDecision {
    AllowOnce,
    AllowForScope,
    KeepLocal,
    UseExistingScope,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudConsentScope {
    pub id: i64,
    pub household_id: String,
    pub provider_id: String,
    pub purpose: String,
    pub fields: Vec<String>,
    pub created_at: String,
    pub revoked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntelligenceRequest {
    pub purpose: String,
    pub document_name: String,
    pub media_type: String,
    pub evidence: Vec<IntelligenceEvidence>,
    pub unresolved_fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntelligenceEvidence {
    pub field: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntelligenceResult {
    pub provider_id: String,
    pub fields: BTreeMap<String, String>,
    pub evidence: Vec<IntelligenceEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CloudAssistanceOutcome {
    Completed,
    Denied,
    WaitingForConnectivity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudAssistanceAuditEvent {
    pub id: i64,
    pub household_id: String,
    pub provider_id: String,
    pub purpose: String,
    pub consent: CloudConsentDecision,
    pub outcome: CloudAssistanceOutcome,
    pub reason: String,
}

pub trait IntelligenceProvider: Send + Sync {
    fn descriptor(&self) -> IntelligenceProviderDescriptor;
    fn evaluate(
        &self,
        request: &IntelligenceRequest,
        credential: Option<&[u8]>,
    ) -> Result<IntelligenceResult, ProviderError>;
}

trait ProviderTransport: Send + Sync {
    fn post_json(
        &self,
        endpoint: &str,
        headers: &[(&str, String)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, ProviderError>;
}

struct ReqwestProviderTransport;

impl ProviderTransport for ReqwestProviderTransport {
    fn post_json(
        &self,
        endpoint: &str,
        headers: &[(&str, String)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, ProviderError> {
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|_| ProviderError::Unavailable)?;
        let mut request = client.post(endpoint);
        for (name, value) in headers {
            request = request.header(*name, value);
        }
        let response = request
            .json(body)
            .send()
            .map_err(|_| ProviderError::Unavailable)?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            return Err(ProviderError::RequestRejected(provider_error_message(
                status.as_u16(),
                &body,
            )));
        }
        response
            .json::<serde_json::Value>()
            .map_err(|_| ProviderError::InvalidResult)
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ProviderError {
    #[error("the provider is not configured")]
    NotConfigured,
    #[error("the provider is unavailable")]
    Unavailable,
    #[error("the provider returned an invalid result")]
    InvalidResult,
    #[error("the provider credential is invalid")]
    InvalidCredential,
    #[error("the provider rejected the request: {0}")]
    RequestRejected(String),
}

/// Extracts a useful, bounded diagnostic from a provider's error response.
///
/// Provider responses can contain request details or key-like values. Only the
/// documented error message/type/code are surfaced, and known API-key prefixes
/// are redacted before the text reaches the UI or audit event.
fn provider_error_message(status: u16, body: &str) -> String {
    let parsed = serde_json::from_str::<serde_json::Value>(body).ok();
    let error = parsed.as_ref().and_then(|value| value.get("error"));
    let message = error
        .and_then(|value| value.get("message"))
        .and_then(serde_json::Value::as_str)
        .or_else(|| {
            error
                .and_then(|value| value.get("type"))
                .and_then(serde_json::Value::as_str)
        })
        .or_else(|| {
            error
                .and_then(|value| value.get("code"))
                .and_then(serde_json::Value::as_str)
        })
        .unwrap_or("");
    let message = redact_secret_like_text(message);
    if message.is_empty() {
        format!("HTTP {status}")
    } else {
        format!("HTTP {status}: {message}")
    }
}

fn redact_secret_like_text(value: &str) -> String {
    let mut redacted = value.to_owned();
    for prefix in ["sk-proj-", "sk-ant-", "sk-"] {
        loop {
            let Some(start) = redacted.find(prefix) else {
                break;
            };
            let end = redacted[start..]
                .find(|character: char| {
                    character.is_whitespace()
                        || matches!(character, '"' | '\'' | ')' | ']' | ',' | '.' | ';')
                })
                .map(|offset| start + offset)
                .unwrap_or(redacted.len());
            redacted.replace_range(start..end, "[redacted]");
        }
    }
    redacted.chars().take(240).collect()
}

/// The local Luna-managed adapter. It is deterministic and does not require a
/// credential, so the consent and contract seam remains usable offline.
#[derive(Default)]
pub struct LunaManagedProvider;

impl IntelligenceProvider for LunaManagedProvider {
    fn descriptor(&self) -> IntelligenceProviderDescriptor {
        IntelligenceProviderDescriptor {
            id: MANAGED_PROVIDER_ID.to_owned(),
            name: MANAGED_PROVIDER_NAME.to_owned(),
            description: "A Luna-managed structured reasoning provider.".to_owned(),
            auth_url: None,
        }
    }

    fn evaluate(
        &self,
        request: &IntelligenceRequest,
        _credential: Option<&[u8]>,
    ) -> Result<IntelligenceResult, ProviderError> {
        let fields = request
            .evidence
            .iter()
            .filter(|evidence| {
                !evidence.value.trim().is_empty()
                    && request
                        .unresolved_fields
                        .iter()
                        .any(|field| field == &evidence.field)
            })
            .map(|evidence| (evidence.field.clone(), evidence.value.clone()))
            .collect::<BTreeMap<_, _>>();
        Ok(IntelligenceResult {
            provider_id: MANAGED_PROVIDER_ID.to_owned(),
            evidence: request.evidence.clone(),
            fields,
        })
    }
}

pub struct OpenAiProvider {
    endpoint: String,
    model: String,
    transport: Arc<dyn ProviderTransport>,
}

impl Default for OpenAiProvider {
    fn default() -> Self {
        Self::new(
            "https://api.openai.com/v1/chat/completions",
            "gpt-4o-mini",
            Arc::new(ReqwestProviderTransport),
        )
    }
}

impl OpenAiProvider {
    fn new(
        endpoint: impl Into<String>,
        model: impl Into<String>,
        transport: Arc<dyn ProviderTransport>,
    ) -> Self {
        Self {
            endpoint: endpoint.into(),
            model: model.into(),
            transport,
        }
    }
}

impl IntelligenceProvider for OpenAiProvider {
    fn descriptor(&self) -> IntelligenceProviderDescriptor {
        IntelligenceProviderDescriptor {
            id: OPENAI_PROVIDER_ID.to_owned(),
            name: "OpenAI".to_owned(),
            description: "Use your own OpenAI API key for document evaluation.".to_owned(),
            auth_url: Some("https://platform.openai.com/api-keys".to_owned()),
        }
    }

    fn evaluate(
        &self,
        request: &IntelligenceRequest,
        credential: Option<&[u8]>,
    ) -> Result<IntelligenceResult, ProviderError> {
        let credential = credential.ok_or(ProviderError::NotConfigured)?;
        let response = self.transport.post_json(
            &self.endpoint,
            &[
                (
                    "Authorization",
                    format!("Bearer {}", credential_text(credential)?),
                ),
                ("Content-Type", "application/json".to_owned()),
            ],
            &chat_completion_body(&self.model, request),
        )?;
        parse_chat_completion(OPENAI_PROVIDER_ID, request, &response)
    }
}

pub struct AnthropicProvider {
    endpoint: String,
    model: String,
    transport: Arc<dyn ProviderTransport>,
}

impl Default for AnthropicProvider {
    fn default() -> Self {
        Self::new(
            "https://api.anthropic.com/v1/messages",
            "claude-3-5-haiku-latest",
            Arc::new(ReqwestProviderTransport),
        )
    }
}

impl AnthropicProvider {
    fn new(
        endpoint: impl Into<String>,
        model: impl Into<String>,
        transport: Arc<dyn ProviderTransport>,
    ) -> Self {
        Self {
            endpoint: endpoint.into(),
            model: model.into(),
            transport,
        }
    }
}

impl IntelligenceProvider for AnthropicProvider {
    fn descriptor(&self) -> IntelligenceProviderDescriptor {
        IntelligenceProviderDescriptor {
            id: ANTHROPIC_PROVIDER_ID.to_owned(),
            name: "Anthropic".to_owned(),
            description: "Use your own Anthropic API key for document evaluation.".to_owned(),
            auth_url: Some("https://console.anthropic.com/settings/keys".to_owned()),
        }
    }

    fn evaluate(
        &self,
        request: &IntelligenceRequest,
        credential: Option<&[u8]>,
    ) -> Result<IntelligenceResult, ProviderError> {
        let credential = credential.ok_or(ProviderError::NotConfigured)?;
        let response = self.transport.post_json(
            &self.endpoint,
            &[
                ("x-api-key", credential_text(credential)?),
                ("anthropic-version", "2023-06-01".to_owned()),
                ("Content-Type", "application/json".to_owned()),
            ],
            &anthropic_body(&self.model, request),
        )?;
        parse_anthropic_response(ANTHROPIC_PROVIDER_ID, request, &response)
    }
}

fn credential_text(credential: &[u8]) -> Result<String, ProviderError> {
    let value = std::str::from_utf8(credential)
        .map_err(|_| ProviderError::InvalidCredential)?
        .trim();
    (!value.is_empty())
        .then_some(value.to_owned())
        .ok_or(ProviderError::InvalidCredential)
}

fn evaluation_prompt(request: &IntelligenceRequest) -> String {
    format!(
        "Return only a JSON object whose keys are the unresolved Household Context fields. Preserve evidence values when useful and do not invent values. Request: {}",
        serde_json::to_string(request).unwrap_or_else(|_| "{}".to_owned())
    )
}

fn chat_completion_body(model: &str, request: &IntelligenceRequest) -> serde_json::Value {
    serde_json::json!({
        "model": model,
        "temperature": 0,
        "response_format": {"type": "json_object"},
        "messages": [
            {"role": "system", "content": "You are Luna's document evaluation provider. Return structured JSON only."},
            {"role": "user", "content": evaluation_prompt(request)}
        ]
    })
}

fn anthropic_body(model: &str, request: &IntelligenceRequest) -> serde_json::Value {
    serde_json::json!({
        "model": model,
        "max_tokens": 512,
        "temperature": 0,
        "system": "You are Luna's document evaluation provider. Return a JSON object only.",
        "messages": [{"role": "user", "content": evaluation_prompt(request)}]
    })
}

fn parse_chat_completion(
    provider_id: &str,
    request: &IntelligenceRequest,
    response: &serde_json::Value,
) -> Result<IntelligenceResult, ProviderError> {
    let content = response
        .get("choices")
        .and_then(|choices| choices.get(0))
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(serde_json::Value::as_str)
        .ok_or(ProviderError::InvalidResult)?;
    parse_provider_json(provider_id, request, content)
}

fn parse_anthropic_response(
    provider_id: &str,
    request: &IntelligenceRequest,
    response: &serde_json::Value,
) -> Result<IntelligenceResult, ProviderError> {
    let content = response
        .get("content")
        .and_then(|content| content.get(0))
        .and_then(|block| block.get("text"))
        .and_then(serde_json::Value::as_str)
        .ok_or(ProviderError::InvalidResult)?;
    parse_provider_json(provider_id, request, content)
}

fn parse_provider_json(
    provider_id: &str,
    request: &IntelligenceRequest,
    content: &str,
) -> Result<IntelligenceResult, ProviderError> {
    let fields = serde_json::from_str::<BTreeMap<String, String>>(content)
        .map_err(|_| ProviderError::InvalidResult)?
        .into_iter()
        .filter(|(field, value)| {
            request
                .unresolved_fields
                .iter()
                .any(|allowed| allowed == field)
                && !value.trim().is_empty()
        })
        .collect();
    Ok(IntelligenceResult {
        provider_id: provider_id.to_owned(),
        evidence: request.evidence.clone(),
        fields,
    })
}

#[derive(Debug, Error)]
pub enum IntelligenceError {
    #[error("the requested intelligence provider is not available")]
    UnknownProvider,
    #[error("cloud consent is required for this provider and purpose")]
    ConsentRequired,
    #[error("cloud assistance was kept local")]
    KeptLocal,
    #[error("provider {provider_id} failed: {error}")]
    ProviderFailure {
        provider_id: String,
        error: ProviderError,
    },
    #[error("protected Household state is unavailable")]
    TrustedDevice(#[from] TrustedDeviceError),
    #[error("credential vault is unavailable")]
    Vault(#[from] VaultError),
    #[error("intelligence storage is unavailable")]
    Storage(#[from] rusqlite::Error),
    #[error("protected intelligence data is invalid")]
    ProtectedState(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConsentPayload {
    provider_id: String,
    purpose: String,
    fields: Vec<String>,
    created_at: String,
    revoked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AuditPayload {
    provider_id: String,
    purpose: String,
    consent: CloudConsentDecision,
    outcome: CloudAssistanceOutcome,
    reason: String,
}

#[derive(Clone)]
pub struct CloudIntelligenceStore<V: CredentialVault> {
    database: PathBuf,
    trusted_device: TrustedDeviceManager<V>,
    providers: Arc<Vec<Arc<dyn IntelligenceProvider>>>,
}

impl<V: CredentialVault> CloudIntelligenceStore<V> {
    pub fn open(
        database: impl AsRef<Path>,
        trusted_device: TrustedDeviceManager<V>,
    ) -> Result<Self, IntelligenceError> {
        Self::open_with_providers(
            database,
            trusted_device,
            vec![
                Arc::new(LunaManagedProvider),
                Arc::new(OpenAiProvider::default()),
                Arc::new(AnthropicProvider::default()),
            ],
        )
    }

    pub fn open_with_provider<P: IntelligenceProvider + 'static>(
        database: impl AsRef<Path>,
        trusted_device: TrustedDeviceManager<V>,
        provider: P,
    ) -> Result<Self, IntelligenceError> {
        Self::open_with_providers(database, trusted_device, vec![Arc::new(provider)])
    }

    fn open_with_providers(
        database: impl AsRef<Path>,
        trusted_device: TrustedDeviceManager<V>,
        providers: Vec<Arc<dyn IntelligenceProvider>>,
    ) -> Result<Self, IntelligenceError> {
        let store = Self {
            database: database.as_ref().to_owned(),
            trusted_device,
            providers: Arc::new(providers),
        };
        store.connect()?.execute_batch(
            "CREATE TABLE IF NOT EXISTS cloud_consents (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                household_id TEXT NOT NULL,
                protected_payload TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS cloud_consents_household ON cloud_consents(household_id);
            CREATE TABLE IF NOT EXISTS cloud_assistance_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                household_id TEXT NOT NULL,
                protected_payload TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS cloud_assistance_events_household ON cloud_assistance_events(household_id);",
        )?;
        Ok(store)
    }

    pub fn providers(&self) -> Vec<IntelligenceProviderDescriptor> {
        self.providers
            .iter()
            .map(|provider| provider.descriptor())
            .collect()
    }

    pub fn provider_statuses(
        &self,
        household_id: &str,
    ) -> Result<Vec<IntelligenceProviderStatus>, IntelligenceError> {
        self.providers()
            .into_iter()
            .map(|descriptor| {
                let configured = descriptor.id == MANAGED_PROVIDER_ID
                    || self
                        .trusted_device
                        .vault()
                        .get_secret(&credential_key(household_id, &descriptor.id))?
                        .is_some();
                Ok(IntelligenceProviderStatus {
                    descriptor,
                    configured,
                })
            })
            .collect()
    }

    pub fn list_consent_scopes(
        &self,
        household_id: &str,
    ) -> Result<Vec<CloudConsentScope>, IntelligenceError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare("SELECT id, protected_payload FROM cloud_consents WHERE household_id = ?1 ORDER BY id DESC")?;
        let rows = statement
            .query_map(params![household_id], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(|(id, protected)| {
                let payload: ConsentPayload = self.open_protected(household_id, &protected)?;
                Ok(CloudConsentScope {
                    id,
                    household_id: household_id.to_owned(),
                    provider_id: payload.provider_id,
                    purpose: payload.purpose,
                    fields: payload.fields,
                    created_at: payload.created_at,
                    revoked: payload.revoked,
                })
            })
            .collect()
    }

    pub fn grant_scope(
        &self,
        household_id: &str,
        provider_id: &str,
        purpose: &str,
        fields: Vec<String>,
    ) -> Result<CloudConsentScope, IntelligenceError> {
        self.require_provider(provider_id)?;
        let payload = ConsentPayload {
            provider_id: provider_id.to_owned(),
            purpose: purpose.trim().to_owned(),
            fields,
            created_at: now(),
            revoked: false,
        };
        let protected = self.protect(household_id, &payload)?;
        let connection = self.connect()?;
        connection.execute(
            "INSERT INTO cloud_consents (household_id, protected_payload) VALUES (?1, ?2)",
            params![household_id, protected],
        )?;
        Ok(CloudConsentScope {
            id: connection.last_insert_rowid(),
            household_id: household_id.to_owned(),
            provider_id: payload.provider_id,
            purpose: payload.purpose,
            fields: payload.fields,
            created_at: payload.created_at,
            revoked: false,
        })
    }

    pub fn revoke_scope(&self, household_id: &str, scope_id: i64) -> Result<(), IntelligenceError> {
        let protected: String = self
            .connect()?
            .query_row(
                "SELECT protected_payload FROM cloud_consents WHERE id = ?1 AND household_id = ?2",
                params![scope_id, household_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or(IntelligenceError::ConsentRequired)?;
        let mut payload: ConsentPayload = self.open_protected(household_id, &protected)?;
        payload.revoked = true;
        let next = self.protect(household_id, &payload)?;
        self.connect()?.execute(
            "UPDATE cloud_consents SET protected_payload = ?1 WHERE id = ?2 AND household_id = ?3",
            params![next, scope_id, household_id],
        )?;
        Ok(())
    }

    pub fn set_provider_credential(
        &self,
        household_id: &str,
        provider_id: &str,
        credential: &[u8],
    ) -> Result<(), IntelligenceError> {
        self.require_provider(provider_id)?;
        self.trusted_device
            .vault()
            .set_secret(&credential_key(household_id, provider_id), credential)?;
        Ok(())
    }

    pub fn clear_provider_credential(
        &self,
        household_id: &str,
        provider_id: &str,
    ) -> Result<(), IntelligenceError> {
        self.trusted_device
            .vault()
            .delete_secret(&credential_key(household_id, provider_id))?;
        Ok(())
    }

    pub fn evaluate(
        &self,
        household_id: &str,
        request: &IntelligenceRequest,
        provider_id: &str,
        consent: CloudConsentDecision,
    ) -> Result<IntelligenceResult, IntelligenceError> {
        self.require_provider(provider_id)?;
        let allowed = match consent {
            CloudConsentDecision::AllowOnce | CloudConsentDecision::AllowForScope => true,
            CloudConsentDecision::UseExistingScope => self.has_scope(
                household_id,
                provider_id,
                &request.purpose,
                &consent_fields(request),
            )?,
            CloudConsentDecision::KeepLocal => false,
        };
        if consent == CloudConsentDecision::KeepLocal {
            self.record_event(
                household_id,
                provider_id,
                &request.purpose,
                consent,
                CloudAssistanceOutcome::Denied,
                "Member chose to keep this request local.",
            )?;
            return Err(IntelligenceError::KeptLocal);
        }
        if !allowed {
            return Err(IntelligenceError::ConsentRequired);
        }
        if consent == CloudConsentDecision::AllowForScope {
            self.grant_scope(
                household_id,
                provider_id,
                &request.purpose,
                consent_fields(request),
            )?;
        }
        let credential = self
            .trusted_device
            .vault()
            .get_secret(&credential_key(household_id, provider_id))?;
        let provider = self
            .providers
            .iter()
            .find(|provider| provider.descriptor().id == provider_id)
            .ok_or(IntelligenceError::UnknownProvider)?;
        let result = match provider.evaluate(request, credential.as_deref()) {
            Ok(result) => {
                self.record_event(
                    household_id,
                    provider_id,
                    &request.purpose,
                    consent,
                    CloudAssistanceOutcome::Completed,
                    "Provider returned a structured result.",
                )?;
                result
            }
            Err(error @ ProviderError::Unavailable) => {
                self.record_event(
                    household_id,
                    provider_id,
                    &request.purpose,
                    consent,
                    CloudAssistanceOutcome::WaitingForConnectivity,
                    "Provider was unavailable; Luna is waiting and will not switch providers.",
                )?;
                return Err(IntelligenceError::ProviderFailure {
                    provider_id: provider_id.to_owned(),
                    error,
                });
            }
            Err(error) => {
                self.record_event(
                    household_id,
                    provider_id,
                    &request.purpose,
                    consent,
                    CloudAssistanceOutcome::WaitingForConnectivity,
                    "Provider failed; Luna is waiting and will not switch providers.",
                )?;
                return Err(IntelligenceError::ProviderFailure {
                    provider_id: provider_id.to_owned(),
                    error,
                });
            }
        };
        Ok(result)
    }

    pub fn list_audit_events(
        &self,
        household_id: &str,
    ) -> Result<Vec<CloudAssistanceAuditEvent>, IntelligenceError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare("SELECT id, protected_payload FROM cloud_assistance_events WHERE household_id = ?1 ORDER BY id DESC")?;
        let rows = statement
            .query_map(params![household_id], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(|(id, protected)| {
                let payload: AuditPayload = self.open_protected(household_id, &protected)?;
                Ok(CloudAssistanceAuditEvent {
                    id,
                    household_id: household_id.to_owned(),
                    provider_id: payload.provider_id,
                    purpose: payload.purpose,
                    consent: payload.consent,
                    outcome: payload.outcome,
                    reason: payload.reason,
                })
            })
            .collect()
    }

    fn has_scope(
        &self,
        household_id: &str,
        provider_id: &str,
        purpose: &str,
        requested_fields: &[String],
    ) -> Result<bool, IntelligenceError> {
        Ok(self
            .list_consent_scopes(household_id)?
            .into_iter()
            .any(|scope| {
                !scope.revoked
                    && scope.provider_id == provider_id
                    && scope.purpose == purpose
                    && requested_fields
                        .iter()
                        .all(|field| scope.fields.iter().any(|allowed| allowed == field))
            }))
    }

    fn require_provider(&self, provider_id: &str) -> Result<(), IntelligenceError> {
        self.providers
            .iter()
            .any(|provider| provider.descriptor().id == provider_id)
            .then_some(())
            .ok_or(IntelligenceError::UnknownProvider)
    }

    fn record_event(
        &self,
        household_id: &str,
        provider_id: &str,
        purpose: &str,
        consent: CloudConsentDecision,
        outcome: CloudAssistanceOutcome,
        reason: &str,
    ) -> Result<(), IntelligenceError> {
        let payload = AuditPayload {
            provider_id: provider_id.to_owned(),
            purpose: purpose.to_owned(),
            consent,
            outcome,
            reason: reason.to_owned(),
        };
        let protected = self.protect(household_id, &payload)?;
        self.connect()?.execute(
            "INSERT INTO cloud_assistance_events (household_id, protected_payload) VALUES (?1, ?2)",
            params![household_id, protected],
        )?;
        Ok(())
    }

    fn protect<T: Serialize>(
        &self,
        household_id: &str,
        value: &T,
    ) -> Result<String, IntelligenceError> {
        let bytes = serde_json::to_vec(value)?;
        Ok(serde_json::to_string(
            &self
                .trusted_device
                .protect_household_state(household_id, &bytes)?,
        )?)
    }

    fn open_protected<T: DeserializeOwned>(
        &self,
        household_id: &str,
        protected: &str,
    ) -> Result<T, IntelligenceError> {
        let state: ProtectedHouseholdState = serde_json::from_str(protected)?;
        let bytes = self
            .trusted_device
            .open_household_state(household_id, &state)?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    fn connect(&self) -> rusqlite::Result<Connection> {
        Connection::open(&self.database)
    }
}

fn credential_key(household_id: &str, provider_id: &str) -> String {
    format!("luna.cloud.{household_id}.{provider_id}")
}

fn consent_fields(request: &IntelligenceRequest) -> Vec<String> {
    request
        .unresolved_fields
        .iter()
        .chain(request.evidence.iter().map(|evidence| &evidence.field))
        .cloned()
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect()
}
fn now() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trusted_device::{CredentialVault, TrustedDeviceManager, VaultError};
    use std::{
        collections::HashMap,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc, Mutex,
        },
    };

    #[derive(Clone, Default)]
    struct TestVault(Arc<Mutex<HashMap<String, Vec<u8>>>>);
    impl CredentialVault for TestVault {
        fn set_secret(&self, name: &str, secret: &[u8]) -> Result<(), VaultError> {
            self.0
                .lock()
                .unwrap()
                .insert(name.to_owned(), secret.to_vec());
            Ok(())
        }
        fn get_secret(&self, name: &str) -> Result<Option<Vec<u8>>, VaultError> {
            Ok(self.0.lock().unwrap().get(name).cloned())
        }
        fn delete_secret(&self, name: &str) -> Result<(), VaultError> {
            self.0.lock().unwrap().remove(name);
            Ok(())
        }
    }

    struct MockTransport {
        response: serde_json::Value,
    }

    impl ProviderTransport for MockTransport {
        fn post_json(
            &self,
            _endpoint: &str,
            _headers: &[(&str, String)],
            _body: &serde_json::Value,
        ) -> Result<serde_json::Value, ProviderError> {
            Ok(self.response.clone())
        }
    }

    fn store() -> CloudIntelligenceStore<TestVault> {
        static NEXT_DATABASE: AtomicUsize = AtomicUsize::new(0);
        let database = std::env::temp_dir().join(format!(
            "luna-intelligence-test-{}-{}.db",
            std::process::id(),
            NEXT_DATABASE.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_file(&database);
        let vault = TestVault::default();
        let trusted = TrustedDeviceManager::new(vault.clone());
        let enrollment = trusted.enrol_first_device("household").unwrap();
        trusted
            .confirm_recovery_key(
                "household",
                &enrollment.recovery_key,
                &enrollment.recovery_envelope,
            )
            .unwrap();
        trusted.set_current_key_epoch("household", 1).unwrap();
        trusted.configure_device_pin("household", "123456").unwrap();
        trusted.unlock_device("household", "123456").unwrap();
        CloudIntelligenceStore::open(database, trusted).unwrap()
    }

    #[test]
    fn scoped_consent_is_inspectable_and_revocable() {
        let store = store();
        store
            .set_provider_credential("household", MANAGED_PROVIDER_ID, b"opaque")
            .unwrap();
        let scope = store
            .grant_scope(
                "household",
                MANAGED_PROVIDER_ID,
                "document-evaluation",
                vec!["documentType".into()],
            )
            .unwrap();
        assert_eq!(store.list_consent_scopes("household").unwrap()[0], scope);
        store.revoke_scope("household", scope.id).unwrap();
        assert!(store.list_consent_scopes("household").unwrap()[0].revoked);
    }

    #[test]
    fn evaluation_records_provider_and_consent_without_secret() {
        let store = store();
        store
            .set_provider_credential("household", MANAGED_PROVIDER_ID, b"super-secret")
            .unwrap();
        let result = store
            .evaluate(
                "household",
                &IntelligenceRequest {
                    purpose: "document-evaluation".into(),
                    document_name: "bill.pdf".into(),
                    media_type: "application/pdf".into(),
                    evidence: vec![IntelligenceEvidence {
                        field: "documentType".into(),
                        value: "bill".into(),
                    }],
                    unresolved_fields: vec!["serviceProvider".into()],
                },
                MANAGED_PROVIDER_ID,
                CloudConsentDecision::AllowOnce,
            )
            .unwrap();
        assert_eq!(result.provider_id, MANAGED_PROVIDER_ID);
        let events = store.list_audit_events("household").unwrap();
        assert_eq!(events[0].consent, CloudConsentDecision::AllowOnce);
        assert!(!events[0].reason.contains("super-secret"));
    }

    #[test]
    fn no_consent_and_keep_local_never_call_provider() {
        let store = store();
        let request = IntelligenceRequest {
            purpose: "document-evaluation".into(),
            document_name: "bill.pdf".into(),
            media_type: "application/pdf".into(),
            evidence: vec![],
            unresolved_fields: vec!["documentType".into()],
        };
        assert!(matches!(
            store.evaluate(
                "household",
                &request,
                OPENAI_PROVIDER_ID,
                CloudConsentDecision::AllowOnce
            ),
            Err(IntelligenceError::ProviderFailure {
                error: ProviderError::NotConfigured,
                ..
            })
        ));
        assert!(matches!(
            store.evaluate(
                "household",
                &request,
                MANAGED_PROVIDER_ID,
                CloudConsentDecision::KeepLocal
            ),
            Err(IntelligenceError::KeptLocal)
        ));
    }

    #[test]
    fn local_managed_provider_is_available_without_a_credential() {
        let store = store();
        let result = store
            .evaluate(
                "household",
                &IntelligenceRequest {
                    purpose: "document-evaluation".into(),
                    document_name: "bill.pdf".into(),
                    media_type: "application/pdf".into(),
                    evidence: vec![IntelligenceEvidence {
                        field: "documentType".into(),
                        value: "bill".into(),
                    }],
                    unresolved_fields: vec!["documentType".into()],
                },
                MANAGED_PROVIDER_ID,
                CloudConsentDecision::AllowOnce,
            )
            .unwrap();
        assert_eq!(result.fields.get("documentType"), Some(&"bill".to_owned()));
    }

    #[test]
    fn a_scoped_grant_cannot_authorize_fields_outside_its_scope() {
        let store = store();
        store
            .set_provider_credential("household", MANAGED_PROVIDER_ID, b"opaque")
            .unwrap();
        store
            .grant_scope(
                "household",
                MANAGED_PROVIDER_ID,
                "document-evaluation",
                vec!["documentType".into()],
            )
            .unwrap();
        let request = IntelligenceRequest {
            purpose: "document-evaluation".into(),
            document_name: "bill.pdf".into(),
            media_type: "application/pdf".into(),
            evidence: vec![],
            unresolved_fields: vec!["account".into()],
        };
        assert!(matches!(
            store.evaluate(
                "household",
                &request,
                MANAGED_PROVIDER_ID,
                CloudConsentDecision::UseExistingScope,
            ),
            Err(IntelligenceError::ConsentRequired)
        ));
        let evidence_request = IntelligenceRequest {
            evidence: vec![IntelligenceEvidence {
                field: "extractedText".into(),
                value: "private document text".into(),
            }],
            unresolved_fields: vec!["documentType".into()],
            ..request.clone()
        };
        assert!(matches!(
            store.evaluate(
                "household",
                &evidence_request,
                MANAGED_PROVIDER_ID,
                CloudConsentDecision::UseExistingScope,
            ),
            Err(IntelligenceError::ConsentRequired)
        ));
        let allowed_request = IntelligenceRequest {
            unresolved_fields: vec!["documentType".into()],
            purpose: request.purpose.clone(),
            document_name: request.document_name.clone(),
            media_type: request.media_type.clone(),
            evidence: request.evidence.clone(),
        };
        assert!(store
            .evaluate(
                "household",
                &allowed_request,
                MANAGED_PROVIDER_ID,
                CloudConsentDecision::UseExistingScope,
            )
            .is_ok());
        assert!(matches!(
            store.evaluate(
                "household",
                &request,
                MANAGED_PROVIDER_ID,
                CloudConsentDecision::KeepLocal,
            ),
            Err(IntelligenceError::KeptLocal)
        ));
    }

    #[test]
    fn openai_and_anthropic_adapters_return_structured_results_without_network() {
        let request = IntelligenceRequest {
            purpose: "document-evaluation".into(),
            document_name: "bill.pdf".into(),
            media_type: "application/pdf".into(),
            evidence: vec![IntelligenceEvidence {
                field: "documentType".into(),
                value: "bill".into(),
            }],
            unresolved_fields: vec!["serviceProvider".into()],
        };
        let openai = OpenAiProvider::new(
            "https://example.invalid/openai",
            "test-model",
            Arc::new(MockTransport {
                response: serde_json::json!({
                    "choices": [{"message": {"content": "{\"serviceProvider\":\"AGL\",\"account\":\"not-requested\"}"}}]
                }),
            }),
        );
        let anthropic = AnthropicProvider::new(
            "https://example.invalid/anthropic",
            "test-model",
            Arc::new(MockTransport {
                response: serde_json::json!({
                    "content": [{"text": "{\"serviceProvider\":\"AGL\"}"}]
                }),
            }),
        );
        let openai_result = openai.evaluate(&request, Some(b"openai-key")).unwrap();
        assert_eq!(openai_result.provider_id, OPENAI_PROVIDER_ID);
        assert_eq!(
            openai_result.fields.get("serviceProvider"),
            Some(&"AGL".to_owned())
        );
        assert!(!openai_result.fields.contains_key("account"));
        assert_eq!(
            anthropic
                .evaluate(&request, Some(b"anthropic-key"))
                .unwrap()
                .provider_id,
            ANTHROPIC_PROVIDER_ID
        );
    }

    #[test]
    fn provider_error_message_preserves_actionable_diagnostics_without_secrets() {
        let message = provider_error_message(
            429,
            r#"{"error":{"message":"You exceeded your current quota for sk-proj-secret-value.","type":"insufficient_quota","code":"insufficient_quota"}}"#,
        );
        assert_eq!(
            message,
            "HTTP 429: You exceeded your current quota for [redacted]."
        );
        assert!(!message.contains("sk-proj-secret-value"));
    }

    #[test]
    fn provider_error_message_falls_back_to_status_for_unstructured_errors() {
        assert_eq!(
            provider_error_message(502, "upstream unavailable"),
            "HTTP 502"
        );
    }
}
