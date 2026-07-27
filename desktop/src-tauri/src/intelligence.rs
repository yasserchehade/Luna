//! Luna-owned intelligence contract, consent policy, validation and History.
//!
//! Intelligence gateways are replaceable infrastructure. They receive only a
//! deliberately minimised request after Luna has selected an exact Intelligence
//! Provider and model and validated an applicable Consent Grant.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{params, Connection, OptionalExtension};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use thiserror::Error;

use crate::{
    litellm::LiteLlmGateway,
    trusted_device::{
        CredentialVault, ProtectedHouseholdState, TrustedDeviceError, TrustedDeviceManager,
        VaultError,
    },
};

pub const MANAGED_INTELLIGENCE_PROVIDER_ID: &str = "openai";
pub const MANAGED_INTELLIGENCE_MODEL_ID: &str = "gpt-4.1-mini";
const MAX_SAFE_RETRY_ATTEMPTS: usize = 2;
const MAX_FIELD_VALUE_CHARS: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntelligenceModelDescriptor {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntelligenceProviderDescriptor {
    pub id: String,
    pub name: String,
    pub description: String,
    pub models: Vec<IntelligenceModelDescriptor>,
    pub managed_by_luna: bool,
    pub auth_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntelligenceProviderStatus {
    pub descriptor: IntelligenceProviderDescriptor,
    pub configured: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntelligenceSelection {
    pub provider_id: String,
    pub model_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum IntelligenceCapability {
    DirectionInterpretation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CloudConsentDecision {
    AllowOnce,
    AllowForScope,
    KeepLocal,
    UseExistingScope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConsentGrantKind {
    OneTime,
    Reusable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudConsentScope {
    pub id: i64,
    pub household_id: String,
    pub provider_id: String,
    pub model_id: String,
    pub capability: IntelligenceCapability,
    pub purpose: String,
    pub document_arrival_id: Option<String>,
    pub future_scope: Option<String>,
    pub fields: Vec<String>,
    pub kind: ConsentGrantKind,
    pub granted_by: String,
    pub created_at: String,
    pub consumed_at: Option<String>,
    pub revoked_at: Option<String>,
    pub revoked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntelligenceEvidence {
    pub field: String,
    pub value: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentContentExcerpt {
    pub source: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntelligenceResponseSchema {
    pub allowed_fields: Vec<String>,
    pub allow_candidate_direction: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntelligenceExecutionConstraints {
    pub timeout_ms: u64,
    pub max_output_tokens: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntelligenceRequest {
    pub request_id: String,
    pub document_arrival_id: String,
    pub capability: IntelligenceCapability,
    pub provider_id: String,
    pub model_id: String,
    pub evidence: Vec<IntelligenceEvidence>,
    pub content_excerpts: Vec<DocumentContentExcerpt>,
    pub expected_response: IntelligenceResponseSchema,
    pub consent_grant_id: Option<i64>,
    pub constraints: IntelligenceExecutionConstraints,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdditionalIntelligenceEvidence {
    pub field: String,
    pub value: String,
    pub source_reference: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CandidateDirectionInterpretation {
    pub document_type: Option<String>,
    pub service_provider: Option<String>,
    pub addressee: Option<String>,
    pub property: Option<String>,
    pub account: Option<String>,
    pub amount: Option<String>,
    pub relevant_dates: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct IntelligenceUsage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub estimated_cost_usd: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UntrustedIntelligenceResult {
    pub request_id: String,
    pub document_arrival_id: String,
    pub provider_id: String,
    pub model_id: String,
    pub fields: BTreeMap<String, String>,
    pub evidence: Vec<AdditionalIntelligenceEvidence>,
    pub source_references: Vec<String>,
    #[serde(default)]
    pub usage: IntelligenceUsage,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntelligenceResult {
    pub request_id: String,
    pub document_arrival_id: String,
    pub provider_id: String,
    pub model_id: String,
    pub consent_grant_id: i64,
    pub fields: BTreeMap<String, String>,
    pub evidence: Vec<AdditionalIntelligenceEvidence>,
    pub source_references: Vec<String>,
    pub candidate_direction: Option<CandidateDirectionInterpretation>,
    pub usage: IntelligenceUsage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CloudAssistanceOutcome {
    Completed,
    Denied,
    WaitingForRetry,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CandidateDisposition {
    Pending,
    Accepted,
    Corrected,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudAssistanceAuditEvent {
    pub id: i64,
    pub household_id: String,
    pub request_id: String,
    pub document_arrival_id: String,
    pub provider_id: String,
    pub model_id: String,
    pub capability: IntelligenceCapability,
    pub purpose: String,
    pub consent: CloudConsentDecision,
    pub consent_grant_id: Option<i64>,
    pub granted_by: String,
    pub outcome: CloudAssistanceOutcome,
    pub candidate_disposition: CandidateDisposition,
    pub reason: String,
    pub usage: IntelligenceUsage,
}

#[derive(Debug, Error, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum IntelligenceFailure {
    #[error("Cloud Assistance requires an applicable Consent Grant")]
    ConsentRequired,
    #[error("the applicable Consent Grant has been revoked")]
    ConsentRevoked,
    #[error("the one-time Consent Grant has already been used")]
    ConsentConsumed,
    #[error("the selected Intelligence Provider is unavailable")]
    ProviderUnavailable,
    #[error("the selected Intelligence Provider rejected the request")]
    ProviderRejectedRequest,
    #[error("Luna's gateway authentication is unavailable")]
    AuthenticationUnavailable,
    #[error("the selected Intelligence Provider rate-limited the request")]
    RateLimited,
    #[error("the Intelligence Request timed out")]
    TimedOut,
    #[error("the Intelligence Provider returned an invalid structured result")]
    InvalidStructuredResult,
    #[error("the requested intelligence capability is unsupported")]
    UnsupportedCapability,
    #[error("the Luna-managed Intelligence Gateway is unavailable")]
    GatewayUnavailable,
    #[error("the Household kept this Document local")]
    LocalOnlyPolicy,
    #[error("the Intelligence Request was cancelled")]
    RequestCancelled,
    #[error("the selected Intelligence Provider or model is not approved")]
    UnsupportedSelection,
    #[error("protected Household intelligence state is unavailable")]
    ProtectedStateUnavailable,
    #[error("intelligence storage is unavailable")]
    StorageUnavailable,
}

pub trait IntelligenceGateway: Send + Sync {
    fn id(&self) -> &str;

    fn requires_access_credential(&self) -> bool {
        true
    }

    fn evaluate_document(
        &self,
        request: &IntelligenceRequest,
        access_credential: Option<&[u8]>,
    ) -> Result<UntrustedIntelligenceResult, IntelligenceFailure>;
}

/// A deterministic implementation of the same Luna-owned contract used by
/// production gateways. It is never registered by the production application.
#[derive(Clone)]
pub struct DeterministicIntelligenceGateway {
    provider_id: String,
    model_id: String,
    fields: Arc<BTreeMap<String, String>>,
    requests: Arc<Mutex<Vec<IntelligenceRequest>>>,
    failures: Arc<Mutex<VecDeque<IntelligenceFailure>>>,
}

impl DeterministicIntelligenceGateway {
    pub fn new(
        provider_id: impl Into<String>,
        model_id: impl Into<String>,
        fields: BTreeMap<String, String>,
    ) -> Self {
        Self {
            provider_id: provider_id.into(),
            model_id: model_id.into(),
            fields: Arc::new(fields),
            requests: Arc::new(Mutex::new(Vec::new())),
            failures: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    pub fn fail_next(&self, failure: IntelligenceFailure) {
        if let Ok(mut failures) = self.failures.lock() {
            failures.push_back(failure);
        }
    }

    pub fn requests(&self) -> Vec<IntelligenceRequest> {
        self.requests
            .lock()
            .map(|requests| requests.clone())
            .unwrap_or_default()
    }
}

impl IntelligenceGateway for DeterministicIntelligenceGateway {
    fn id(&self) -> &str {
        "deterministic-test"
    }

    fn requires_access_credential(&self) -> bool {
        false
    }

    fn evaluate_document(
        &self,
        request: &IntelligenceRequest,
        _access_credential: Option<&[u8]>,
    ) -> Result<UntrustedIntelligenceResult, IntelligenceFailure> {
        self.requests
            .lock()
            .map_err(|_| IntelligenceFailure::GatewayUnavailable)?
            .push(request.clone());
        if let Some(failure) = self
            .failures
            .lock()
            .map_err(|_| IntelligenceFailure::GatewayUnavailable)?
            .pop_front()
        {
            return Err(failure);
        }
        Ok(UntrustedIntelligenceResult {
            request_id: request.request_id.clone(),
            document_arrival_id: request.document_arrival_id.clone(),
            provider_id: self.provider_id.clone(),
            model_id: self.model_id.clone(),
            fields: self.fields.as_ref().clone(),
            evidence: Vec::new(),
            source_references: Vec::new(),
            usage: IntelligenceUsage::default(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConsentPayload {
    provider_id: String,
    model_id: String,
    capability: IntelligenceCapability,
    purpose: String,
    document_arrival_id: Option<String>,
    future_scope: Option<String>,
    fields: Vec<String>,
    kind: ConsentGrantKind,
    granted_by: String,
    created_at: String,
    consumed_at: Option<String>,
    revoked_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AuditPayload {
    request_id: String,
    document_arrival_id: String,
    provider_id: String,
    model_id: String,
    capability: IntelligenceCapability,
    purpose: String,
    consent: CloudConsentDecision,
    consent_grant_id: Option<i64>,
    granted_by: String,
    outcome: CloudAssistanceOutcome,
    candidate_disposition: CandidateDisposition,
    reason: String,
    usage: IntelligenceUsage,
}

#[derive(Clone)]
pub struct CloudIntelligenceStore<V: CredentialVault> {
    database: PathBuf,
    trusted_device: TrustedDeviceManager<V>,
    gateway: Arc<dyn IntelligenceGateway>,
    providers: Arc<Vec<IntelligenceProviderDescriptor>>,
}

impl<V: CredentialVault> CloudIntelligenceStore<V> {
    pub fn open(
        database: impl AsRef<Path>,
        trusted_device: TrustedDeviceManager<V>,
    ) -> Result<Self, IntelligenceFailure> {
        let endpoint = std::env::var("LUNA_MANAGED_INTELLIGENCE_URL")
            .unwrap_or_else(|_| "https://intelligence.luna.invalid/v1/chat/completions".to_owned());
        Self::open_with_gateway(
            database,
            trusted_device,
            LiteLlmGateway::new(endpoint),
            managed_provider_catalog(),
        )
    }

    pub fn open_with_gateway<G: IntelligenceGateway + 'static>(
        database: impl AsRef<Path>,
        trusted_device: TrustedDeviceManager<V>,
        gateway: G,
        providers: Vec<IntelligenceProviderDescriptor>,
    ) -> Result<Self, IntelligenceFailure> {
        let store = Self {
            database: database.as_ref().to_owned(),
            trusted_device,
            gateway: Arc::new(gateway),
            providers: Arc::new(providers),
        };
        store
            .connect()?
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS cloud_consents (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    household_id TEXT NOT NULL,
                    protected_payload TEXT NOT NULL
                );
                CREATE INDEX IF NOT EXISTS cloud_consents_household
                    ON cloud_consents(household_id);
                CREATE TABLE IF NOT EXISTS cloud_assistance_events (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    household_id TEXT NOT NULL,
                    protected_payload TEXT NOT NULL
                );
                CREATE INDEX IF NOT EXISTS cloud_assistance_events_household
                    ON cloud_assistance_events(household_id);",
            )
            .map_err(|_| IntelligenceFailure::StorageUnavailable)?;
        Ok(store)
    }

    pub fn providers(&self) -> Vec<IntelligenceProviderDescriptor> {
        self.providers.as_ref().clone()
    }

    pub fn provider_statuses(
        &self,
        household_id: &str,
    ) -> Result<Vec<IntelligenceProviderStatus>, IntelligenceFailure> {
        let configured = !self.gateway.requires_access_credential()
            || self
                .trusted_device
                .vault()
                .get_secret(&gateway_credential_key(household_id, self.gateway.id()))
                .map_err(|_| IntelligenceFailure::AuthenticationUnavailable)?
                .is_some();
        Ok(self
            .providers()
            .into_iter()
            .map(|descriptor| IntelligenceProviderStatus {
                descriptor,
                configured,
            })
            .collect())
    }

    pub fn set_gateway_access_credential(
        &self,
        household_id: &str,
        credential: &[u8],
    ) -> Result<(), IntelligenceFailure> {
        if credential.is_empty() {
            return Err(IntelligenceFailure::AuthenticationUnavailable);
        }
        self.trusted_device
            .vault()
            .set_secret(
                &gateway_credential_key(household_id, self.gateway.id()),
                credential,
            )
            .map_err(|_| IntelligenceFailure::AuthenticationUnavailable)
    }

    pub fn clear_gateway_access_credential(
        &self,
        household_id: &str,
    ) -> Result<(), IntelligenceFailure> {
        self.trusted_device
            .vault()
            .delete_secret(&gateway_credential_key(household_id, self.gateway.id()))
            .map_err(|_| IntelligenceFailure::AuthenticationUnavailable)
    }

    pub fn list_consent_scopes(
        &self,
        household_id: &str,
    ) -> Result<Vec<CloudConsentScope>, IntelligenceFailure> {
        let connection = self.connect()?;
        let mut statement = connection
            .prepare(
                "SELECT id, protected_payload FROM cloud_consents
                 WHERE household_id = ?1 ORDER BY id DESC",
            )
            .map_err(|_| IntelligenceFailure::StorageUnavailable)?;
        let rows = statement
            .query_map(params![household_id], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|_| IntelligenceFailure::StorageUnavailable)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| IntelligenceFailure::StorageUnavailable)?;
        rows.into_iter()
            .map(|(id, protected)| {
                let payload: ConsentPayload = self.open_protected(household_id, &protected)?;
                Ok(consent_scope(household_id, id, payload))
            })
            .collect()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn grant_scope(
        &self,
        household_id: &str,
        selection: &IntelligenceSelection,
        capability: IntelligenceCapability,
        purpose: &str,
        fields: Vec<String>,
        granted_by: &str,
        future_scope: &str,
    ) -> Result<CloudConsentScope, IntelligenceFailure> {
        self.require_selection(selection)?;
        self.insert_consent(
            household_id,
            ConsentPayload {
                provider_id: selection.provider_id.clone(),
                model_id: selection.model_id.clone(),
                capability,
                purpose: purpose.trim().to_owned(),
                document_arrival_id: None,
                future_scope: Some(future_scope.trim().to_owned()),
                fields: normalized_fields(fields),
                kind: ConsentGrantKind::Reusable,
                granted_by: granted_by.trim().to_owned(),
                created_at: now(),
                consumed_at: None,
                revoked_at: None,
            },
        )
    }

    pub fn revoke_scope(
        &self,
        household_id: &str,
        scope_id: i64,
    ) -> Result<(), IntelligenceFailure> {
        let mut payload = self.load_consent(household_id, scope_id)?;
        payload.revoked_at = Some(now());
        self.save_consent(household_id, scope_id, &payload)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn evaluate_document(
        &self,
        household_id: &str,
        selection: IntelligenceSelection,
        mut request: IntelligenceRequest,
        consent: CloudConsentDecision,
        granted_by: &str,
        existing_consent_grant_id: Option<i64>,
    ) -> Result<IntelligenceResult, IntelligenceFailure> {
        self.require_selection(&selection)?;
        if request.provider_id != selection.provider_id || request.model_id != selection.model_id {
            return Err(IntelligenceFailure::UnsupportedSelection);
        }
        if request.capability != IntelligenceCapability::DirectionInterpretation {
            return Err(IntelligenceFailure::UnsupportedCapability);
        }
        if consent == CloudConsentDecision::KeepLocal {
            self.record_event(
                household_id,
                &request,
                consent,
                None,
                granted_by,
                CloudAssistanceOutcome::Denied,
                "The Household chose Keep local; no document information left the Trusted Device.",
                IntelligenceUsage::default(),
            )?;
            return Err(IntelligenceFailure::LocalOnlyPolicy);
        }

        let grant = match consent {
            CloudConsentDecision::AllowOnce => self.insert_consent(
                household_id,
                ConsentPayload {
                    provider_id: selection.provider_id.clone(),
                    model_id: selection.model_id.clone(),
                    capability: request.capability,
                    purpose: capability_purpose(request.capability).to_owned(),
                    document_arrival_id: Some(request.document_arrival_id.clone()),
                    future_scope: None,
                    fields: disclosed_fields(&request),
                    kind: ConsentGrantKind::OneTime,
                    granted_by: granted_by.trim().to_owned(),
                    created_at: now(),
                    consumed_at: None,
                    revoked_at: None,
                },
            )?,
            CloudConsentDecision::AllowForScope => self.grant_scope(
                household_id,
                &selection,
                request.capability,
                capability_purpose(request.capability),
                disclosed_fields(&request),
                granted_by,
                "Future difficult Documents requiring Direction Interpretation with the same disclosed fields.",
            )?,
            CloudConsentDecision::UseExistingScope => {
                let id =
                    existing_consent_grant_id.ok_or(IntelligenceFailure::ConsentRequired)?;
                let payload = self.load_consent(household_id, id)?;
                self.validate_consent(&payload, &selection, &request)?;
                consent_scope(household_id, id, payload)
            }
            CloudConsentDecision::KeepLocal => unreachable!(),
        };
        self.consume_if_one_time(household_id, &grant)?;
        request.consent_grant_id = Some(grant.id);

        let credential = self
            .trusted_device
            .vault()
            .get_secret(&gateway_credential_key(household_id, self.gateway.id()))
            .map_err(|_| IntelligenceFailure::AuthenticationUnavailable)?;
        if self.gateway.requires_access_credential() && credential.is_none() {
            self.record_waiting_failure(
                household_id,
                &request,
                consent,
                &grant,
                IntelligenceFailure::AuthenticationUnavailable,
            )?;
            return Err(IntelligenceFailure::AuthenticationUnavailable);
        }

        let mut last_failure = IntelligenceFailure::GatewayUnavailable;
        for attempt in 0..MAX_SAFE_RETRY_ATTEMPTS {
            match self
                .gateway
                .evaluate_document(&request, credential.as_deref())
            {
                Ok(untrusted) => {
                    let result = match validate_result(&request, grant.id, untrusted) {
                        Ok(result) => result,
                        Err(failure) => {
                            self.record_waiting_failure(
                                household_id,
                                &request,
                                consent,
                                &grant,
                                failure.clone(),
                            )?;
                            return Err(failure);
                        }
                    };
                    self.record_event(
                        household_id,
                        &request,
                        consent,
                        Some(&grant),
                        granted_by,
                        CloudAssistanceOutcome::Completed,
                        "Cloud Assistance returned validated Evidence and a candidate Direction Interpretation.",
                        result.usage.clone(),
                    )?;
                    return Ok(result);
                }
                Err(failure) if failure.is_retryable() && attempt + 1 < MAX_SAFE_RETRY_ATTEMPTS => {
                    last_failure = failure;
                }
                Err(failure) => {
                    last_failure = failure;
                    break;
                }
            }
        }
        self.record_waiting_failure(
            household_id,
            &request,
            consent,
            &grant,
            last_failure.clone(),
        )?;
        Err(last_failure)
    }

    pub fn record_candidate_disposition(
        &self,
        household_id: &str,
        request_id: &str,
        disposition: CandidateDisposition,
    ) -> Result<(), IntelligenceFailure> {
        let connection = self.connect()?;
        let mut statement = connection
            .prepare(
                "SELECT id, protected_payload FROM cloud_assistance_events
                 WHERE household_id = ?1 ORDER BY id DESC",
            )
            .map_err(|_| IntelligenceFailure::StorageUnavailable)?;
        let rows = statement
            .query_map(params![household_id], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|_| IntelligenceFailure::StorageUnavailable)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| IntelligenceFailure::StorageUnavailable)?;
        for (id, protected) in rows {
            let mut payload: AuditPayload = self.open_protected(household_id, &protected)?;
            if payload.request_id == request_id {
                payload.candidate_disposition = disposition;
                let next = self.protect(household_id, &payload)?;
                connection
                    .execute(
                        "UPDATE cloud_assistance_events SET protected_payload = ?1
                         WHERE id = ?2 AND household_id = ?3",
                        params![next, id, household_id],
                    )
                    .map_err(|_| IntelligenceFailure::StorageUnavailable)?;
                return Ok(());
            }
        }
        Err(IntelligenceFailure::StorageUnavailable)
    }

    pub fn record_candidate_validation_failure(
        &self,
        household_id: &str,
        request_id: &str,
    ) -> Result<(), IntelligenceFailure> {
        let connection = self.connect()?;
        let mut statement = connection
            .prepare(
                "SELECT id, protected_payload FROM cloud_assistance_events
                 WHERE household_id = ?1 ORDER BY id DESC",
            )
            .map_err(|_| IntelligenceFailure::StorageUnavailable)?;
        let rows = statement
            .query_map(params![household_id], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|_| IntelligenceFailure::StorageUnavailable)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| IntelligenceFailure::StorageUnavailable)?;
        for (id, protected) in rows {
            let mut payload: AuditPayload = self.open_protected(household_id, &protected)?;
            if payload.request_id == request_id {
                payload.outcome = CloudAssistanceOutcome::WaitingForRetry;
                payload.candidate_disposition = CandidateDisposition::Rejected;
                payload.reason = "Luna rejected the candidate Direction Interpretation during owning-domain validation; no Document Handling decision was made.".to_owned();
                let next = self.protect(household_id, &payload)?;
                connection
                    .execute(
                        "UPDATE cloud_assistance_events SET protected_payload = ?1
                         WHERE id = ?2 AND household_id = ?3",
                        params![next, id, household_id],
                    )
                    .map_err(|_| IntelligenceFailure::StorageUnavailable)?;
                return Ok(());
            }
        }
        Err(IntelligenceFailure::StorageUnavailable)
    }

    pub fn list_audit_events(
        &self,
        household_id: &str,
    ) -> Result<Vec<CloudAssistanceAuditEvent>, IntelligenceFailure> {
        let connection = self.connect()?;
        let mut statement = connection
            .prepare(
                "SELECT id, protected_payload FROM cloud_assistance_events
                 WHERE household_id = ?1 ORDER BY id DESC",
            )
            .map_err(|_| IntelligenceFailure::StorageUnavailable)?;
        let rows = statement
            .query_map(params![household_id], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|_| IntelligenceFailure::StorageUnavailable)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| IntelligenceFailure::StorageUnavailable)?;
        rows.into_iter()
            .map(|(id, protected)| {
                let payload: AuditPayload = self.open_protected(household_id, &protected)?;
                Ok(CloudAssistanceAuditEvent {
                    id,
                    household_id: household_id.to_owned(),
                    request_id: payload.request_id,
                    document_arrival_id: payload.document_arrival_id,
                    provider_id: payload.provider_id,
                    model_id: payload.model_id,
                    capability: payload.capability,
                    purpose: payload.purpose,
                    consent: payload.consent,
                    consent_grant_id: payload.consent_grant_id,
                    granted_by: payload.granted_by,
                    outcome: payload.outcome,
                    candidate_disposition: payload.candidate_disposition,
                    reason: payload.reason,
                    usage: payload.usage,
                })
            })
            .collect()
    }

    fn require_selection(
        &self,
        selection: &IntelligenceSelection,
    ) -> Result<(), IntelligenceFailure> {
        self.providers
            .iter()
            .any(|provider| {
                provider.id == selection.provider_id
                    && provider
                        .models
                        .iter()
                        .any(|model| model.id == selection.model_id)
            })
            .then_some(())
            .ok_or(IntelligenceFailure::UnsupportedSelection)
    }

    fn validate_consent(
        &self,
        payload: &ConsentPayload,
        selection: &IntelligenceSelection,
        request: &IntelligenceRequest,
    ) -> Result<(), IntelligenceFailure> {
        if payload.revoked_at.is_some() {
            return Err(IntelligenceFailure::ConsentRevoked);
        }
        if payload.kind == ConsentGrantKind::OneTime && payload.consumed_at.is_some() {
            return Err(IntelligenceFailure::ConsentConsumed);
        }
        let same_selection = payload.provider_id == selection.provider_id
            && payload.model_id == selection.model_id
            && payload.capability == request.capability;
        let same_document = payload
            .document_arrival_id
            .as_ref()
            .is_none_or(|arrival| arrival == &request.document_arrival_id);
        let requested = disclosed_fields(request);
        let fields_allowed = requested
            .iter()
            .all(|field| payload.fields.iter().any(|allowed| allowed == field));
        if same_selection && same_document && fields_allowed {
            Ok(())
        } else {
            Err(IntelligenceFailure::ConsentRequired)
        }
    }

    fn consume_if_one_time(
        &self,
        household_id: &str,
        scope: &CloudConsentScope,
    ) -> Result<(), IntelligenceFailure> {
        if scope.kind == ConsentGrantKind::OneTime {
            let mut payload = self.load_consent(household_id, scope.id)?;
            if payload.consumed_at.is_some() {
                return Err(IntelligenceFailure::ConsentConsumed);
            }
            payload.consumed_at = Some(now());
            self.save_consent(household_id, scope.id, &payload)?;
        }
        Ok(())
    }

    fn insert_consent(
        &self,
        household_id: &str,
        payload: ConsentPayload,
    ) -> Result<CloudConsentScope, IntelligenceFailure> {
        let protected = self.protect(household_id, &payload)?;
        let connection = self.connect()?;
        connection
            .execute(
                "INSERT INTO cloud_consents (household_id, protected_payload) VALUES (?1, ?2)",
                params![household_id, protected],
            )
            .map_err(|_| IntelligenceFailure::StorageUnavailable)?;
        Ok(consent_scope(
            household_id,
            connection.last_insert_rowid(),
            payload,
        ))
    }

    fn load_consent(
        &self,
        household_id: &str,
        id: i64,
    ) -> Result<ConsentPayload, IntelligenceFailure> {
        let protected = self
            .connect()?
            .query_row(
                "SELECT protected_payload FROM cloud_consents
                 WHERE id = ?1 AND household_id = ?2",
                params![id, household_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|_| IntelligenceFailure::StorageUnavailable)?
            .ok_or(IntelligenceFailure::ConsentRequired)?;
        self.open_protected(household_id, &protected)
    }

    fn save_consent(
        &self,
        household_id: &str,
        id: i64,
        payload: &ConsentPayload,
    ) -> Result<(), IntelligenceFailure> {
        let protected = self.protect(household_id, payload)?;
        self.connect()?
            .execute(
                "UPDATE cloud_consents SET protected_payload = ?1
                 WHERE id = ?2 AND household_id = ?3",
                params![protected, id, household_id],
            )
            .map_err(|_| IntelligenceFailure::StorageUnavailable)?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn record_event(
        &self,
        household_id: &str,
        request: &IntelligenceRequest,
        consent: CloudConsentDecision,
        grant: Option<&CloudConsentScope>,
        granted_by: &str,
        outcome: CloudAssistanceOutcome,
        reason: &str,
        usage: IntelligenceUsage,
    ) -> Result<(), IntelligenceFailure> {
        let payload = AuditPayload {
            request_id: request.request_id.clone(),
            document_arrival_id: request.document_arrival_id.clone(),
            provider_id: request.provider_id.clone(),
            model_id: request.model_id.clone(),
            capability: request.capability,
            purpose: capability_purpose(request.capability).to_owned(),
            consent,
            consent_grant_id: grant.map(|scope| scope.id),
            granted_by: granted_by.trim().to_owned(),
            outcome,
            candidate_disposition: CandidateDisposition::Pending,
            reason: reason.to_owned(),
            usage,
        };
        let protected = self.protect(household_id, &payload)?;
        self.connect()?
            .execute(
                "INSERT INTO cloud_assistance_events (household_id, protected_payload)
                 VALUES (?1, ?2)",
                params![household_id, protected],
            )
            .map_err(|_| IntelligenceFailure::StorageUnavailable)?;
        Ok(())
    }

    fn record_waiting_failure(
        &self,
        household_id: &str,
        request: &IntelligenceRequest,
        consent: CloudConsentDecision,
        grant: &CloudConsentScope,
        failure: IntelligenceFailure,
    ) -> Result<(), IntelligenceFailure> {
        self.record_event(
            household_id,
            request,
            consent,
            Some(grant),
            &grant.granted_by,
            CloudAssistanceOutcome::WaitingForRetry,
            failure.history_reason(),
            IntelligenceUsage::default(),
        )
    }

    fn protect<T: Serialize>(
        &self,
        household_id: &str,
        value: &T,
    ) -> Result<String, IntelligenceFailure> {
        let bytes = serde_json::to_vec(value)
            .map_err(|_| IntelligenceFailure::ProtectedStateUnavailable)?;
        let protected = self
            .trusted_device
            .protect_household_state(household_id, &bytes)
            .map_err(map_trusted_device_error)?;
        serde_json::to_string(&protected)
            .map_err(|_| IntelligenceFailure::ProtectedStateUnavailable)
    }

    fn open_protected<T: DeserializeOwned>(
        &self,
        household_id: &str,
        protected: &str,
    ) -> Result<T, IntelligenceFailure> {
        let state: ProtectedHouseholdState = serde_json::from_str(protected)
            .map_err(|_| IntelligenceFailure::ProtectedStateUnavailable)?;
        let bytes = self
            .trusted_device
            .open_household_state(household_id, &state)
            .map_err(map_trusted_device_error)?;
        serde_json::from_slice(&bytes).map_err(|_| IntelligenceFailure::ProtectedStateUnavailable)
    }

    fn connect(&self) -> Result<Connection, IntelligenceFailure> {
        Connection::open(&self.database).map_err(|_| IntelligenceFailure::StorageUnavailable)
    }
}

impl IntelligenceFailure {
    fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::ProviderUnavailable | Self::GatewayUnavailable | Self::TimedOut
        )
    }

    fn history_reason(&self) -> &'static str {
        match self {
            Self::ProviderUnavailable => {
                "The selected Intelligence Provider was unavailable; Luna is waiting without switching providers."
            }
            Self::ProviderRejectedRequest => {
                "The selected Intelligence Provider rejected the request; Luna is waiting for a safe retry or member choice."
            }
            Self::AuthenticationUnavailable => {
                "The Luna gateway access credential was unavailable; no upstream credential was exposed."
            }
            Self::RateLimited => {
                "The selected Intelligence Provider rate-limited the request; Luna did not switch providers."
            }
            Self::TimedOut => {
                "The Intelligence Request timed out; Luna is waiting without switching providers."
            }
            Self::InvalidStructuredResult => {
                "Luna rejected an invalid structured result and made no Document Handling decision."
            }
            Self::GatewayUnavailable => {
                "The Luna-managed Intelligence Gateway was unavailable; Luna is waiting without switching providers."
            }
            Self::RequestCancelled => "The Intelligence Request was cancelled before completion.",
            _ => "Cloud Assistance could not complete and made no Document Handling decision.",
        }
    }
}

fn managed_provider_catalog() -> Vec<IntelligenceProviderDescriptor> {
    vec![IntelligenceProviderDescriptor {
        id: MANAGED_INTELLIGENCE_PROVIDER_ID.to_owned(),
        name: "OpenAI".to_owned(),
        description: "Luna-managed Cloud Assistance through the provisional LiteLLM gateway."
            .to_owned(),
        models: vec![IntelligenceModelDescriptor {
            id: MANAGED_INTELLIGENCE_MODEL_ID.to_owned(),
            name: "GPT-4.1 mini".to_owned(),
        }],
        managed_by_luna: true,
        auth_url: None,
    }]
}

fn consent_scope(household_id: &str, id: i64, payload: ConsentPayload) -> CloudConsentScope {
    CloudConsentScope {
        id,
        household_id: household_id.to_owned(),
        provider_id: payload.provider_id,
        model_id: payload.model_id,
        capability: payload.capability,
        purpose: payload.purpose,
        document_arrival_id: payload.document_arrival_id,
        future_scope: payload.future_scope,
        fields: payload.fields,
        kind: payload.kind,
        granted_by: payload.granted_by,
        created_at: payload.created_at,
        consumed_at: payload.consumed_at,
        revoked: payload.revoked_at.is_some(),
        revoked_at: payload.revoked_at,
    }
}

fn capability_purpose(capability: IntelligenceCapability) -> &'static str {
    match capability {
        IntelligenceCapability::DirectionInterpretation => "direction-interpretation",
    }
}

fn disclosed_fields(request: &IntelligenceRequest) -> Vec<String> {
    normalized_fields(
        request
            .evidence
            .iter()
            .map(|evidence| evidence.field.clone())
            .chain(
                request
                    .content_excerpts
                    .iter()
                    .map(|excerpt| format!("contentExcerpt:{}", excerpt.source)),
            )
            .collect(),
    )
}

fn normalized_fields(fields: Vec<String>) -> Vec<String> {
    fields
        .into_iter()
        .map(|field| field.trim().to_owned())
        .filter(|field| !field.is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn validate_result(
    request: &IntelligenceRequest,
    consent_grant_id: i64,
    result: UntrustedIntelligenceResult,
) -> Result<IntelligenceResult, IntelligenceFailure> {
    if result.request_id != request.request_id
        || result.document_arrival_id != request.document_arrival_id
        || result.provider_id != request.provider_id
        || result.model_id != request.model_id
    {
        return Err(IntelligenceFailure::InvalidStructuredResult);
    }
    let allowed = request
        .expected_response
        .allowed_fields
        .iter()
        .collect::<BTreeSet<_>>();
    if result.fields.iter().any(|(field, value)| {
        !allowed.contains(field)
            || value.trim().is_empty()
            || value.chars().count() > MAX_FIELD_VALUE_CHARS
    }) || result
        .evidence
        .iter()
        .any(|evidence| !allowed.contains(&evidence.field) || evidence.value.trim().is_empty())
    {
        return Err(IntelligenceFailure::InvalidStructuredResult);
    }
    let candidate_direction = request
        .expected_response
        .allow_candidate_direction
        .then(|| candidate_from_fields(&result.fields))
        .filter(|candidate| candidate != &CandidateDirectionInterpretation::default());
    Ok(IntelligenceResult {
        request_id: result.request_id,
        document_arrival_id: result.document_arrival_id,
        provider_id: result.provider_id,
        model_id: result.model_id,
        consent_grant_id,
        fields: result.fields,
        evidence: result.evidence,
        source_references: result.source_references,
        candidate_direction,
        usage: result.usage,
    })
}

fn candidate_from_fields(fields: &BTreeMap<String, String>) -> CandidateDirectionInterpretation {
    CandidateDirectionInterpretation {
        document_type: fields.get("documentType").cloned(),
        service_provider: fields.get("serviceProvider").cloned(),
        addressee: fields.get("addressee").cloned(),
        property: fields.get("property").cloned(),
        account: fields.get("account").cloned(),
        amount: fields.get("amount").cloned(),
        relevant_dates: fields
            .get("relevantDates")
            .map(|dates| {
                dates
                    .split(',')
                    .map(str::trim)
                    .filter(|date| !date.is_empty())
                    .map(ToOwned::to_owned)
                    .collect()
            })
            .unwrap_or_default(),
    }
}

fn gateway_credential_key(household_id: &str, gateway_id: &str) -> String {
    format!("luna.intelligence.gateway.{household_id}.{gateway_id}")
}

fn map_trusted_device_error(_: TrustedDeviceError) -> IntelligenceFailure {
    IntelligenceFailure::ProtectedStateUnavailable
}

impl From<VaultError> for IntelligenceFailure {
    fn from(_: VaultError) -> Self {
        Self::AuthenticationUnavailable
    }
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
                .expect("test vault lock")
                .insert(name.to_owned(), secret.to_vec());
            Ok(())
        }

        fn get_secret(&self, name: &str) -> Result<Option<Vec<u8>>, VaultError> {
            Ok(self.0.lock().expect("test vault lock").get(name).cloned())
        }

        fn delete_secret(&self, name: &str) -> Result<(), VaultError> {
            self.0.lock().expect("test vault lock").remove(name);
            Ok(())
        }
    }

    fn trusted_device() -> TrustedDeviceManager<TestVault> {
        let trusted = TrustedDeviceManager::new(TestVault::default());
        let enrollment = trusted
            .enrol_first_device("household")
            .expect("first device enrollment");
        trusted
            .confirm_recovery_key(
                "household",
                &enrollment.recovery_key,
                &enrollment.recovery_envelope,
            )
            .expect("confirm recovery key");
        trusted
            .set_current_key_epoch("household", 1)
            .expect("set key epoch");
        trusted
            .configure_device_pin("household", "123456")
            .expect("configure PIN");
        trusted
            .unlock_device("household", "123456")
            .expect("unlock device");
        trusted
    }

    fn store_with_gateway(
        gateway: DeterministicIntelligenceGateway,
    ) -> CloudIntelligenceStore<TestVault> {
        store_with_gateway_and_catalog(gateway, managed_provider_catalog())
    }

    fn store_with_gateway_and_catalog(
        gateway: DeterministicIntelligenceGateway,
        providers: Vec<IntelligenceProviderDescriptor>,
    ) -> CloudIntelligenceStore<TestVault> {
        static NEXT_DATABASE: AtomicUsize = AtomicUsize::new(0);
        let database = std::env::temp_dir().join(format!(
            "luna-intelligence-test-{}-{}.db",
            std::process::id(),
            NEXT_DATABASE.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_file(&database);
        CloudIntelligenceStore::open_with_gateway(database, trusted_device(), gateway, providers)
            .expect("open test intelligence store")
    }

    fn authorised_request(arrival: &str) -> IntelligenceRequest {
        IntelligenceRequest {
            request_id: format!("request-{arrival}"),
            document_arrival_id: arrival.to_owned(),
            capability: IntelligenceCapability::DirectionInterpretation,
            provider_id: MANAGED_INTELLIGENCE_PROVIDER_ID.to_owned(),
            model_id: MANAGED_INTELLIGENCE_MODEL_ID.to_owned(),
            evidence: vec![IntelligenceEvidence {
                field: "mediaType".to_owned(),
                value: "application/pdf".to_owned(),
                source: "Local Inspection".to_owned(),
            }],
            content_excerpts: vec![DocumentContentExcerpt {
                source: "locally extracted text".to_owned(),
                text: "Electricity account notice".to_owned(),
            }],
            expected_response: IntelligenceResponseSchema {
                allowed_fields: vec!["documentType".to_owned()],
                allow_candidate_direction: true,
            },
            consent_grant_id: None,
            constraints: IntelligenceExecutionConstraints {
                timeout_ms: 30_000,
                max_output_tokens: 512,
            },
        }
    }

    #[test]
    fn luna_sends_the_explicit_provider_and_model_through_the_gateway_contract() {
        let gateway = DeterministicIntelligenceGateway::new(
            "openai",
            "gpt-4.1-mini",
            BTreeMap::from([("documentType".to_owned(), "electricity bill".to_owned())]),
        );
        let store = store_with_gateway(gateway.clone());

        let result = store
            .evaluate_document(
                "household",
                IntelligenceSelection {
                    provider_id: "openai".to_owned(),
                    model_id: "gpt-4.1-mini".to_owned(),
                },
                authorised_request("arrival-42"),
                CloudConsentDecision::AllowOnce,
                "organiser-1",
                None,
            )
            .expect("evaluate document");

        assert_eq!(result.provider_id, "openai");
        assert_eq!(result.model_id, "gpt-4.1-mini");
        let requests = gateway.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].provider_id, "openai");
        assert_eq!(requests[0].model_id, "gpt-4.1-mini");
    }

    #[test]
    fn invalid_structured_identity_is_rejected_and_recorded_as_waiting() {
        let gateway = DeterministicIntelligenceGateway::new(
            "anthropic",
            "claude-sonnet-4-5",
            BTreeMap::new(),
        );
        let store = store_with_gateway(gateway);

        assert_eq!(
            store.evaluate_document(
                "household",
                IntelligenceSelection {
                    provider_id: "openai".to_owned(),
                    model_id: "gpt-4.1-mini".to_owned(),
                },
                authorised_request("arrival-invalid"),
                CloudConsentDecision::AllowOnce,
                "organiser-1",
                None,
            ),
            Err(IntelligenceFailure::InvalidStructuredResult)
        );

        let events = store
            .list_audit_events("household")
            .expect("list Cloud Assistance History");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].provider_id, "openai");
        assert_eq!(events[0].model_id, "gpt-4.1-mini");
        assert_eq!(events[0].outcome, CloudAssistanceOutcome::WaitingForRetry);
    }

    #[test]
    fn consent_for_one_intelligence_provider_cannot_authorise_another() {
        let gateway = DeterministicIntelligenceGateway::new(
            "anthropic",
            "claude-sonnet-4-5",
            BTreeMap::new(),
        );
        let providers = vec![
            managed_provider_catalog().remove(0),
            IntelligenceProviderDescriptor {
                id: "anthropic".to_owned(),
                name: "Anthropic".to_owned(),
                description: "Alternative evaluated route".to_owned(),
                models: vec![IntelligenceModelDescriptor {
                    id: "claude-sonnet-4-5".to_owned(),
                    name: "Claude Sonnet 4.5".to_owned(),
                }],
                managed_by_luna: true,
                auth_url: None,
            },
        ];
        let store = store_with_gateway_and_catalog(gateway.clone(), providers);
        let openai = IntelligenceSelection {
            provider_id: "openai".to_owned(),
            model_id: "gpt-4.1-mini".to_owned(),
        };
        let grant = store
            .grant_scope(
                "household",
                &openai,
                IntelligenceCapability::DirectionInterpretation,
                "direction-interpretation",
                disclosed_fields(&authorised_request("arrival-a")),
                "organiser-1",
                "Future difficult Documents with the same disclosed fields.",
            )
            .expect("grant OpenAI consent");
        let mut request = authorised_request("arrival-b");
        request.provider_id = "anthropic".to_owned();
        request.model_id = "claude-sonnet-4-5".to_owned();

        assert_eq!(
            store.evaluate_document(
                "household",
                IntelligenceSelection {
                    provider_id: "anthropic".to_owned(),
                    model_id: "claude-sonnet-4-5".to_owned(),
                },
                request,
                CloudConsentDecision::UseExistingScope,
                "organiser-1",
                Some(grant.id),
            ),
            Err(IntelligenceFailure::ConsentRequired)
        );
        assert!(gateway.requests().is_empty());
    }

    #[test]
    fn allow_once_consent_is_persisted_consumed_and_cannot_be_reused() {
        let gateway =
            DeterministicIntelligenceGateway::new("openai", "gpt-4.1-mini", BTreeMap::new());
        let store = store_with_gateway(gateway.clone());
        let selection = IntelligenceSelection {
            provider_id: "openai".to_owned(),
            model_id: "gpt-4.1-mini".to_owned(),
        };
        let result = store
            .evaluate_document(
                "household",
                selection.clone(),
                authorised_request("arrival-once"),
                CloudConsentDecision::AllowOnce,
                "organiser-1",
                None,
            )
            .expect("one-time evaluation");
        let grant = store
            .list_consent_scopes("household")
            .expect("list Consent Grants")
            .into_iter()
            .find(|grant| grant.id == result.consent_grant_id)
            .expect("one-time Consent Grant");
        assert_eq!(grant.kind, ConsentGrantKind::OneTime);
        assert!(grant.consumed_at.is_some());

        assert_eq!(
            store.evaluate_document(
                "household",
                selection,
                authorised_request("arrival-once"),
                CloudConsentDecision::UseExistingScope,
                "organiser-1",
                Some(grant.id),
            ),
            Err(IntelligenceFailure::ConsentConsumed)
        );
        assert_eq!(gateway.requests().len(), 1);
    }

    #[test]
    fn reusable_consent_is_inspectable_and_revocable() {
        let gateway =
            DeterministicIntelligenceGateway::new("openai", "gpt-4.1-mini", BTreeMap::new());
        let store = store_with_gateway(gateway.clone());
        let selection = IntelligenceSelection {
            provider_id: "openai".to_owned(),
            model_id: "gpt-4.1-mini".to_owned(),
        };
        let request = authorised_request("arrival-scope");
        let grant = store
            .grant_scope(
                "household",
                &selection,
                request.capability,
                "direction-interpretation",
                disclosed_fields(&request),
                "organiser-1",
                "Future difficult Documents with the same disclosed fields.",
            )
            .expect("grant reusable consent");
        assert_eq!(grant.kind, ConsentGrantKind::Reusable);
        assert_eq!(grant.granted_by, "organiser-1");
        assert_eq!(grant.provider_id, "openai");
        assert_eq!(grant.model_id, "gpt-4.1-mini");
        assert!(grant.future_scope.is_some());

        store
            .revoke_scope("household", grant.id)
            .expect("revoke Consent Grant");
        let revoked = store
            .list_consent_scopes("household")
            .expect("inspect Consent Grants")
            .into_iter()
            .find(|candidate| candidate.id == grant.id)
            .expect("revoked Consent Grant");
        assert!(revoked.revoked);
        assert_eq!(
            store.evaluate_document(
                "household",
                selection,
                request,
                CloudConsentDecision::UseExistingScope,
                "organiser-1",
                Some(grant.id),
            ),
            Err(IntelligenceFailure::ConsentRevoked)
        );
        assert!(gateway.requests().is_empty());
    }

    #[test]
    fn keep_local_records_the_choice_without_transmitting_document_information() {
        let gateway =
            DeterministicIntelligenceGateway::new("openai", "gpt-4.1-mini", BTreeMap::new());
        let store = store_with_gateway(gateway.clone());
        assert_eq!(
            store.evaluate_document(
                "household",
                IntelligenceSelection {
                    provider_id: "openai".to_owned(),
                    model_id: "gpt-4.1-mini".to_owned(),
                },
                authorised_request("arrival-local"),
                CloudConsentDecision::KeepLocal,
                "organiser-1",
                None,
            ),
            Err(IntelligenceFailure::LocalOnlyPolicy)
        );
        assert!(gateway.requests().is_empty());
        let events = store.list_audit_events("household").expect("list History");
        assert_eq!(events[0].outcome, CloudAssistanceOutcome::Denied);
        assert!(events[0].consent_grant_id.is_none());
    }

    #[test]
    fn safe_retry_reuses_the_exact_provider_model_and_request() {
        let gateway =
            DeterministicIntelligenceGateway::new("openai", "gpt-4.1-mini", BTreeMap::new());
        gateway.fail_next(IntelligenceFailure::ProviderUnavailable);
        let store = store_with_gateway(gateway.clone());
        store
            .evaluate_document(
                "household",
                IntelligenceSelection {
                    provider_id: "openai".to_owned(),
                    model_id: "gpt-4.1-mini".to_owned(),
                },
                authorised_request("arrival-retry"),
                CloudConsentDecision::AllowOnce,
                "organiser-1",
                None,
            )
            .expect("same-route retry");
        let requests = gateway.requests();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0], requests[1]);
        assert_eq!(requests[0].provider_id, "openai");
        assert_eq!(requests[0].model_id, "gpt-4.1-mini");
    }

    #[test]
    fn gateway_credentials_never_enter_history_or_ordinary_configuration() {
        let gateway =
            DeterministicIntelligenceGateway::new("openai", "gpt-4.1-mini", BTreeMap::new());
        let store = store_with_gateway(gateway);
        let secret = "narrow-gateway-secret-value";
        store
            .set_gateway_access_credential("household", secret.as_bytes())
            .expect("store gateway access credential");
        store
            .evaluate_document(
                "household",
                IntelligenceSelection {
                    provider_id: "openai".to_owned(),
                    model_id: "gpt-4.1-mini".to_owned(),
                },
                authorised_request("arrival-secret"),
                CloudConsentDecision::AllowOnce,
                "organiser-1",
                None,
            )
            .expect("evaluate Document");
        let events = store.list_audit_events("household").expect("list History");
        assert!(events.iter().all(|event| !event.reason.contains(secret)));
        let ordinary_database = std::fs::read(&store.database).expect("read test database");
        assert!(!String::from_utf8_lossy(&ordinary_database).contains(secret));
    }
}
