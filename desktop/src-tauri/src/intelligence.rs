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
    household_work::{
        HouseholdWorkKind, HouseholdWorkStatus, ProposedActionKind,
        ValidatedHouseholdWorkDirection, WorkFact, WorkFactCertainty, WorkFactKey,
    },
    litellm::LiteLlmGateway,
    portable_memory::{
        PortableConsentDetails, PortableConsentField, PortableConsentGrantKind,
        PortableConsentProvider, PortableConsentScope, PortableConsentState,
    },
    trusted_device::{
        CredentialVault, ProtectedHouseholdState, TrustedDeviceError, TrustedDeviceManager,
        VaultError,
    },
};

pub const MANAGED_INTELLIGENCE_PROVIDER_ID: &str = "openai";
pub const MANAGED_INTELLIGENCE_MODEL_ID: &str = "gpt-4.1-mini";
pub const BYOK_OPENAI_PROVIDER_ID: &str = "openai-byok";
pub const MAX_HOUSEHOLD_EXTRACTED_TEXT_CHARS: usize = 12_000;
const MAX_SAFE_RETRY_ATTEMPTS: usize = 2;
const MAX_FIELD_VALUE_CHARS: usize = 1_024;
const MAX_EVIDENCE_ITEMS: usize = 32;
const MAX_SOURCE_REFERENCE_CHARS: usize = 256;

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
    pub gateway_configured: bool,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PortableConsentExport {
    pub scope: CloudConsentScope,
    pub future_scope_evidence: Vec<IntelligenceEvidence>,
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
pub struct HouseholdAdministrationMessage {
    pub author: String,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HouseholdAdministrationSource {
    pub reference: String,
    pub filename: String,
    pub media_type: String,
    #[serde(skip_serializing)]
    pub original_base64: String,
    pub extracted_text: Option<String>,
    pub original_size_bytes: u64,
    pub extracted_text_truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HouseholdContextItem {
    pub category: String,
    pub value: String,
    pub source_reference: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailableHouseholdTool {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HouseholdAdministrationRequest {
    pub request_id: String,
    pub conversation_id: i64,
    pub current_message: String,
    pub relevant_conversation: Vec<HouseholdAdministrationMessage>,
    pub source: Option<HouseholdAdministrationSource>,
    pub household_context: Vec<HouseholdContextItem>,
    pub active_household_work: Vec<crate::household_work::HouseholdWorkSummary>,
    pub source_linked_household_work: Option<crate::household_work::HouseholdWorkSummary>,
    pub available_tools: Vec<AvailableHouseholdTool>,
    pub authority_and_approval_constraints: String,
    pub response_schema_version: String,
    pub constraints: IntelligenceExecutionConstraints,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HouseholdWorkOperation {
    None,
    Create,
    Update,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HouseholdWorkProposal {
    pub operation: HouseholdWorkOperation,
    #[serde(default)]
    pub work_id: Option<String>,
    #[serde(default)]
    pub kind: Option<HouseholdWorkKind>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub status: Option<HouseholdWorkStatus>,
    #[serde(default)]
    pub facts: Vec<WorkFact>,
    #[serde(default)]
    pub due_at: Option<String>,
    #[serde(default)]
    pub urgency: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HouseholdActionProposal {
    pub kind: ProposedActionKind,
    pub summary: String,
    #[serde(default)]
    pub arguments: BTreeMap<String, String>,
    pub approval_required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HouseholdClarification {
    pub question: String,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub field: Option<WorkFactKey>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UntrustedHouseholdAdministrationResult {
    pub request_id: String,
    pub provider_id: String,
    pub model_id: String,
    pub reply: String,
    pub work: HouseholdWorkProposal,
    #[serde(default)]
    pub clarification: Option<HouseholdClarification>,
    #[serde(default)]
    pub proposed_actions: Vec<HouseholdActionProposal>,
    pub usage: IntelligenceUsage,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HouseholdAdministrationResult {
    pub request_id: String,
    pub provider_id: String,
    pub model_id: String,
    pub reply: String,
    pub work: HouseholdWorkProposal,
    pub clarification: Option<HouseholdClarification>,
    pub proposed_actions: Vec<HouseholdActionProposal>,
    pub usage: IntelligenceUsage,
    #[serde(skip)]
    pub validated_member_direction: Option<ValidatedHouseholdWorkDirection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
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
    #[error("the selected Intelligence Provider credential is missing, invalid or revoked")]
    ProviderAuthenticationUnavailable,
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

    fn access_credential_id(&self, _provider_id: &str) -> &str {
        self.id()
    }

    fn requires_access_credential(&self) -> bool {
        true
    }

    fn evaluate_document(
        &self,
        request: &IntelligenceRequest,
        access_credential: Option<&[u8]>,
        provider_credential: Option<&[u8]>,
    ) -> Result<UntrustedIntelligenceResult, IntelligenceFailure>;

    fn reason_about_household_administration(
        &self,
        request: &HouseholdAdministrationRequest,
        access_credential: Option<&[u8]>,
        provider_credential: Option<&[u8]>,
    ) -> Result<UntrustedHouseholdAdministrationResult, IntelligenceFailure> {
        let legacy_request = IntelligenceRequest {
            request_id: request.request_id.clone(),
            document_arrival_id: request
                .source
                .as_ref()
                .map(|source| source.reference.clone())
                .unwrap_or_else(|| format!("conversation-{}", request.conversation_id)),
            capability: IntelligenceCapability::DirectionInterpretation,
            provider_id: MANAGED_INTELLIGENCE_PROVIDER_ID.to_owned(),
            model_id: MANAGED_INTELLIGENCE_MODEL_ID.to_owned(),
            evidence: request
                .household_context
                .iter()
                .map(|item| IntelligenceEvidence {
                    field: item.category.clone(),
                    value: item.value.clone(),
                    source: item.source_reference.clone(),
                })
                .collect(),
            content_excerpts: request
                .source
                .as_ref()
                .and_then(|source| source.extracted_text.clone())
                .map(|text| {
                    vec![DocumentContentExcerpt {
                        source: request
                            .source
                            .as_ref()
                            .map(|source| source.reference.clone())
                            .unwrap_or_else(|| "conversation".to_owned()),
                        text: text.chars().take(12_000).collect(),
                    }]
                })
                .unwrap_or_default(),
            expected_response: IntelligenceResponseSchema {
                allowed_fields: vec![
                    "documentType".to_owned(),
                    "serviceProvider".to_owned(),
                    "property".to_owned(),
                    "account".to_owned(),
                    "amount".to_owned(),
                    "relevantDates".to_owned(),
                ],
                allow_candidate_direction: true,
            },
            consent_grant_id: None,
            constraints: request.constraints.clone(),
        };
        let legacy =
            self.evaluate_document(&legacy_request, access_credential, provider_credential)?;
        Ok(household_result_from_fields(
            request,
            legacy.provider_id,
            legacy.model_id,
            legacy.fields,
            legacy.usage,
        ))
    }

    fn test_provider_connection(
        &self,
        request: &IntelligenceRequest,
        access_credential: Option<&[u8]>,
        provider_credential: &[u8],
    ) -> Result<(), IntelligenceFailure> {
        let result =
            self.evaluate_document(request, access_credential, Some(provider_credential))?;
        if result.provider_id != request.provider_id || result.model_id != request.model_id {
            return Err(IntelligenceFailure::InvalidStructuredResult);
        }
        Ok(())
    }
}

fn household_result_from_fields(
    request: &HouseholdAdministrationRequest,
    provider_id: String,
    model_id: String,
    fields: BTreeMap<String, String>,
    usage: IntelligenceUsage,
) -> UntrustedHouseholdAdministrationResult {
    let evidence_ref = request
        .source
        .as_ref()
        .map(|source| source.reference.clone())
        .unwrap_or_else(|| format!("conversation-{}", request.conversation_id));
    let mut facts = Vec::new();
    for (key, value) in fields {
        let fact_key = match key.as_str() {
            "serviceProvider" | "provider" => WorkFactKey::Provider,
            "property" => WorkFactKey::Property,
            "account" => WorkFactKey::Account,
            "amount" => WorkFactKey::Amount,
            "relevantDates" | "dueDate" => WorkFactKey::DueDate,
            "requiredAction" => WorkFactKey::RequiredAction,
            "urgency" => WorkFactKey::Urgency,
            _ => WorkFactKey::Other,
        };
        if !value.trim().is_empty() {
            facts.push(WorkFact {
                key: fact_key,
                value,
                evidence_refs: vec![evidence_ref.clone()],
                certainty: WorkFactCertainty::Likely,
            });
        }
    }
    let member_direction = validated_household_work_direction(&request.current_message);
    let needs_property_clarification = facts.iter().all(|fact| fact.key != WorkFactKey::Property)
        && request
            .source
            .as_ref()
            .and_then(|source| source.extracted_text.as_deref())
            .is_some_and(|text| {
                let text = text.to_ascii_lowercase();
                text.contains("ambiguous") || text.contains("two properties")
            });
    let relevant_work = request
        .source_linked_household_work
        .as_ref()
        .or(request.active_household_work.first());
    let (operation, work_id, status) = if let Some(direction) = member_direction {
        let status = match direction {
            ValidatedHouseholdWorkDirection::Complete => Some(HouseholdWorkStatus::Completed),
            ValidatedHouseholdWorkDirection::Dismiss => Some(HouseholdWorkStatus::Dismissed),
            ValidatedHouseholdWorkDirection::MarkNoLongerRelevant => {
                Some(HouseholdWorkStatus::NoLongerRelevant)
            }
            ValidatedHouseholdWorkDirection::Reopen => Some(HouseholdWorkStatus::Active),
        };
        (
            HouseholdWorkOperation::Update,
            relevant_work.map(|work| work.id.clone()),
            status,
        )
    } else {
        (
            if relevant_work.is_some() {
                HouseholdWorkOperation::Update
            } else {
                HouseholdWorkOperation::Create
            },
            relevant_work.map(|work| work.id.clone()),
            needs_property_clarification.then_some(HouseholdWorkStatus::NeedsClarification),
        )
    };
    let summary = if facts.is_empty() {
        "Uploaded household document requires attention.".to_owned()
    } else {
        format!("Household administration from {}", evidence_ref)
    };
    let reply = if let Some(status) = status {
        match status {
            HouseholdWorkStatus::Completed => {
                "I recorded that this household work is complete.".to_owned()
            }
            HouseholdWorkStatus::Dismissed => {
                "I dismissed this household work and kept the original source.".to_owned()
            }
            _ => "I updated the household work.".to_owned(),
        }
    } else if facts.is_empty() {
        "I have the document. I need one focused clarification after checking its contents."
            .to_owned()
    } else {
        format!("I found the relevant details and recorded household work from {evidence_ref}.")
    };
    let clarification = needs_property_clarification.then_some(HouseholdClarification {
        question: "Which household property does this document concern?".to_owned(),
        reason: Some(
            "The document does not identify one of the household properties clearly.".to_owned(),
        ),
        field: Some(WorkFactKey::Property),
    });
    let proposed_actions = if facts.is_empty() || status.is_some() || clarification.is_some() {
        Vec::new()
    } else {
        vec![HouseholdActionProposal {
            kind: ProposedActionKind::Reminder,
            summary: "Prepare a reminder for this household work after you approve it.".to_owned(),
            arguments: BTreeMap::new(),
            approval_required: true,
        }]
    };
    UntrustedHouseholdAdministrationResult {
        request_id: request.request_id.clone(),
        provider_id,
        model_id,
        reply,
        work: HouseholdWorkProposal {
            operation,
            work_id,
            kind: Some(HouseholdWorkKind::Bill),
            summary: Some(summary),
            status,
            facts,
            due_at: None,
            urgency: None,
        },
        clarification,
        proposed_actions,
        usage,
    }
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
        _provider_credential: Option<&[u8]>,
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

    fn test_provider_connection(
        &self,
        request: &IntelligenceRequest,
        access_credential: Option<&[u8]>,
        provider_credential: &[u8],
    ) -> Result<(), IntelligenceFailure> {
        self.evaluate_document(request, access_credential, Some(provider_credential))?;
        Ok(())
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
    #[serde(default)]
    future_scope_evidence: Vec<IntelligenceEvidence>,
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
            provider_catalog(),
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
        Ok(self
            .providers()
            .into_iter()
            .map(|descriptor| {
                let gateway_configured = !self.gateway.requires_access_credential()
                    || self
                        .trusted_device
                        .vault()
                        .get_secret(&gateway_credential_key(
                            household_id,
                            self.gateway.access_credential_id(&descriptor.id),
                        ))
                        .ok()
                        .flatten()
                        .is_some();
                let provider_configured = descriptor.managed_by_luna
                    || self
                        .trusted_device
                        .vault()
                        .get_secret(&provider_credential_key(household_id, &descriptor.id))
                        .ok()
                        .flatten()
                        .is_some();
                IntelligenceProviderStatus {
                    descriptor,
                    gateway_configured,
                    configured: gateway_configured && provider_configured,
                }
            })
            .collect())
    }

    pub fn set_provider_credential(
        &self,
        household_id: &str,
        provider_id: &str,
        credential: &[u8],
    ) -> Result<(), IntelligenceFailure> {
        let provider = self
            .providers
            .iter()
            .find(|provider| provider.id == provider_id && !provider.managed_by_luna)
            .ok_or(IntelligenceFailure::UnsupportedSelection)?;
        if credential.is_empty() {
            return Err(IntelligenceFailure::ProviderAuthenticationUnavailable);
        }
        self.trusted_device
            .vault()
            .set_secret(
                &provider_credential_key(household_id, &provider.id),
                credential,
            )
            .map_err(|_| IntelligenceFailure::ProviderAuthenticationUnavailable)
    }

    pub fn test_and_set_provider_credential(
        &self,
        household_id: &str,
        provider_id: &str,
        credential: &[u8],
    ) -> Result<(), IntelligenceFailure> {
        let provider = self
            .providers
            .iter()
            .find(|provider| provider.id == provider_id && !provider.managed_by_luna)
            .ok_or(IntelligenceFailure::UnsupportedSelection)?;
        if credential.is_empty() {
            return Err(IntelligenceFailure::ProviderAuthenticationUnavailable);
        }
        let model = provider
            .models
            .first()
            .ok_or(IntelligenceFailure::UnsupportedSelection)?;
        let access_credential = self
            .trusted_device
            .vault()
            .get_secret(&gateway_credential_key(
                household_id,
                self.gateway.access_credential_id(provider_id),
            ))
            .map_err(|_| IntelligenceFailure::AuthenticationUnavailable)?;
        if self.gateway.requires_access_credential() && access_credential.is_none() {
            return Err(IntelligenceFailure::AuthenticationUnavailable);
        }
        let request = IntelligenceRequest {
            request_id: format!("luna-provider-test-{}", now()),
            document_arrival_id: "synthetic-provider-connection-test".to_owned(),
            capability: IntelligenceCapability::DirectionInterpretation,
            provider_id: provider.id.clone(),
            model_id: model.id.clone(),
            evidence: Vec::new(),
            content_excerpts: vec![DocumentContentExcerpt {
                source: "synthetic connection test".to_owned(),
                text: "LUNA_SYNTHETIC_PROVIDER_CONNECTION_TEST. No Household information."
                    .to_owned(),
            }],
            expected_response: IntelligenceResponseSchema {
                allowed_fields: Vec::new(),
                allow_candidate_direction: false,
            },
            consent_grant_id: None,
            constraints: IntelligenceExecutionConstraints {
                timeout_ms: 30_000,
                max_output_tokens: 128,
            },
        };
        self.gateway.test_provider_connection(
            &request,
            access_credential.as_deref(),
            credential,
        )?;
        self.set_provider_credential(household_id, provider_id, credential)
    }

    pub fn clear_provider_credential(
        &self,
        household_id: &str,
        provider_id: &str,
    ) -> Result<(), IntelligenceFailure> {
        self.trusted_device
            .vault()
            .delete_secret(&provider_credential_key(household_id, provider_id))
            .map_err(|_| IntelligenceFailure::ProviderAuthenticationUnavailable)
    }

    pub fn set_gateway_access_credential(
        &self,
        household_id: &str,
        credential: &[u8],
    ) -> Result<(), IntelligenceFailure> {
        self.set_gateway_access_credential_for_provider(
            household_id,
            MANAGED_INTELLIGENCE_PROVIDER_ID,
            credential,
        )
    }

    pub fn set_gateway_access_credential_for_provider(
        &self,
        household_id: &str,
        provider_id: &str,
        credential: &[u8],
    ) -> Result<(), IntelligenceFailure> {
        if credential.is_empty() {
            return Err(IntelligenceFailure::AuthenticationUnavailable);
        }
        self.trusted_device
            .vault()
            .set_secret(
                &gateway_credential_key(
                    household_id,
                    self.gateway.access_credential_id(provider_id),
                ),
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
            .delete_secret(&gateway_credential_key(
                household_id,
                self.gateway
                    .access_credential_id(MANAGED_INTELLIGENCE_PROVIDER_ID),
            ))
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

    pub(crate) fn portable_consent_exports(
        &self,
        household_id: &str,
    ) -> Result<Vec<PortableConsentExport>, IntelligenceFailure> {
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
                Ok(PortableConsentExport {
                    scope: consent_scope(household_id, id, payload.clone()),
                    future_scope_evidence: payload.future_scope_evidence,
                })
            })
            .collect()
    }

    pub(crate) fn apply_portable_consent(
        &self,
        household_id: &str,
        local_id: Option<i64>,
        provider: &PortableConsentProvider,
        scope: &PortableConsentScope,
        state: &PortableConsentState,
        details: &PortableConsentDetails,
    ) -> Result<i64, IntelligenceFailure> {
        if details.kind == PortableConsentGrantKind::OneTime {
            return Err(IntelligenceFailure::ConsentRequired);
        }
        let payload = ConsentPayload {
            provider_id: match provider {
                PortableConsentProvider::LunaManaged => MANAGED_INTELLIGENCE_PROVIDER_ID.to_owned(),
                PortableConsentProvider::OpenAi => BYOK_OPENAI_PROVIDER_ID.to_owned(),
                PortableConsentProvider::Anthropic => "anthropic-byok".to_owned(),
            },
            model_id: details.model_id.clone(),
            capability: IntelligenceCapability::DirectionInterpretation,
            purpose: capability_purpose(IntelligenceCapability::DirectionInterpretation).to_owned(),
            document_arrival_id: None,
            future_scope: scope.future_scope.clone(),
            future_scope_evidence: scope
                .future_scope_evidence
                .iter()
                .map(|evidence| IntelligenceEvidence {
                    field: evidence.field.clone(),
                    value: evidence.value.clone(),
                    source: String::new(),
                })
                .collect(),
            fields: scope
                .fields
                .iter()
                .map(|field| match field {
                    PortableConsentField::DocumentType => "documentType",
                    PortableConsentField::ServiceProvider => "serviceProvider",
                    PortableConsentField::Addressee => "addressee",
                    PortableConsentField::Property => "property",
                    PortableConsentField::Account => "account",
                    PortableConsentField::Amount => "amount",
                    PortableConsentField::RelevantDates => "relevantDates",
                    PortableConsentField::Additional(field) => field.as_str(),
                })
                .map(str::to_owned)
                .collect(),
            kind: match details.kind {
                PortableConsentGrantKind::OneTime => ConsentGrantKind::OneTime,
                PortableConsentGrantKind::Reusable => ConsentGrantKind::Reusable,
            },
            granted_by: details.granted_by.to_string(),
            created_at: details.created_at.clone(),
            consumed_at: details.consumed_at.clone(),
            revoked_at: if *state == PortableConsentState::Revoked {
                details
                    .revoked_at
                    .clone()
                    .or_else(|| Some(details.created_at.clone()))
            } else {
                details.revoked_at.clone()
            },
        };
        if let Some(local_id) = local_id {
            if self.load_consent(household_id, local_id).is_ok() {
                self.save_consent(household_id, local_id, &payload)?;
                return Ok(local_id);
            }
        }
        Ok(self.insert_consent(household_id, payload)?.id)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn grant_scope(
        &self,
        household_id: &str,
        selection: &IntelligenceSelection,
        capability: IntelligenceCapability,
        purpose: &str,
        fields: Vec<String>,
        future_scope_evidence: Vec<IntelligenceEvidence>,
        granted_by: &str,
    ) -> Result<CloudConsentScope, IntelligenceFailure> {
        self.require_selection(selection)?;
        let future_scope = reusable_scope_description(&future_scope_evidence);
        self.insert_consent(
            household_id,
            ConsentPayload {
                provider_id: selection.provider_id.clone(),
                model_id: selection.model_id.clone(),
                capability,
                purpose: purpose.trim().to_owned(),
                document_arrival_id: None,
                future_scope: Some(future_scope),
                future_scope_evidence,
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
                    future_scope_evidence: Vec::new(),
                },
            )?,
            CloudConsentDecision::AllowForScope => self.grant_scope(
                household_id,
                &selection,
                request.capability,
                capability_purpose(request.capability),
                disclosed_fields(&request),
                request.evidence.clone(),
                granted_by,
            )?,
            CloudConsentDecision::UseExistingScope => {
                let id = existing_consent_grant_id.ok_or(IntelligenceFailure::ConsentRequired)?;
                let payload = self.load_consent(household_id, id)?;
                self.validate_consent(&payload, &selection, &request)?;
                consent_scope(household_id, id, payload)
            }
            CloudConsentDecision::KeepLocal => unreachable!(),
        };
        request.consent_grant_id = Some(grant.id);

        let credential = self
            .trusted_device
            .vault()
            .get_secret(&gateway_credential_key(
                household_id,
                self.gateway.access_credential_id(&selection.provider_id),
            ))
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
        let provider_credential = if self
            .providers
            .iter()
            .any(|provider| provider.id == selection.provider_id && !provider.managed_by_luna)
        {
            self.trusted_device
                .vault()
                .get_secret(&provider_credential_key(
                    household_id,
                    &selection.provider_id,
                ))
                .map_err(|_| IntelligenceFailure::AuthenticationUnavailable)?
        } else {
            None
        };
        if selection.provider_id == BYOK_OPENAI_PROVIDER_ID && provider_credential.is_none() {
            self.record_waiting_failure(
                household_id,
                &request,
                consent,
                &grant,
                IntelligenceFailure::ProviderAuthenticationUnavailable,
            )?;
            return Err(IntelligenceFailure::ProviderAuthenticationUnavailable);
        }
        self.consume_if_one_time(household_id, &grant)?;

        let mut last_failure = IntelligenceFailure::GatewayUnavailable;
        for attempt in 0..MAX_SAFE_RETRY_ATTEMPTS {
            match self.gateway.evaluate_document(
                &request,
                credential.as_deref(),
                provider_credential.as_deref(),
            ) {
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
                        &grant.granted_by,
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

    pub fn reason_about_household_administration(
        &self,
        household_id: &str,
        request: HouseholdAdministrationRequest,
    ) -> Result<HouseholdAdministrationResult, IntelligenceFailure> {
        let selection = IntelligenceSelection {
            provider_id: MANAGED_INTELLIGENCE_PROVIDER_ID.to_owned(),
            model_id: MANAGED_INTELLIGENCE_MODEL_ID.to_owned(),
        };
        self.require_selection(&selection)?;
        let credential = self
            .trusted_device
            .vault()
            .get_secret(&gateway_credential_key(
                household_id,
                self.gateway.access_credential_id(&selection.provider_id),
            ))
            .map_err(|_| IntelligenceFailure::AuthenticationUnavailable)?;
        if self.gateway.requires_access_credential() && credential.is_none() {
            return Err(IntelligenceFailure::AuthenticationUnavailable);
        }
        let provider_credential = None;
        let mut last_failure = IntelligenceFailure::GatewayUnavailable;
        for attempt in 0..MAX_SAFE_RETRY_ATTEMPTS {
            match self.gateway.reason_about_household_administration(
                &request,
                credential.as_deref(),
                provider_credential,
            ) {
                Ok(untrusted) => {
                    let result = validate_household_result(&request, untrusted)?;
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
        Err(last_failure)
    }

    pub fn record_candidate_disposition(
        &self,
        household_id: &str,
        request_id: &str,
        disposition: CandidateDisposition,
    ) -> Result<(), IntelligenceFailure> {
        let mut payload = self.load_latest_audit_payload(household_id, request_id)?;
        payload.candidate_disposition = disposition;
        self.append_audit_payload(household_id, &payload)
    }

    pub fn record_candidate_validation_failure(
        &self,
        household_id: &str,
        request_id: &str,
    ) -> Result<(), IntelligenceFailure> {
        let mut payload = self.load_latest_audit_payload(household_id, request_id)?;
        payload.outcome = CloudAssistanceOutcome::WaitingForRetry;
        payload.candidate_disposition = CandidateDisposition::Rejected;
        payload.reason = "Luna rejected the candidate Direction Interpretation during owning-domain validation; no Document Handling decision was made.".to_owned();
        self.append_audit_payload(household_id, &payload)
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
        let same_document_or_future_scope = payload.document_arrival_id.as_ref().map_or_else(
            || same_scope_evidence(&payload.future_scope_evidence, &request.evidence),
            |arrival| arrival == &request.document_arrival_id,
        );
        let requested = disclosed_fields(request);
        let fields_allowed = requested
            .iter()
            .all(|field| payload.fields.iter().any(|allowed| allowed == field));
        if same_selection && same_document_or_future_scope && fields_allowed {
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
        self.append_audit_payload(household_id, &payload)
    }

    fn load_latest_audit_payload(
        &self,
        household_id: &str,
        request_id: &str,
    ) -> Result<AuditPayload, IntelligenceFailure> {
        let connection = self.connect()?;
        let mut statement = connection
            .prepare(
                "SELECT protected_payload FROM cloud_assistance_events
                 WHERE household_id = ?1 ORDER BY id DESC",
            )
            .map_err(|_| IntelligenceFailure::StorageUnavailable)?;
        let rows = statement
            .query_map(params![household_id], |row| row.get::<_, String>(0))
            .map_err(|_| IntelligenceFailure::StorageUnavailable)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| IntelligenceFailure::StorageUnavailable)?;
        for protected in rows {
            let payload: AuditPayload = self.open_protected(household_id, &protected)?;
            if payload.request_id == request_id {
                return Ok(payload);
            }
        }
        Err(IntelligenceFailure::StorageUnavailable)
    }

    fn append_audit_payload(
        &self,
        household_id: &str,
        payload: &AuditPayload,
    ) -> Result<(), IntelligenceFailure> {
        let protected = self.protect(household_id, payload)?;
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

fn validate_household_result(
    request: &HouseholdAdministrationRequest,
    result: UntrustedHouseholdAdministrationResult,
) -> Result<HouseholdAdministrationResult, IntelligenceFailure> {
    let validated_member_direction =
        validated_household_work_direction(request.current_message.as_str());
    if result.request_id != request.request_id
        || result.provider_id != MANAGED_INTELLIGENCE_PROVIDER_ID
        || result.model_id != MANAGED_INTELLIGENCE_MODEL_ID
        || result.reply.trim().is_empty()
        || result.reply.chars().count() > 4_000
        || result.work.facts.len() > 16
        || result.proposed_actions.len() > 4
    {
        return Err(IntelligenceFailure::InvalidStructuredResult);
    }
    if matches!(result.work.operation, HouseholdWorkOperation::Create)
        && (result.work.work_id.is_some() || request.source_linked_household_work.is_some())
    {
        return Err(IntelligenceFailure::InvalidStructuredResult);
    }
    let mut updated_work = None;
    if matches!(result.work.operation, HouseholdWorkOperation::Update) {
        let Some(work_id) = result.work.work_id.as_ref() else {
            return Err(IntelligenceFailure::InvalidStructuredResult);
        };
        updated_work = request
            .active_household_work
            .iter()
            .find(|work| work.id == *work_id)
            .or_else(|| {
                request
                    .source_linked_household_work
                    .as_ref()
                    .filter(|work| work.id == *work_id)
            });
        if updated_work.is_none() {
            return Err(IntelligenceFailure::InvalidStructuredResult);
        }
    }
    if let Some(status) = result.work.status.filter(|status| status.is_terminal()) {
        if !matches!(result.work.operation, HouseholdWorkOperation::Update)
            || validated_member_direction.and_then(|direction| direction.terminal_status())
                != Some(status)
        {
            return Err(IntelligenceFailure::InvalidStructuredResult);
        }
    }
    if updated_work.is_some_and(|work| work.status.is_terminal()) {
        if validated_member_direction != Some(ValidatedHouseholdWorkDirection::Reopen)
            || result
                .work
                .status
                .is_none_or(HouseholdWorkStatus::is_terminal)
        {
            return Err(IntelligenceFailure::InvalidStructuredResult);
        }
    }
    let source_reference = request
        .source
        .as_ref()
        .map(|source| source.reference.as_str());
    for fact in &result.work.facts {
        if fact.value.trim().is_empty()
            || fact.value.chars().count() > MAX_FIELD_VALUE_CHARS
            || fact.evidence_refs.is_empty()
            || fact.evidence_refs.iter().any(|reference| {
                Some(reference.as_str()) != source_reference
                    && !request
                        .relevant_conversation
                        .iter()
                        .any(|message| reference == &format!("conversation-{}", message.author))
                    && !request
                        .household_context
                        .iter()
                        .any(|item| &item.source_reference == reference)
            })
        {
            return Err(IntelligenceFailure::InvalidStructuredResult);
        }
    }
    if let Some(clarification) = result.clarification.as_ref() {
        if clarification.question.trim().is_empty() || clarification.question.chars().count() > 500
        {
            return Err(IntelligenceFailure::InvalidStructuredResult);
        }
    }
    for action in &result.proposed_actions {
        let allowed_arguments: &[&str] = match action.kind {
            ProposedActionKind::DraftReply => &["recipient", "subject", "body"],
            ProposedActionKind::Reminder => &["remindAt", "message"],
        };
        if action.summary.trim().is_empty()
            || action.summary.chars().count() > 500
            || action.arguments.iter().any(|(key, value)| {
                !allowed_arguments.contains(&key.as_str())
                    || value.trim().is_empty()
                    || value.chars().count() > MAX_FIELD_VALUE_CHARS
                    || has_invalid_control_character(value)
            })
        {
            return Err(IntelligenceFailure::InvalidStructuredResult);
        }
    }
    Ok(HouseholdAdministrationResult {
        request_id: result.request_id,
        provider_id: result.provider_id,
        model_id: result.model_id,
        reply: result.reply,
        work: result.work,
        clarification: result.clarification,
        proposed_actions: result.proposed_actions,
        usage: result.usage,
        validated_member_direction,
    })
}

fn validated_household_work_direction(message: &str) -> Option<ValidatedHouseholdWorkDirection> {
    let message = message.to_ascii_lowercase();
    let contains_any = |phrases: &[&str]| phrases.iter().any(|phrase| message.contains(phrase));

    if contains_any(&[
        "wasn't paid",
        "was not paid",
        "isn't paid",
        "is not paid",
        "hasn't been paid",
        "has not been paid",
        "reopen",
        "still needs attention",
        "still need to pay",
    ]) {
        return Some(ValidatedHouseholdWorkDirection::Reopen);
    }
    if contains_any(&[
        "already paid",
        "paid it",
        "paid that",
        "has been paid",
        "is complete",
        "completed it",
        "done with this",
    ]) {
        return Some(ValidatedHouseholdWorkDirection::Complete);
    }
    if contains_any(&[
        "no longer relevant",
        "not relevant anymore",
        "doesn't apply",
        "does not apply",
    ]) {
        return Some(ValidatedHouseholdWorkDirection::MarkNoLongerRelevant);
    }
    if contains_any(&[
        "dismiss",
        "ignore this",
        "don't need this",
        "do not need this",
    ]) {
        return Some(ValidatedHouseholdWorkDirection::Dismiss);
    }
    None
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
            Self::ProviderAuthenticationUnavailable => {
                "The selected Intelligence Provider credential was unavailable or rejected; Luna did not use a managed route."
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

pub(crate) fn managed_provider_catalog() -> Vec<IntelligenceProviderDescriptor> {
    vec![IntelligenceProviderDescriptor {
        id: MANAGED_INTELLIGENCE_PROVIDER_ID.to_owned(),
        name: "OpenAI".to_owned(),
        description: "OpenAI Cloud Assistance provided by Luna for eligible Household Plans."
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
            .chain(request.expected_response.allowed_fields.iter().cloned())
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

fn reusable_scope_description(evidence: &[IntelligenceEvidence]) -> String {
    let media_type = evidence
        .iter()
        .find(|item| item.field == "mediaType")
        .map(|item| item.value.trim())
        .filter(|value| !value.is_empty())
        .unwrap_or("same-media-type");
    format!(
        "Future difficult {media_type} Documents with the same currently displayed local context values and disclosed fields."
    )
}

fn same_scope_evidence(
    allowed: &[IntelligenceEvidence],
    requested: &[IntelligenceEvidence],
) -> bool {
    let normalize = |evidence: &[IntelligenceEvidence]| {
        evidence
            .iter()
            .map(|item| (item.field.trim().to_owned(), item.value.trim().to_owned()))
            .collect::<BTreeSet<_>>()
    };
    normalize(allowed) == normalize(requested)
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
    let allowed_sources = request
        .evidence
        .iter()
        .map(|evidence| evidence.source.as_str())
        .chain(
            request
                .content_excerpts
                .iter()
                .map(|excerpt| excerpt.source.as_str()),
        )
        .collect::<BTreeSet<_>>();
    if result.fields.iter().any(|(field, value)| {
        !allowed.contains(field)
            || value.trim().is_empty()
            || value.chars().count() > MAX_FIELD_VALUE_CHARS
            || has_invalid_control_character(value)
    }) || result.evidence.iter().any(|evidence| {
        !allowed.contains(&evidence.field)
            || evidence.value.trim().is_empty()
            || evidence.value.chars().count() > MAX_FIELD_VALUE_CHARS
            || has_invalid_control_character(&evidence.value)
            || evidence.source_reference.as_ref().is_some_and(|reference| {
                reference.chars().count() > MAX_SOURCE_REFERENCE_CHARS
                    || has_invalid_control_character(reference)
                    || !allowed_sources.contains(reference.as_str())
            })
    }) || result.evidence.len() > MAX_EVIDENCE_ITEMS
        || result.source_references.len() > MAX_EVIDENCE_ITEMS
        || result.source_references.iter().any(|reference| {
            reference.chars().count() > MAX_SOURCE_REFERENCE_CHARS
                || has_invalid_control_character(reference)
                || !allowed_sources.contains(reference.as_str())
        })
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

fn has_invalid_control_character(value: &str) -> bool {
    value
        .chars()
        .any(|character| character.is_control() && !character.is_whitespace())
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

pub(crate) fn provider_catalog() -> Vec<IntelligenceProviderDescriptor> {
    let mut providers = managed_provider_catalog();
    providers.push(IntelligenceProviderDescriptor {
        id: BYOK_OPENAI_PROVIDER_ID.to_owned(),
        name: "OpenAI — bring your own key".to_owned(),
        description: "OpenAI Cloud Assistance billed directly to your provider account.".to_owned(),
        models: vec![IntelligenceModelDescriptor {
            id: MANAGED_INTELLIGENCE_MODEL_ID.to_owned(),
            name: "GPT-4.1 mini".to_owned(),
        }],
        managed_by_luna: false,
        auth_url: Some("https://platform.openai.com/api-keys".to_owned()),
    });
    providers
}

fn provider_credential_key(household_id: &str, provider_id: &str) -> String {
    format!("luna.intelligence.provider.{household_id}.{provider_id}")
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
        let scope_request = authorised_request("arrival-a");
        let grant = store
            .grant_scope(
                "household",
                &openai,
                IntelligenceCapability::DirectionInterpretation,
                "direction-interpretation",
                disclosed_fields(&scope_request),
                scope_request.evidence.clone(),
                "organiser-1",
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
                request.evidence.clone(),
                "organiser-1",
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
    fn candidate_disposition_appends_history_without_rewriting_the_presented_event() {
        let gateway = DeterministicIntelligenceGateway::new(
            "openai",
            "gpt-4.1-mini",
            BTreeMap::from([("documentType".to_owned(), "electricity bill".to_owned())]),
        );
        let store = store_with_gateway(gateway);
        let request = authorised_request("arrival-history");

        store
            .evaluate_document(
                "household",
                IntelligenceSelection {
                    provider_id: "openai".to_owned(),
                    model_id: "gpt-4.1-mini".to_owned(),
                },
                request.clone(),
                CloudConsentDecision::AllowOnce,
                "organiser-1",
                None,
            )
            .expect("evaluate document");
        store
            .record_candidate_disposition(
                "household",
                &request.request_id,
                CandidateDisposition::Accepted,
            )
            .expect("record accepted candidate");

        let events = store
            .list_audit_events("household")
            .expect("list immutable History");
        assert_eq!(events.len(), 2);
        assert_eq!(
            events[0].candidate_disposition,
            CandidateDisposition::Accepted
        );
        assert_eq!(
            events[1].candidate_disposition,
            CandidateDisposition::Pending
        );
        assert_ne!(events[0].id, events[1].id);
        assert_eq!(events[0].request_id, events[1].request_id);
    }

    #[test]
    fn gateway_credentials_remain_in_the_vault_and_out_of_public_history() {
        let gateway =
            DeterministicIntelligenceGateway::new("openai", "gpt-4.1-mini", BTreeMap::new());
        let trusted_device = trusted_device();
        let vault = trusted_device.vault().clone();
        let database = std::env::temp_dir().join(format!(
            "luna-credential-boundary-{}-{}.db",
            std::process::id(),
            now()
        ));
        let store = CloudIntelligenceStore::open_with_gateway(
            database,
            trusted_device,
            gateway,
            managed_provider_catalog(),
        )
        .expect("open Intelligence store");
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
        assert!(vault
            .0
            .lock()
            .expect("test vault lock")
            .values()
            .any(|value| value == secret.as_bytes()));
        let events = store.list_audit_events("household").expect("list History");
        let public_history = serde_json::to_string(&events).expect("serialize public History");
        let public_consents = serde_json::to_string(
            &store
                .list_consent_scopes("household")
                .expect("list public Consent Grants"),
        )
        .expect("serialize public Consent Grants");
        assert!(!public_history.contains(secret));
        assert!(!public_consents.contains(secret));
    }

    #[test]
    fn byok_connection_is_tested_before_the_provider_key_is_kept_only_in_the_vault() {
        let gateway = DeterministicIntelligenceGateway::new(
            BYOK_OPENAI_PROVIDER_ID,
            MANAGED_INTELLIGENCE_MODEL_ID,
            BTreeMap::new(),
        );
        let trusted_device = trusted_device();
        let vault = trusted_device.vault().clone();
        let database = std::env::temp_dir().join(format!(
            "luna-byok-credential-boundary-{}-{}.db",
            std::process::id(),
            now()
        ));
        let store = CloudIntelligenceStore::open_with_gateway(
            &database,
            trusted_device,
            gateway.clone(),
            provider_catalog(),
        )
        .expect("open Intelligence store");
        let secret = "customer-provider-secret-value";

        store
            .test_and_set_provider_credential(
                "household",
                BYOK_OPENAI_PROVIDER_ID,
                secret.as_bytes(),
            )
            .expect("test and store provider credential");

        assert_eq!(gateway.requests().len(), 1);
        assert_eq!(gateway.requests()[0].provider_id, BYOK_OPENAI_PROVIDER_ID);
        let status = store
            .provider_statuses("household")
            .expect("provider status")
            .into_iter()
            .find(|status| status.descriptor.id == BYOK_OPENAI_PROVIDER_ID)
            .expect("BYOK provider status");
        assert!(status.gateway_configured);
        assert!(status.configured);
        assert!(vault
            .0
            .lock()
            .expect("test vault lock")
            .values()
            .any(|value| value == secret.as_bytes()));
        assert!(
            !String::from_utf8_lossy(&std::fs::read(&database).expect("read database"))
                .contains(secret)
        );

        store
            .clear_provider_credential("household", BYOK_OPENAI_PROVIDER_ID)
            .expect("remove provider credential");
        assert!(
            !store
                .provider_statuses("household")
                .expect("provider status after removal")
                .into_iter()
                .find(|status| status.descriptor.id == BYOK_OPENAI_PROVIDER_ID)
                .expect("BYOK provider status after removal")
                .configured
        );
    }

    #[test]
    fn reusable_consent_cannot_expand_to_new_local_scope_response_fields_or_models() {
        let gateway =
            DeterministicIntelligenceGateway::new("openai", "gpt-4.1-mini", BTreeMap::new());
        let providers = vec![IntelligenceProviderDescriptor {
            id: "openai".to_owned(),
            name: "OpenAI".to_owned(),
            description: "Test routes".to_owned(),
            models: vec![
                IntelligenceModelDescriptor {
                    id: "gpt-4.1-mini".to_owned(),
                    name: "GPT-4.1 mini".to_owned(),
                },
                IntelligenceModelDescriptor {
                    id: "another-model".to_owned(),
                    name: "Another model".to_owned(),
                },
            ],
            managed_by_luna: true,
            auth_url: None,
        }];
        let store = store_with_gateway_and_catalog(gateway.clone(), providers);
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
                request.evidence.clone(),
                "organiser-1",
            )
            .expect("grant reusable consent");
        let mut changed_local_scope = authorised_request("arrival-other-media");
        changed_local_scope.evidence[0].value = "image/png".to_owned();
        assert_eq!(
            store.evaluate_document(
                "household",
                selection.clone(),
                changed_local_scope,
                CloudConsentDecision::UseExistingScope,
                "organiser-1",
                Some(grant.id),
            ),
            Err(IntelligenceFailure::ConsentRequired)
        );
        let mut wider = authorised_request("arrival-wider");
        wider
            .expected_response
            .allowed_fields
            .push("serviceProvider".to_owned());
        assert_eq!(
            store.evaluate_document(
                "household",
                selection.clone(),
                wider,
                CloudConsentDecision::UseExistingScope,
                "organiser-1",
                Some(grant.id),
            ),
            Err(IntelligenceFailure::ConsentRequired)
        );
        let mut changed_model = authorised_request("arrival-model");
        changed_model.model_id = "another-model".to_owned();
        assert_eq!(
            store.evaluate_document(
                "household",
                IntelligenceSelection {
                    provider_id: "openai".to_owned(),
                    model_id: "another-model".to_owned(),
                },
                changed_model,
                CloudConsentDecision::UseExistingScope,
                "organiser-1",
                Some(grant.id),
            ),
            Err(IntelligenceFailure::ConsentRequired)
        );
        assert!(gateway.requests().is_empty());
    }

    #[test]
    fn missing_gateway_authentication_does_not_consume_one_time_consent() {
        let database = std::env::temp_dir().join(format!(
            "luna-auth-consent-{}-{}.db",
            std::process::id(),
            now()
        ));
        let store =
            CloudIntelligenceStore::open(database, trusted_device()).expect("open Intelligence");
        assert_eq!(
            store.evaluate_document(
                "household",
                IntelligenceSelection {
                    provider_id: "openai".to_owned(),
                    model_id: "gpt-4.1-mini".to_owned(),
                },
                authorised_request("arrival-auth"),
                CloudConsentDecision::AllowOnce,
                "organiser-1",
                None,
            ),
            Err(IntelligenceFailure::AuthenticationUnavailable)
        );
        let scopes = store
            .list_consent_scopes("household")
            .expect("list consent");
        assert_eq!(scopes.len(), 1);
        assert!(scopes[0].consumed_at.is_none());
    }

    #[test]
    fn missing_byok_provider_credential_fails_closed_before_transmission_or_consent_use() {
        let gateway = DeterministicIntelligenceGateway::new(
            BYOK_OPENAI_PROVIDER_ID,
            MANAGED_INTELLIGENCE_MODEL_ID,
            BTreeMap::new(),
        );
        let store = store_with_gateway_and_catalog(gateway.clone(), provider_catalog());
        let mut request = authorised_request("arrival-byok-missing-key");
        request.provider_id = BYOK_OPENAI_PROVIDER_ID.to_owned();

        assert_eq!(
            store.evaluate_document(
                "household",
                IntelligenceSelection {
                    provider_id: BYOK_OPENAI_PROVIDER_ID.to_owned(),
                    model_id: MANAGED_INTELLIGENCE_MODEL_ID.to_owned(),
                },
                request,
                CloudConsentDecision::AllowOnce,
                "organiser-1",
                None,
            ),
            Err(IntelligenceFailure::ProviderAuthenticationUnavailable)
        );
        assert!(gateway.requests().is_empty());
        let scopes = store
            .list_consent_scopes("household")
            .expect("list BYOK consent");
        assert_eq!(scopes.len(), 1);
        assert!(scopes[0].consumed_at.is_none());
    }

    #[derive(Clone)]
    struct ModeAwareGateway;

    impl IntelligenceGateway for ModeAwareGateway {
        fn id(&self) -> &str {
            "mode-aware-test"
        }

        fn access_credential_id(&self, provider_id: &str) -> &str {
            if provider_id == BYOK_OPENAI_PROVIDER_ID {
                "byok-access"
            } else {
                "managed-access"
            }
        }

        fn evaluate_document(
            &self,
            _request: &IntelligenceRequest,
            _access_credential: Option<&[u8]>,
            _provider_credential: Option<&[u8]>,
        ) -> Result<UntrustedIntelligenceResult, IntelligenceFailure> {
            Err(IntelligenceFailure::GatewayUnavailable)
        }
    }

    #[test]
    fn a_byok_only_household_is_not_reported_as_having_managed_access() {
        let database = std::env::temp_dir().join(format!(
            "luna-mode-credential-boundary-{}-{}.db",
            std::process::id(),
            now()
        ));
        let store = CloudIntelligenceStore::open_with_gateway(
            database,
            trusted_device(),
            ModeAwareGateway,
            provider_catalog(),
        )
        .expect("open mode-aware Intelligence store");
        store
            .set_gateway_access_credential_for_provider(
                "household",
                BYOK_OPENAI_PROVIDER_ID,
                b"byok-only-gateway-key",
            )
            .expect("store BYOK gateway access");
        store
            .set_provider_credential(
                "household",
                BYOK_OPENAI_PROVIDER_ID,
                b"customer-provider-key",
            )
            .expect("store customer provider key");

        let statuses = store
            .provider_statuses("household")
            .expect("list mode-aware provider statuses");
        let managed = statuses
            .iter()
            .find(|status| status.descriptor.id == MANAGED_INTELLIGENCE_PROVIDER_ID)
            .expect("managed status");
        assert!(!managed.gateway_configured);
        assert!(!managed.configured);
        let byok = statuses
            .iter()
            .find(|status| status.descriptor.id == BYOK_OPENAI_PROVIDER_ID)
            .expect("BYOK status");
        assert!(byok.gateway_configured);
        assert!(byok.configured);
    }

    #[test]
    fn provider_catalog_uses_customer_language_instead_of_gateway_implementation_terms() {
        for provider in provider_catalog() {
            let description = provider.description.to_ascii_lowercase();
            assert!(!description.contains("litellm"));
            assert!(!description.contains("gateway"));
        }
    }

    #[derive(Clone)]
    struct OversizedEvidenceGateway;

    impl IntelligenceGateway for OversizedEvidenceGateway {
        fn id(&self) -> &str {
            "oversized-evidence-test"
        }

        fn requires_access_credential(&self) -> bool {
            false
        }

        fn evaluate_document(
            &self,
            request: &IntelligenceRequest,
            _access_credential: Option<&[u8]>,
            _provider_credential: Option<&[u8]>,
        ) -> Result<UntrustedIntelligenceResult, IntelligenceFailure> {
            Ok(UntrustedIntelligenceResult {
                request_id: request.request_id.clone(),
                document_arrival_id: request.document_arrival_id.clone(),
                provider_id: request.provider_id.clone(),
                model_id: request.model_id.clone(),
                fields: BTreeMap::new(),
                evidence: vec![AdditionalIntelligenceEvidence {
                    field: "documentType".to_owned(),
                    value: "x".repeat(1_025),
                    source_reference: Some("invented source".to_owned()),
                }],
                source_references: vec!["invented source".to_owned()],
                usage: IntelligenceUsage::default(),
            })
        }
    }

    #[test]
    fn oversized_or_untraceable_provider_evidence_is_rejected_through_the_gateway_boundary() {
        let request = authorised_request("arrival-untrusted");
        let database = std::env::temp_dir().join(format!(
            "luna-untrusted-boundary-{}-{}.db",
            std::process::id(),
            now()
        ));
        let store = CloudIntelligenceStore::open_with_gateway(
            database,
            trusted_device(),
            OversizedEvidenceGateway,
            managed_provider_catalog(),
        )
        .expect("open Intelligence store");
        assert_eq!(
            store.evaluate_document(
                "household",
                IntelligenceSelection {
                    provider_id: "openai".to_owned(),
                    model_id: "gpt-4.1-mini".to_owned(),
                },
                request,
                CloudConsentDecision::AllowOnce,
                "organiser-1",
                None,
            ),
            Err(IntelligenceFailure::InvalidStructuredResult)
        );
    }

    fn household_validation_request(
        message: &str,
        status: HouseholdWorkStatus,
    ) -> HouseholdAdministrationRequest {
        let linked_work = crate::household_work::HouseholdWorkSummary {
            id: "work-1".to_owned(),
            status,
            kind: HouseholdWorkKind::Bill,
            summary: "Electricity bill".to_owned(),
            facts: Vec::new(),
            source_refs: vec!["document-1".to_owned()],
            due_at: None,
            urgency: None,
        };
        HouseholdAdministrationRequest {
            request_id: "request-1".to_owned(),
            conversation_id: 1,
            current_message: message.to_owned(),
            relevant_conversation: Vec::new(),
            source: None,
            household_context: Vec::new(),
            active_household_work: status
                .is_open()
                .then_some(linked_work.clone())
                .into_iter()
                .collect(),
            source_linked_household_work: Some(linked_work),
            available_tools: Vec::new(),
            authority_and_approval_constraints: "Luna validates transitions.".to_owned(),
            response_schema_version: "household-administration.v1".to_owned(),
            constraints: IntelligenceExecutionConstraints {
                timeout_ms: 1_000,
                max_output_tokens: 500,
            },
        }
    }

    fn household_update(status: HouseholdWorkStatus) -> UntrustedHouseholdAdministrationResult {
        UntrustedHouseholdAdministrationResult {
            request_id: "request-1".to_owned(),
            provider_id: MANAGED_INTELLIGENCE_PROVIDER_ID.to_owned(),
            model_id: MANAGED_INTELLIGENCE_MODEL_ID.to_owned(),
            reply: "I updated the household work.".to_owned(),
            work: HouseholdWorkProposal {
                operation: HouseholdWorkOperation::Update,
                work_id: Some("work-1".to_owned()),
                kind: None,
                summary: None,
                status: Some(status),
                facts: Vec::new(),
                due_at: None,
                urgency: None,
            },
            clarification: None,
            proposed_actions: Vec::new(),
            usage: IntelligenceUsage::default(),
        }
    }

    #[test]
    fn hallucinated_terminal_household_work_states_are_rejected() {
        for status in [
            HouseholdWorkStatus::Completed,
            HouseholdWorkStatus::Dismissed,
            HouseholdWorkStatus::NoLongerRelevant,
        ] {
            assert_eq!(
                validate_household_result(
                    &household_validation_request("When is this due?", HouseholdWorkStatus::Active),
                    household_update(status),
                ),
                Err(IntelligenceFailure::InvalidStructuredResult)
            );
        }
    }

    #[test]
    fn explicit_member_payment_authorises_completion_inside_luna() {
        let result = validate_household_result(
            &household_validation_request(
                "I already paid that.",
                HouseholdWorkStatus::AwaitingApproval,
            ),
            household_update(HouseholdWorkStatus::Completed),
        )
        .expect("validate explicit completion direction");
        assert_eq!(
            result.validated_member_direction,
            Some(ValidatedHouseholdWorkDirection::Complete)
        );
    }

    #[test]
    fn a_read_only_question_cannot_close_household_work() {
        let mut proposal = household_update(HouseholdWorkStatus::Completed);
        proposal.work.operation = HouseholdWorkOperation::None;
        proposal.work.work_id = None;
        assert_eq!(
            validate_household_result(
                &household_validation_request(
                    "Can you explain this bill?",
                    HouseholdWorkStatus::AwaitingApproval,
                ),
                proposal,
            ),
            Err(IntelligenceFailure::InvalidStructuredResult)
        );
    }

    #[test]
    fn explicit_correction_can_reopen_source_linked_terminal_work() {
        let mut proposal = household_update(HouseholdWorkStatus::Active);
        let request = household_validation_request(
            "Actually, it wasn't paid.",
            HouseholdWorkStatus::Completed,
        );
        let result = validate_household_result(&request, proposal.clone())
            .expect("validate explicit reopening direction");
        assert_eq!(
            result.validated_member_direction,
            Some(ValidatedHouseholdWorkDirection::Reopen)
        );

        proposal.work.operation = HouseholdWorkOperation::Create;
        proposal.work.work_id = None;
        assert_eq!(
            validate_household_result(&request, proposal),
            Err(IntelligenceFailure::InvalidStructuredResult)
        );
    }
}
