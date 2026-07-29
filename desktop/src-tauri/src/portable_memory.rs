use std::{
    collections::{HashMap, HashSet},
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Component, Path, PathBuf},
};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    conversation::{
        AuditAuthority as ConversationAuditAuthority, AuditEventKind, ConversationError,
        ConversationStore, DocumentArrival, DocumentProcessingState, DuplicateDecision,
        DuplicateKind, RebuiltDocumentRelationship,
    },
    intelligence::{
        CandidateDisposition, CloudAssistanceOutcome, CloudIntelligenceStore, ConsentGrantKind,
        IntelligenceCapability, IntelligenceFailure,
    },
    CredentialVault, ProtectedHouseholdState, TrustedDeviceError, TrustedDeviceManager,
};

const PORTABLE_MEMORY_DIRECTORY: &str = ".luna-memory";
const PORTABLE_MEMORY_VERSION: u8 = 1;
const PORTABLE_EVENT_EXTENSION: &str = "luna-event";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortableConversationReference {
    pub conversation_id: PortableReference,
    pub message_id: PortableReference,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PortableReference(String);

impl PortableReference {
    pub fn new(value: impl Into<String>) -> Result<Self, PortableMemoryError> {
        let value = value.into();
        let valid = value.split_once(':').is_some_and(|(kind, id)| {
            matches!(
                kind,
                "event"
                    | "conversation"
                    | "message"
                    | "filing-rule"
                    | "document"
                    | "direction"
                    | "grant"
                    | "field"
                    | "document-type"
                    | "subject"
            ) && valid_uuid(id)
        });
        if !valid {
            return Err(PortableMemoryError::SensitiveMaterial);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn kind(&self) -> &str {
        self.0
            .split_once(':')
            .map(|(kind, _)| kind)
            .expect("PortableReference is validated at construction")
    }
}

fn valid_uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()
            }
        })
}

impl std::fmt::Display for PortableReference {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Serialize for PortableReference {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for PortableReference {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(D::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PortableFilingRuleState {
    Active,
    Paused,
    Deleted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortableFilingRuleDefinition {
    pub document_type: String,
    pub service_provider: String,
    pub addressee: String,
    pub property: Option<String>,
    pub account: Option<String>,
    pub file_name: String,
    pub cabinet_destination: String,
    pub taught_by: PortableReference,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PortableDocumentRelationshipKind {
    ExactDuplicate,
    LinkedCopy,
    UpdatedVersion,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PortableMemberDirectionKind {
    FileDocument,
    KeepBoth,
    ReplaceExisting,
    UpdateFilingRule,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PortableAuthority {
    MemberDirection,
    FilingRule,
    AuthorityGrant,
    ConsentGrant,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PortableConsentProvider {
    LunaManaged,
    OpenAi,
    Anthropic,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PortableIntelligenceCapability {
    DirectionInterpretation,
    ConversationReply,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PortableConsentGrantKind {
    OneTime,
    Reusable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PortableConsentPurpose {
    DocumentEvaluation,
    ConversationReply,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PortableConsentState {
    AllowedOnce,
    Granted,
    Revoked,
    Denied,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PortableExecutionOutcomeKind {
    FiledAndVerified,
    FilingRuleChanged,
    CloudAssistanceCompleted,
    KeptLocal,
    WaitingForCloudAssistance,
    CabinetUnavailable,
    ProviderUnavailable,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PortableAuditEventKind {
    DocumentFiled,
    ExactMatchHandledAutomatically,
    FilingRuleChanged,
    ConsentChanged,
    ExecutionCompleted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortableConsentScope {
    pub document_type: Option<PortableReference>,
    #[serde(default)]
    pub document_reference: Option<PortableReference>,
    #[serde(default)]
    pub future_scope: Option<String>,
    #[serde(default)]
    pub future_scope_evidence: Vec<PortableConsentScopeEvidence>,
    pub fields: Vec<PortableConsentField>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortableConsentScopeEvidence {
    pub field: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PortableConsentField {
    DocumentType,
    ServiceProvider,
    Addressee,
    Property,
    Account,
    Amount,
    RelevantDates,
    Additional(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PortableCandidateDisposition {
    Pending,
    Accepted,
    Corrected,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortableConsentDetails {
    pub model_id: String,
    pub capability: PortableIntelligenceCapability,
    pub purpose: PortableConsentPurpose,
    pub kind: PortableConsentGrantKind,
    pub granted_by: PortableReference,
    pub created_at: String,
    pub consumed_at: Option<String>,
    pub revoked_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PortableFact {
    FilingRule {
        rule_reference: PortableReference,
        state: PortableFilingRuleState,
        definition: PortableFilingRuleDefinition,
    },
    DocumentRelationship {
        document_reference: PortableReference,
        related_document_reference: PortableReference,
        relationship: PortableDocumentRelationshipKind,
    },
    MemberDirection {
        direction: PortableMemberDirectionKind,
        subject_reference: PortableReference,
    },
    AuthorityGrant {
        grant_reference: PortableReference,
        subject_reference: PortableReference,
        scope: Vec<PortableReference>,
    },
    ConsentGrant {
        grant_reference: PortableReference,
        provider: PortableConsentProvider,
        scope: PortableConsentScope,
        state: PortableConsentState,
        details: PortableConsentDetails,
    },
    ExecutionOutcome {
        subject_reference: PortableReference,
        outcome: PortableExecutionOutcomeKind,
    },
    AuditEvent {
        event_kind: PortableAuditEventKind,
        authority: PortableAuthority,
        subject_reference: PortableReference,
        outcome: PortableExecutionOutcomeKind,
        #[serde(default)]
        candidate_disposition: Option<PortableCandidateDisposition>,
    },
    ConflictResolution {
        existing_event_id: PortableReference,
        conflicting_event_id: PortableReference,
        chosen_event_id: PortableReference,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortableEventDraft {
    pub event_id: String,
    pub sequence: u64,
    pub previous_event_digest: Option<String>,
    pub supersedes_event_digest: Option<String>,
    pub occurred_at: String,
    pub conversation_reference: Option<PortableConversationReference>,
    pub fact: PortableFact,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortableConflictResolutionDraft {
    pub event_id: String,
    pub sequence: u64,
    pub previous_event_digest: Option<String>,
    pub occurred_at: String,
    pub chosen_event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortableEvent {
    pub household_id: String,
    pub event_id: String,
    pub signer_device_id: String,
    pub key_epoch: u32,
    pub sequence: u64,
    pub previous_event_digest: Option<String>,
    pub supersedes_event_digest: Option<String>,
    pub occurred_at: String,
    pub conversation_reference: Option<PortableConversationReference>,
    pub fact: PortableFact,
    pub digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustedDeviceAuthorization {
    pub device_id: String,
    pub authorization_public_key: [u8; 32],
    pub activated_key_epoch: u32,
    pub revoked_after: Option<PortableAuthorizationCutoff>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortableAuthorizationCutoff {
    pub key_epoch: u32,
    pub sequence: u64,
    pub event_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortableConflict {
    pub id: i64,
    pub subject_reference: String,
    pub existing_event_id: String,
    pub conflicting_event_id: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortableImportReport {
    pub imported: usize,
    pub duplicates: usize,
    pub conflicts: Vec<PortableConflict>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortableHouseholdProjection {
    pub filing_rules: Vec<PortableEvent>,
    pub document_relationships: Vec<PortableEvent>,
    pub member_directions: Vec<PortableEvent>,
    pub authority_grants: Vec<PortableEvent>,
    pub consent_grants: Vec<PortableEvent>,
    pub execution_outcomes: Vec<PortableEvent>,
    pub audit_events: Vec<PortableEvent>,
    pub conflicts: Vec<PortableConflict>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortableHistoryEvent {
    pub event_id: String,
    pub occurred_at: String,
    pub event_kind: PortableAuditEventKind,
    pub authority: PortableAuthority,
    pub subject_reference: PortableReference,
    pub outcome: PortableExecutionOutcomeKind,
    #[serde(default)]
    pub candidate_disposition: Option<PortableCandidateDisposition>,
}

#[derive(Debug, Error)]
pub enum PortableMemoryError {
    #[error("The configured Cabinet is unavailable.")]
    CabinetUnavailable,
    #[error("The portable event is invalid.")]
    InvalidEvent,
    #[error(
        "Credentials, tokens, private keys and plaintext secrets cannot enter portable memory."
    )]
    SensitiveMaterial,
    #[error("The portable record is not signed by a Trusted Device.")]
    UntrustedDevice,
    #[error("The portable record was modified or is replay-invalid.")]
    RejectedRecord,
    #[error("A different portable record already occupies this append-only event location.")]
    RecordConflict,
    #[error("Portable-memory storage is unavailable.")]
    Storage(#[from] rusqlite::Error),
    #[error("Portable-memory serialization failed.")]
    Serialization(#[from] serde_json::Error),
    #[error("Portable-memory cryptography failed.")]
    TrustedDevice(#[from] TrustedDeviceError),
    #[error("Portable-memory files are unavailable.")]
    File(#[from] io::Error),
    #[error("Portable memory could not read or rebuild Conversation-owned state.")]
    Conversation(#[from] ConversationError),
    #[error("Portable memory could not read or rebuild Cloud Assistance state.")]
    Intelligence(#[from] IntelligenceFailure),
}

#[derive(Clone)]
pub struct PortableMemoryStore<V: CredentialVault> {
    database: PathBuf,
    trusted_device: TrustedDeviceManager<V>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct UnsignedPortableEvent<'a> {
    household_id: &'a str,
    event_id: &'a str,
    signer_device_id: &'a str,
    key_epoch: u32,
    sequence: u64,
    previous_event_digest: &'a Option<String>,
    supersedes_event_digest: &'a Option<String>,
    occurred_at: &'a str,
    conversation_reference: &'a Option<PortableConversationReference>,
    fact: &'a PortableFact,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UnsignedPortableEnvelope {
    version: u8,
    signer_device_id: String,
    key_epoch: u32,
    sequence: u64,
    previous_event_digest: Option<String>,
    event_digest: String,
    protected_event: ProtectedHouseholdState,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PortableEnvelope {
    version: u8,
    signer_device_id: String,
    key_epoch: u32,
    sequence: u64,
    previous_event_digest: Option<String>,
    event_digest: String,
    protected_event: ProtectedHouseholdState,
    signature: String,
}

impl<V: CredentialVault> PortableMemoryStore<V> {
    pub fn open(
        database: impl AsRef<Path>,
        trusted_device: TrustedDeviceManager<V>,
    ) -> Result<Self, PortableMemoryError> {
        let store = Self {
            database: database.as_ref().to_owned(),
            trusted_device,
        };
        let connection = store.connect()?;
        connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS portable_events (
                household_id TEXT NOT NULL,
                event_id TEXT NOT NULL,
                event_digest TEXT NOT NULL,
                signer_device_id TEXT NOT NULL,
                key_epoch INTEGER NOT NULL,
                sequence INTEGER NOT NULL,
                protected_payload TEXT NOT NULL,
                portable_record BLOB,
                PRIMARY KEY (household_id, event_id)
            );
            CREATE UNIQUE INDEX IF NOT EXISTS portable_device_sequence
                ON portable_events(household_id, signer_device_id, sequence);
            CREATE TABLE IF NOT EXISTS portable_subject_heads (
                household_id TEXT NOT NULL,
                subject_reference TEXT NOT NULL,
                event_id TEXT NOT NULL,
                event_digest TEXT NOT NULL,
                PRIMARY KEY (household_id, subject_reference)
            );
            CREATE TABLE IF NOT EXISTS portable_conflicts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                household_id TEXT NOT NULL,
                subject_reference TEXT NOT NULL,
                existing_event_id TEXT NOT NULL,
                conflicting_event_id TEXT NOT NULL,
                resolved_event_id TEXT,
                resolution_event_id TEXT
            );
            CREATE UNIQUE INDEX IF NOT EXISTS portable_conflict_pair
                ON portable_conflicts(
                    household_id, subject_reference, existing_event_id, conflicting_event_id
                );
            CREATE TABLE IF NOT EXISTS portable_resolution_choices (
                household_id TEXT NOT NULL,
                resolution_event_id TEXT NOT NULL,
                conflict_id INTEGER,
                subject_reference TEXT NOT NULL,
                chosen_event_id TEXT NOT NULL,
                PRIMARY KEY (household_id, resolution_event_id)
            );
            CREATE TABLE IF NOT EXISTS portable_domain_exports (
                household_id TEXT NOT NULL,
                source_kind TEXT NOT NULL,
                source_id TEXT NOT NULL,
                fingerprint TEXT NOT NULL,
                event_id TEXT NOT NULL,
                event_digest TEXT NOT NULL,
                PRIMARY KEY (household_id, source_kind, source_id)
            );
            CREATE TABLE IF NOT EXISTS portable_entity_bindings (
                household_id TEXT NOT NULL,
                entity_kind TEXT NOT NULL,
                local_id INTEGER NOT NULL,
                portable_reference TEXT NOT NULL,
                PRIMARY KEY (household_id, entity_kind, local_id),
                UNIQUE (household_id, entity_kind, portable_reference)
            );",
        )?;
        if !portable_column_exists(&connection, "key_epoch")? {
            connection.execute(
                "ALTER TABLE portable_events
                 ADD COLUMN key_epoch INTEGER NOT NULL DEFAULT 1",
                [],
            )?;
        }
        if !portable_column_exists(&connection, "portable_record")? {
            connection.execute(
                "ALTER TABLE portable_events ADD COLUMN portable_record BLOB",
                [],
            )?;
        }
        if !table_column_exists(&connection, "portable_conflicts", "resolution_event_id")? {
            connection.execute(
                "ALTER TABLE portable_conflicts ADD COLUMN resolution_event_id TEXT",
                [],
            )?;
        }
        if !table_column_exists(&connection, "portable_resolution_choices", "conflict_id")? {
            connection.execute(
                "ALTER TABLE portable_resolution_choices ADD COLUMN conflict_id INTEGER",
                [],
            )?;
        }
        Ok(store)
    }

    pub fn append(
        &self,
        household_id: &str,
        cabinet_root: impl AsRef<Path>,
        draft: PortableEventDraft,
    ) -> Result<PortableEvent, PortableMemoryError> {
        validate_draft(&draft)?;
        let signer_device_id = self
            .trusted_device
            .current_device_public_key(household_id)?;
        let record_name = format!(
            "{}.{PORTABLE_EVENT_EXTENSION}",
            sha256_hex(draft.event_id.as_bytes())
        );
        let mut connection = self.connect()?;
        let existing: Option<(u32, String, Option<Vec<u8>>)> = connection
            .query_row(
                "SELECT key_epoch, protected_payload, portable_record FROM portable_events
                  WHERE household_id = ?1 AND event_id = ?2",
                params![household_id, draft.event_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        if let Some((key_epoch, protected, Some(record))) = existing {
            let protected: ProtectedHouseholdState = serde_json::from_str(&protected)?;
            let plaintext = self.trusted_device.open_household_state_at_epoch(
                household_id,
                key_epoch,
                &protected,
            )?;
            let event: PortableEvent = serde_json::from_slice(&plaintext)?;
            if event.signer_device_id != signer_device_id || !event_matches_draft(&event, &draft) {
                return Err(PortableMemoryError::RecordConflict);
            }
            let path = event_area(cabinet_root.as_ref())?.join(record_name);
            append_create_only(&path, &record)?;
            return Ok(event);
        }
        if existing.is_some() {
            return Err(PortableMemoryError::RecordConflict);
        }
        let key_epoch = self.trusted_device.current_key_epoch(household_id)?;
        let digest = event_digest(&UnsignedPortableEvent {
            household_id,
            event_id: &draft.event_id,
            signer_device_id: &signer_device_id,
            key_epoch,
            sequence: draft.sequence,
            previous_event_digest: &draft.previous_event_digest,
            supersedes_event_digest: &draft.supersedes_event_digest,
            occurred_at: &draft.occurred_at,
            conversation_reference: &draft.conversation_reference,
            fact: &draft.fact,
        })?;
        let event = PortableEvent {
            household_id: household_id.to_owned(),
            event_id: draft.event_id,
            signer_device_id,
            key_epoch,
            sequence: draft.sequence,
            previous_event_digest: draft.previous_event_digest,
            supersedes_event_digest: draft.supersedes_event_digest,
            occurred_at: draft.occurred_at,
            conversation_reference: draft.conversation_reference,
            fact: draft.fact,
            digest,
        };
        validate_sequence(&connection, &event)?;
        let event_bytes = serde_json::to_vec(&event)?;
        let protected_event = self
            .trusted_device
            .protect_household_state(household_id, &event_bytes)?;
        let unsigned = UnsignedPortableEnvelope {
            version: PORTABLE_MEMORY_VERSION,
            signer_device_id: event.signer_device_id.clone(),
            key_epoch,
            sequence: event.sequence,
            previous_event_digest: event.previous_event_digest.clone(),
            event_digest: event.digest.clone(),
            protected_event,
        };
        let unsigned_bytes = serde_json::to_vec(&unsigned)?;
        let signature = self
            .trusted_device
            .sign_portable_memory_record(household_id, &unsigned_bytes)?;
        let envelope = PortableEnvelope {
            version: unsigned.version,
            signer_device_id: unsigned.signer_device_id,
            key_epoch: unsigned.key_epoch,
            sequence: unsigned.sequence,
            previous_event_digest: unsigned.previous_event_digest,
            event_digest: unsigned.event_digest,
            protected_event: unsigned.protected_event,
            signature: BASE64.encode(signature),
        };
        let record = serde_json::to_vec(&envelope)?;
        let transaction = connection.transaction()?;
        store_local_event(
            &transaction,
            &event,
            &envelope.protected_event,
            Some(&record),
        )?;
        advance_subject_head(&transaction, &event, false)?;
        transaction.commit()?;
        let path = event_area(cabinet_root.as_ref())?.join(record_name);
        append_create_only(&path, &record)?;
        Ok(event)
    }

    pub fn import(
        &self,
        household_id: &str,
        cabinet_root: impl AsRef<Path>,
        trusted_devices: &[TrustedDeviceAuthorization],
    ) -> Result<PortableImportReport, PortableMemoryError> {
        let cabinet_root = cabinet_root.as_ref();
        if !cabinet_root.is_dir() {
            return Err(PortableMemoryError::CabinetUnavailable);
        }
        let event_area = event_area(cabinet_root)?;
        self.deliver_local_records(household_id, &event_area)?;
        let trusted_devices = trusted_devices
            .iter()
            .map(|device| (device.device_id.as_str(), device))
            .collect::<HashMap<_, _>>();
        let mut paths = fs::read_dir(event_area)?
            .map(|entry| entry.map(|value| value.path()))
            .collect::<Result<Vec<_>, _>>()?;
        paths.sort();

        let mut records = Vec::new();
        for path in paths {
            if path.extension().and_then(|value| value.to_str()) != Some(PORTABLE_EVENT_EXTENSION) {
                continue;
            }
            let record = fs::read(path)?;
            let envelope: PortableEnvelope =
                serde_json::from_slice(&record).map_err(|_| PortableMemoryError::RejectedRecord)?;
            let authorization = trusted_devices
                .get(envelope.signer_device_id.as_str())
                .ok_or(PortableMemoryError::UntrustedDevice)?;
            verify_envelope(&envelope, &authorization.authorization_public_key)?;
            verify_envelope_authorization_window(&envelope, authorization)?;
            records.push((envelope, record));
        }
        verify_envelope_chains(&self.connect()?, household_id, &records)?;
        for authorization in trusted_devices.values() {
            if let Some(cutoff) = &authorization.revoked_after {
                let cutoff_is_present = records.iter().any(|(envelope, _)| {
                    envelope.signer_device_id == authorization.device_id
                        && envelope.sequence == cutoff.sequence
                        && envelope.event_digest == cutoff.event_digest
                }) || local_event_digest(
                    &self.connect()?,
                    household_id,
                    &authorization.device_id,
                    cutoff.sequence,
                )?
                .as_deref()
                    == Some(cutoff.event_digest.as_str());
                if !cutoff_is_present {
                    return Err(PortableMemoryError::RejectedRecord);
                }
            }
        }

        let mut events = Vec::new();
        for (envelope, record) in records {
            let plaintext = self
                .trusted_device
                .open_household_state_at_epoch(
                    household_id,
                    envelope.key_epoch,
                    &envelope.protected_event,
                )
                .map_err(|_| PortableMemoryError::RejectedRecord)?;
            let event: PortableEvent = serde_json::from_slice(&plaintext)
                .map_err(|_| PortableMemoryError::RejectedRecord)?;
            verify_event(household_id, &event, &envelope)?;
            events.push((event, envelope.protected_event, record));
        }

        let mut report = PortableImportReport::default();
        while !events.is_empty() {
            let mut made_progress = false;
            let mut index = 0;
            while index < events.len() {
                if event_dependencies_available(&self.connect()?, &events[index].0)? {
                    let (event, protected, record) = events.remove(index);
                    match self.import_event(&event, &protected, &record)? {
                        ImportOutcome::Imported(conflict) => {
                            report.imported += 1;
                            if let Some(conflict) = conflict {
                                report.conflicts.push(conflict);
                            }
                        }
                        ImportOutcome::Duplicate => report.duplicates += 1,
                    }
                    made_progress = true;
                } else {
                    index += 1;
                }
            }
            if !made_progress {
                return Err(PortableMemoryError::RejectedRecord);
            }
        }
        report.conflicts = self.list_conflicts(household_id)?;
        Ok(report)
    }

    pub fn capture_owned_state(
        &self,
        household_id: &str,
        cabinet_root: impl AsRef<Path>,
        conversations: &ConversationStore<V>,
        intelligence: &CloudIntelligenceStore<V>,
    ) -> Result<(), PortableMemoryError> {
        let cabinet_root = cabinet_root.as_ref();
        let mut cabinet_unavailable = false;

        for rule in conversations.list_filing_rules(household_id)? {
            let rule_reference =
                self.entity_reference(household_id, "filing-rule", rule.id, "filing-rule")?;
            let taught_by = PortableReference::new(rule.teacher.clone()).or_else(|_| {
                deterministic_reference("subject", household_id, "member", &rule.teacher)
            })?;
            let fact = PortableFact::FilingRule {
                rule_reference: rule_reference.clone(),
                state: if rule.deleted {
                    PortableFilingRuleState::Deleted
                } else if rule.paused {
                    PortableFilingRuleState::Paused
                } else {
                    PortableFilingRuleState::Active
                },
                definition: PortableFilingRuleDefinition {
                    document_type: rule.document_type.clone(),
                    service_provider: rule.service_provider.clone(),
                    addressee: rule.addressee.clone(),
                    property: rule.property.clone(),
                    account: rule.account.clone(),
                    file_name: rule.file_name.clone(),
                    cabinet_destination: rule.cabinet_destination.clone(),
                    taught_by: taught_by.clone(),
                    created_at: if rule.created_at.is_empty() {
                        portable_now()
                    } else {
                        rule.created_at.clone()
                    },
                },
            };
            cabinet_unavailable |= self.capture_fact(
                household_id,
                cabinet_root,
                "filing-rule",
                &rule.id.to_string(),
                fact,
                None,
                true,
            )?;
        }

        for arrival in conversations.list_document_arrivals(household_id)? {
            let document_reference =
                self.entity_reference(household_id, "document", arrival.id, "document")?;
            let conversation_reference =
                self.conversation_reference(household_id, conversations, &arrival)?;
            if arrival.review_card.filing_decision.is_some()
                || arrival.duplicate_resolution.is_some()
            {
                let direction = match arrival
                    .duplicate_resolution
                    .as_ref()
                    .map(|resolution| resolution.decision)
                {
                    Some(DuplicateDecision::KeepBoth) => PortableMemberDirectionKind::KeepBoth,
                    Some(DuplicateDecision::UpdatedVersion) => {
                        PortableMemberDirectionKind::ReplaceExisting
                    }
                    _ => PortableMemberDirectionKind::FileDocument,
                };
                cabinet_unavailable |= self.capture_fact(
                    household_id,
                    cabinet_root,
                    "member-direction",
                    &arrival.id.to_string(),
                    PortableFact::MemberDirection {
                        direction,
                        subject_reference: deterministic_reference(
                            "subject",
                            household_id,
                            "document-direction",
                            document_reference.as_str(),
                        )?,
                    },
                    conversation_reference.clone(),
                    false,
                )?;
            }
            if let Some(resolution) = &arrival.duplicate_resolution {
                let related_document_reference = self.entity_reference(
                    household_id,
                    "document",
                    resolution.related_arrival_id,
                    "document",
                )?;
                let relationship = match resolution.decision {
                    DuplicateDecision::UpdatedVersion => {
                        Some(PortableDocumentRelationshipKind::UpdatedVersion)
                    }
                    DuplicateDecision::LinkCopies => {
                        Some(PortableDocumentRelationshipKind::LinkedCopy)
                    }
                    DuplicateDecision::KeepBoth | DuplicateDecision::DiscardNew => {
                        (resolution.duplicate_kind == Some(DuplicateKind::Exact))
                            .then_some(PortableDocumentRelationshipKind::ExactDuplicate)
                    }
                };
                if let Some(relationship) = relationship {
                    cabinet_unavailable |= self.capture_fact(
                        household_id,
                        cabinet_root,
                        "document-relationship",
                        &arrival.id.to_string(),
                        PortableFact::DocumentRelationship {
                            document_reference: document_reference.clone(),
                            related_document_reference,
                            relationship,
                        },
                        conversation_reference.clone(),
                        true,
                    )?;
                }
            }
            let outcome = match arrival.processing_state {
                DocumentProcessingState::Filed => {
                    Some(PortableExecutionOutcomeKind::FiledAndVerified)
                }
                DocumentProcessingState::CabinetUnavailable => {
                    Some(PortableExecutionOutcomeKind::CabinetUnavailable)
                }
                DocumentProcessingState::WaitingForCloudAssistance => {
                    Some(PortableExecutionOutcomeKind::WaitingForCloudAssistance)
                }
                _ => None,
            };
            if let Some(outcome) = outcome {
                cabinet_unavailable |= self.capture_fact(
                    household_id,
                    cabinet_root,
                    "document-outcome",
                    &arrival.id.to_string(),
                    PortableFact::ExecutionOutcome {
                        subject_reference: deterministic_reference(
                            "subject",
                            household_id,
                            "document-outcome",
                            document_reference.as_str(),
                        )?,
                        outcome,
                    },
                    conversation_reference,
                    true,
                )?;
            }
        }

        for event in conversations.list_audit_events(household_id)? {
            let subject_reference = self.entity_reference(
                household_id,
                "document",
                event.filed_original.arrival_id,
                "document",
            )?;
            cabinet_unavailable |= self.capture_fact(
                household_id,
                cabinet_root,
                "filing-audit",
                &event.id.to_string(),
                PortableFact::AuditEvent {
                    event_kind: match event.kind {
                        AuditEventKind::DocumentFiled => PortableAuditEventKind::DocumentFiled,
                        AuditEventKind::ExactMatchHandledAutomatically => {
                            PortableAuditEventKind::ExactMatchHandledAutomatically
                        }
                    },
                    authority: match event.authority {
                        ConversationAuditAuthority::MemberDirection => {
                            PortableAuthority::MemberDirection
                        }
                        ConversationAuditAuthority::FilingRule => PortableAuthority::FilingRule,
                    },
                    subject_reference,
                    outcome: PortableExecutionOutcomeKind::FiledAndVerified,
                    candidate_disposition: None,
                },
                None,
                false,
            )?;
        }

        for event in conversations.list_filing_rule_audit_events(household_id)? {
            let subject_reference =
                self.entity_reference(household_id, "filing-rule", event.rule_id, "filing-rule")?;
            cabinet_unavailable |= self.capture_fact(
                household_id,
                cabinet_root,
                "filing-rule-audit",
                &event.id.to_string(),
                PortableFact::AuditEvent {
                    event_kind: PortableAuditEventKind::FilingRuleChanged,
                    authority: PortableAuthority::MemberDirection,
                    subject_reference,
                    outcome: PortableExecutionOutcomeKind::FilingRuleChanged,
                    candidate_disposition: None,
                },
                None,
                false,
            )?;
        }

        for export in intelligence.portable_consent_exports(household_id)? {
            let consent = export.scope;
            let grant_reference =
                self.entity_reference(household_id, "consent", consent.id, "grant")?;
            let document_reference = consent
                .document_arrival_id
                .as_deref()
                .map(|arrival_id| {
                    let local_id = arrival_id
                        .strip_prefix("arrival-")
                        .and_then(|value| value.parse::<i64>().ok())
                        .filter(|value| *value > 0)
                        .ok_or(PortableMemoryError::InvalidEvent)?;
                    self.entity_reference(household_id, "document", local_id, "document")
                })
                .transpose()?;
            let fields = consent
                .fields
                .iter()
                .map(|field| portable_consent_field(field))
                .collect::<Result<Vec<_>, _>>()?;
            cabinet_unavailable |= self.capture_fact(
                household_id,
                cabinet_root,
                "consent",
                &consent.id.to_string(),
                PortableFact::ConsentGrant {
                    grant_reference,
                    provider: portable_provider(&consent.provider_id)?,
                    scope: PortableConsentScope {
                        document_type: None,
                        document_reference,
                        future_scope: consent.future_scope,
                        future_scope_evidence: export
                            .future_scope_evidence
                            .into_iter()
                            .map(|evidence| PortableConsentScopeEvidence {
                                field: evidence.field,
                                value: evidence.value,
                            })
                            .collect(),
                        fields,
                    },
                    state: if consent.revoked {
                        PortableConsentState::Revoked
                    } else if consent.kind == ConsentGrantKind::OneTime {
                        PortableConsentState::AllowedOnce
                    } else {
                        PortableConsentState::Granted
                    },
                    details: PortableConsentDetails {
                        model_id: consent.model_id,
                        capability: match consent.capability {
                            IntelligenceCapability::DirectionInterpretation => {
                                PortableIntelligenceCapability::DirectionInterpretation
                            }
                            IntelligenceCapability::ConversationReply => {
                                PortableIntelligenceCapability::ConversationReply
                            }
                        },
                        purpose: match consent.capability {
                            IntelligenceCapability::DirectionInterpretation => {
                                PortableConsentPurpose::DocumentEvaluation
                            }
                            IntelligenceCapability::ConversationReply => {
                                PortableConsentPurpose::ConversationReply
                            }
                        },
                        kind: match consent.kind {
                            ConsentGrantKind::OneTime => PortableConsentGrantKind::OneTime,
                            ConsentGrantKind::Reusable => PortableConsentGrantKind::Reusable,
                        },
                        granted_by: deterministic_reference(
                            "subject",
                            household_id,
                            "member",
                            &consent.granted_by,
                        )?,
                        created_at: consent.created_at,
                        consumed_at: consent.consumed_at,
                        revoked_at: consent.revoked_at,
                    },
                },
                None,
                true,
            )?;
        }

        for event in intelligence.list_audit_events(household_id)? {
            let subject_reference = deterministic_reference(
                "subject",
                household_id,
                "cloud-request",
                &event.request_id,
            )?;
            let outcome = match event.outcome {
                CloudAssistanceOutcome::Completed => {
                    PortableExecutionOutcomeKind::CloudAssistanceCompleted
                }
                CloudAssistanceOutcome::Denied => PortableExecutionOutcomeKind::KeptLocal,
                CloudAssistanceOutcome::WaitingForRetry => {
                    PortableExecutionOutcomeKind::ProviderUnavailable
                }
                CloudAssistanceOutcome::Cancelled => PortableExecutionOutcomeKind::Failed,
            };
            cabinet_unavailable |= self.capture_fact(
                household_id,
                cabinet_root,
                "cloud-outcome",
                &event.id.to_string(),
                PortableFact::ExecutionOutcome {
                    subject_reference: subject_reference.clone(),
                    outcome: outcome.clone(),
                },
                None,
                false,
            )?;
            cabinet_unavailable |= self.capture_fact(
                household_id,
                cabinet_root,
                "cloud-audit",
                &event.id.to_string(),
                PortableFact::AuditEvent {
                    event_kind: PortableAuditEventKind::ExecutionCompleted,
                    authority: if matches!(
                        event.outcome,
                        CloudAssistanceOutcome::Denied | CloudAssistanceOutcome::Cancelled
                    ) || matches!(
                        event.candidate_disposition,
                        CandidateDisposition::Accepted | CandidateDisposition::Corrected
                    ) {
                        PortableAuthority::MemberDirection
                    } else {
                        PortableAuthority::ConsentGrant
                    },
                    subject_reference,
                    outcome,
                    candidate_disposition: Some(match event.candidate_disposition {
                        CandidateDisposition::Pending => PortableCandidateDisposition::Pending,
                        CandidateDisposition::Accepted => PortableCandidateDisposition::Accepted,
                        CandidateDisposition::Corrected => PortableCandidateDisposition::Corrected,
                        CandidateDisposition::Rejected => PortableCandidateDisposition::Rejected,
                    }),
                },
                None,
                false,
            )?;
        }

        if cabinet_unavailable {
            return Err(PortableMemoryError::CabinetUnavailable);
        }
        Ok(())
    }

    pub fn synchronize_owned_state(
        &self,
        household_id: &str,
        cabinet_root: impl AsRef<Path>,
        trusted_devices: &[TrustedDeviceAuthorization],
        conversations: &ConversationStore<V>,
        intelligence: &CloudIntelligenceStore<V>,
    ) -> Result<PortableImportReport, PortableMemoryError> {
        let cabinet_root = cabinet_root.as_ref();
        let report = self.import(household_id, cabinet_root, trusted_devices)?;
        self.rebuild_owned_state(household_id, conversations, intelligence)?;
        match self.capture_owned_state(household_id, cabinet_root, conversations, intelligence) {
            Ok(()) | Err(PortableMemoryError::CabinetUnavailable) => {}
            Err(error) => return Err(error),
        }
        Ok(report)
    }

    pub fn list_portable_history(
        &self,
        household_id: &str,
    ) -> Result<Vec<PortableHistoryEvent>, PortableMemoryError> {
        Ok(self
            .household_projection(household_id)?
            .audit_events
            .into_iter()
            .filter_map(|event| match event.fact {
                PortableFact::AuditEvent {
                    event_kind,
                    authority,
                    subject_reference,
                    outcome,
                    candidate_disposition,
                } => Some(PortableHistoryEvent {
                    event_id: event.event_id,
                    occurred_at: event.occurred_at,
                    event_kind,
                    authority,
                    subject_reference,
                    outcome,
                    candidate_disposition,
                }),
                _ => None,
            })
            .collect())
    }

    pub fn authorization_cutoff(
        &self,
        household_id: &str,
        device_id: &str,
    ) -> Result<Option<PortableAuthorizationCutoff>, PortableMemoryError> {
        Ok(self
            .connect()?
            .query_row(
                "SELECT key_epoch, sequence, event_digest FROM portable_events
                  WHERE household_id = ?1 AND signer_device_id = ?2
                  ORDER BY sequence DESC LIMIT 1",
                params![household_id, device_id],
                |row| {
                    Ok(PortableAuthorizationCutoff {
                        key_epoch: row.get(0)?,
                        sequence: row.get(1)?,
                        event_digest: row.get(2)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn list_events(
        &self,
        household_id: &str,
    ) -> Result<Vec<PortableEvent>, PortableMemoryError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT key_epoch, protected_payload FROM portable_events
              WHERE household_id = ?1 ORDER BY signer_device_id, sequence",
        )?;
        let protected = statement
            .query_map(params![household_id], |row| {
                Ok((row.get::<_, u32>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        protected
            .into_iter()
            .map(|(key_epoch, value)| {
                let protected: ProtectedHouseholdState = serde_json::from_str(&value)?;
                let plaintext = self.trusted_device.open_household_state_at_epoch(
                    household_id,
                    key_epoch,
                    &protected,
                )?;
                Ok(serde_json::from_slice(&plaintext)?)
            })
            .collect()
    }

    pub fn list_conflicts(
        &self,
        household_id: &str,
    ) -> Result<Vec<PortableConflict>, PortableMemoryError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT id, subject_reference, existing_event_id, conflicting_event_id
               FROM portable_conflicts
              WHERE household_id = ?1 AND resolved_event_id IS NULL
              ORDER BY id",
        )?;
        let conflicts = statement
            .query_map(params![household_id], |row| {
                Ok(PortableConflict {
                    id: row.get(0)?,
                    subject_reference: row.get(1)?,
                    existing_event_id: row.get(2)?,
                    conflicting_event_id: row.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(conflicts)
    }

    pub fn resolve_conflict(
        &self,
        household_id: &str,
        cabinet_root: impl AsRef<Path>,
        conflict_id: i64,
        draft: PortableConflictResolutionDraft,
    ) -> Result<PortableEvent, PortableMemoryError> {
        let connection = self.connect()?;
        let conflict: Option<(String, String)> = connection
            .query_row(
                "SELECT existing_event_id, conflicting_event_id
                   FROM portable_conflicts
                  WHERE household_id = ?1 AND id = ?2 AND resolved_event_id IS NULL",
                params![household_id, conflict_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let (existing_event_id, conflicting_event_id) =
            conflict.ok_or(PortableMemoryError::InvalidEvent)?;
        if draft.chosen_event_id != existing_event_id
            && draft.chosen_event_id != conflicting_event_id
        {
            return Err(PortableMemoryError::InvalidEvent);
        }
        self.append(
            household_id,
            cabinet_root,
            PortableEventDraft {
                event_id: draft.event_id,
                sequence: draft.sequence,
                previous_event_digest: draft.previous_event_digest,
                supersedes_event_digest: None,
                occurred_at: draft.occurred_at,
                conversation_reference: None,
                fact: PortableFact::ConflictResolution {
                    existing_event_id: PortableReference::new(existing_event_id)?,
                    conflicting_event_id: PortableReference::new(conflicting_event_id)?,
                    chosen_event_id: PortableReference::new(draft.chosen_event_id)?,
                },
            },
        )
    }

    pub fn current_subject_event(
        &self,
        household_id: &str,
        subject_reference: &str,
    ) -> Result<Option<PortableEvent>, PortableMemoryError> {
        let conflicts = self.list_conflicts(household_id)?;
        if self
            .conflicted_projection_subjects(household_id, &conflicts)?
            .contains(subject_reference)
        {
            return Ok(None);
        }
        let connection = self.connect()?;
        let protected: Option<(u32, String)> = connection
            .query_row(
                "SELECT events.key_epoch, events.protected_payload
                   FROM portable_subject_heads AS heads
                   JOIN portable_events AS events
                     ON events.household_id = heads.household_id
                    AND events.event_id = heads.event_id
                  WHERE heads.household_id = ?1 AND heads.subject_reference = ?2",
                params![household_id, subject_reference],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        protected
            .map(|(key_epoch, value)| {
                let protected: ProtectedHouseholdState = serde_json::from_str(&value)?;
                let plaintext = self.trusted_device.open_household_state_at_epoch(
                    household_id,
                    key_epoch,
                    &protected,
                )?;
                Ok(serde_json::from_slice(&plaintext)?)
            })
            .transpose()
    }

    pub fn household_projection(
        &self,
        household_id: &str,
    ) -> Result<PortableHouseholdProjection, PortableMemoryError> {
        let events = self.list_events(household_id)?;
        let conflicts = self.list_conflicts(household_id)?;
        let conflicted_subjects = self.conflicted_projection_subjects(household_id, &conflicts)?;
        let heads = self.subject_head_event_ids(household_id)?;
        let is_current = |event: &PortableEvent| {
            portable_subject_reference(&event.fact).is_some_and(|subject| {
                !conflicted_subjects.contains(&subject)
                    && heads.get(&subject) == Some(&event.event_id)
            })
        };
        let mut projection = PortableHouseholdProjection {
            conflicts,
            ..PortableHouseholdProjection::default()
        };
        for event in events {
            match &event.fact {
                PortableFact::FilingRule { .. } if is_current(&event) => {
                    projection.filing_rules.push(event)
                }
                PortableFact::DocumentRelationship { .. } if is_current(&event) => {
                    projection.document_relationships.push(event)
                }
                PortableFact::AuthorityGrant { .. } if is_current(&event) => {
                    projection.authority_grants.push(event)
                }
                PortableFact::ConsentGrant { .. } if is_current(&event) => {
                    projection.consent_grants.push(event)
                }
                PortableFact::MemberDirection { .. } => projection.member_directions.push(event),
                PortableFact::ExecutionOutcome { .. } => projection.execution_outcomes.push(event),
                PortableFact::AuditEvent { .. } => projection.audit_events.push(event),
                PortableFact::FilingRule { .. }
                | PortableFact::DocumentRelationship { .. }
                | PortableFact::AuthorityGrant { .. }
                | PortableFact::ConsentGrant { .. }
                | PortableFact::ConflictResolution { .. } => {}
            }
        }
        Ok(projection)
    }

    fn import_event(
        &self,
        event: &PortableEvent,
        protected: &ProtectedHouseholdState,
        record: &[u8],
    ) -> Result<ImportOutcome, PortableMemoryError> {
        let mut connection = self.connect()?;
        let existing: Option<String> = connection
            .query_row(
                "SELECT event_digest FROM portable_events
                  WHERE household_id = ?1 AND event_id = ?2",
                params![event.household_id, event.event_id],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(existing) = existing {
            return if existing == event.digest {
                Ok(ImportOutcome::Duplicate)
            } else {
                Err(PortableMemoryError::RejectedRecord)
            };
        }
        validate_sequence(&connection, event)?;
        let transaction = connection.transaction()?;
        store_local_event(&transaction, event, protected, Some(record))?;
        let conflict = advance_subject_head(&transaction, event, true)?;
        transaction.commit()?;
        Ok(ImportOutcome::Imported(conflict))
    }

    #[allow(clippy::too_many_arguments)]
    fn capture_fact(
        &self,
        household_id: &str,
        cabinet_root: &Path,
        source_kind: &str,
        source_id: &str,
        fact: PortableFact,
        conversation_reference: Option<PortableConversationReference>,
        mutable: bool,
    ) -> Result<bool, PortableMemoryError> {
        let fingerprint = sha256_hex(&serde_json::to_vec(&(&fact, &conversation_reference))?);
        let connection = self.connect()?;
        let exported: Option<(String, String, String)> = connection
            .query_row(
                "SELECT fingerprint, event_id, event_digest
                   FROM portable_domain_exports
                  WHERE household_id = ?1 AND source_kind = ?2 AND source_id = ?3",
                params![household_id, source_kind, source_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        if exported
            .as_ref()
            .is_some_and(|(existing, _, _)| existing == &fingerprint)
        {
            return Ok(false);
        }
        if exported.is_none() && mutable {
            if let Some(subject_reference) = portable_subject_reference(&fact) {
                if let Some(current) =
                    self.current_subject_event(household_id, &subject_reference)?
                {
                    if current.fact == fact {
                        self.save_export(
                            household_id,
                            source_kind,
                            source_id,
                            &fingerprint,
                            &current,
                        )?;
                        return Ok(false);
                    }
                }
            }
        }
        let event_id = deterministic_reference(
            "event",
            household_id,
            source_kind,
            &format!("{source_id}:{fingerprint}"),
        )?
        .to_string();
        if let Some(event) = self.event_by_id(household_id, &event_id)? {
            if event.fact != fact || event.conversation_reference != conversation_reference {
                return Err(PortableMemoryError::RecordConflict);
            }
            self.save_export(household_id, source_kind, source_id, &fingerprint, &event)?;
            return Ok(false);
        }
        let signer_device_id = self
            .trusted_device
            .current_device_public_key(household_id)?;
        let previous: Option<(u64, String)> = connection
            .query_row(
                "SELECT sequence, event_digest FROM portable_events
                  WHERE household_id = ?1 AND signer_device_id = ?2
                  ORDER BY sequence DESC LIMIT 1",
                params![household_id, signer_device_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let sequence = previous.as_ref().map_or(1, |(sequence, _)| sequence + 1);
        let previous_event_digest = previous.map(|(_, digest)| digest);
        let supersedes_event_digest = mutable
            .then(|| {
                exported
                    .as_ref()
                    .map(|(_, _, event_digest)| event_digest.clone())
            })
            .flatten();
        let draft = PortableEventDraft {
            event_id: event_id.clone(),
            sequence,
            previous_event_digest,
            supersedes_event_digest,
            occurred_at: portable_now(),
            conversation_reference,
            fact,
        };
        let (event, cabinet_unavailable) = match self.append(household_id, cabinet_root, draft) {
            Ok(event) => (event, false),
            Err(PortableMemoryError::CabinetUnavailable) => (
                self.event_by_id(household_id, &event_id)?
                    .ok_or(PortableMemoryError::Storage(
                        rusqlite::Error::QueryReturnedNoRows,
                    ))?,
                true,
            ),
            Err(error) => return Err(error),
        };
        self.save_export(household_id, source_kind, source_id, &fingerprint, &event)?;
        Ok(cabinet_unavailable)
    }

    fn save_export(
        &self,
        household_id: &str,
        source_kind: &str,
        source_id: &str,
        fingerprint: &str,
        event: &PortableEvent,
    ) -> Result<(), PortableMemoryError> {
        self.connect()?.execute(
            "INSERT INTO portable_domain_exports (
                household_id, source_kind, source_id, fingerprint, event_id, event_digest
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(household_id, source_kind, source_id) DO UPDATE SET
                fingerprint = excluded.fingerprint,
                event_id = excluded.event_id,
                event_digest = excluded.event_digest",
            params![
                household_id,
                source_kind,
                source_id,
                fingerprint,
                event.event_id,
                event.digest
            ],
        )?;
        Ok(())
    }

    fn event_by_id(
        &self,
        household_id: &str,
        event_id: &str,
    ) -> Result<Option<PortableEvent>, PortableMemoryError> {
        let protected: Option<(u32, String)> = self
            .connect()?
            .query_row(
                "SELECT key_epoch, protected_payload FROM portable_events
                  WHERE household_id = ?1 AND event_id = ?2",
                params![household_id, event_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        protected
            .map(|(key_epoch, protected)| {
                let protected: ProtectedHouseholdState = serde_json::from_str(&protected)?;
                let plaintext = self.trusted_device.open_household_state_at_epoch(
                    household_id,
                    key_epoch,
                    &protected,
                )?;
                Ok(serde_json::from_slice(&plaintext)?)
            })
            .transpose()
    }

    fn entity_reference(
        &self,
        household_id: &str,
        entity_kind: &str,
        local_id: i64,
        reference_kind: &str,
    ) -> Result<PortableReference, PortableMemoryError> {
        let connection = self.connect()?;
        let existing: Option<String> = connection
            .query_row(
                "SELECT portable_reference FROM portable_entity_bindings
                  WHERE household_id = ?1 AND entity_kind = ?2 AND local_id = ?3",
                params![household_id, entity_kind, local_id],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(existing) = existing {
            return PortableReference::new(existing);
        }
        let device_id = self
            .trusted_device
            .current_device_public_key(household_id)?;
        let reference = deterministic_reference(
            reference_kind,
            household_id,
            entity_kind,
            &format!("{device_id}:{local_id}"),
        )?;
        connection.execute(
            "INSERT INTO portable_entity_bindings (
                household_id, entity_kind, local_id, portable_reference
             ) VALUES (?1, ?2, ?3, ?4)",
            params![household_id, entity_kind, local_id, reference.as_str()],
        )?;
        Ok(reference)
    }

    fn conversation_reference(
        &self,
        household_id: &str,
        conversations: &ConversationStore<V>,
        arrival: &DocumentArrival,
    ) -> Result<Option<PortableConversationReference>, PortableMemoryError> {
        let Some(message) = conversations
            .list_messages(household_id, arrival.conversation_id)?
            .last()
            .cloned()
        else {
            return Ok(None);
        };
        Ok(Some(PortableConversationReference {
            conversation_id: self.entity_reference(
                household_id,
                "conversation",
                arrival.conversation_id,
                "conversation",
            )?,
            message_id: self.entity_reference(household_id, "message", message.id, "message")?,
        }))
    }

    fn rebuild_owned_state(
        &self,
        household_id: &str,
        conversations: &ConversationStore<V>,
        intelligence: &CloudIntelligenceStore<V>,
    ) -> Result<(), PortableMemoryError> {
        let projection = self.household_projection(household_id)?;
        let mut visible_rule_ids = Vec::new();
        for event in &projection.filing_rules {
            let PortableFact::FilingRule {
                rule_reference,
                state,
                definition,
            } = &event.fact
            else {
                continue;
            };
            let local_id =
                self.bound_local_id(household_id, "filing-rule", rule_reference.as_str())?;
            let local_id = conversations.apply_portable_filing_rule(
                household_id,
                local_id,
                definition,
                state,
            )?;
            self.bind_imported_entity(
                household_id,
                "filing-rule",
                local_id,
                rule_reference.as_str(),
            )?;
            let fingerprint = sha256_hex(&serde_json::to_vec(&(
                &event.fact,
                &Option::<PortableConversationReference>::None,
            ))?);
            self.save_export(
                household_id,
                "filing-rule",
                &local_id.to_string(),
                &fingerprint,
                event,
            )?;
            visible_rule_ids.push(local_id);
        }
        let all_bound_rule_ids = self.bound_local_ids(household_id, "filing-rule")?;
        conversations.set_portable_filing_rule_visibility(
            household_id,
            &all_bound_rule_ids,
            &visible_rule_ids,
        )?;
        let relationships = projection
            .document_relationships
            .iter()
            .filter_map(|event| {
                let PortableFact::DocumentRelationship {
                    document_reference,
                    related_document_reference,
                    relationship,
                } = &event.fact
                else {
                    return None;
                };
                Some(RebuiltDocumentRelationship {
                    event_id: event.event_id.clone(),
                    document_reference: document_reference.clone(),
                    related_document_reference: related_document_reference.clone(),
                    relationship: relationship.clone(),
                })
            })
            .collect::<Vec<_>>();
        conversations.replace_rebuilt_document_relationships(household_id, &relationships)?;
        let history = projection
            .audit_events
            .iter()
            .filter_map(|event| {
                let PortableFact::AuditEvent {
                    event_kind,
                    authority,
                    subject_reference,
                    outcome,
                    candidate_disposition,
                } = &event.fact
                else {
                    return None;
                };
                Some(PortableHistoryEvent {
                    event_id: event.event_id.clone(),
                    occurred_at: event.occurred_at.clone(),
                    event_kind: event_kind.clone(),
                    authority: authority.clone(),
                    subject_reference: subject_reference.clone(),
                    outcome: outcome.clone(),
                    candidate_disposition: *candidate_disposition,
                })
            })
            .collect::<Vec<_>>();
        conversations.replace_rebuilt_portable_history(household_id, &history)?;
        for event in &projection.consent_grants {
            let PortableFact::ConsentGrant {
                grant_reference,
                provider,
                scope,
                state,
                details,
            } = &event.fact
            else {
                continue;
            };
            if *state == PortableConsentState::Denied {
                continue;
            }
            if details.kind == PortableConsentGrantKind::OneTime {
                continue;
            }
            let local_id =
                self.bound_local_id(household_id, "consent", grant_reference.as_str())?;
            let local_id = intelligence.apply_portable_consent(
                household_id,
                local_id,
                provider,
                scope,
                state,
                details,
            )?;
            self.bind_imported_entity(household_id, "consent", local_id, grant_reference.as_str())?;
            let fingerprint = sha256_hex(&serde_json::to_vec(&(
                &event.fact,
                &Option::<PortableConversationReference>::None,
            ))?);
            self.save_export(
                household_id,
                "consent",
                &local_id.to_string(),
                &fingerprint,
                event,
            )?;
        }
        Ok(())
    }

    fn bound_local_ids(
        &self,
        household_id: &str,
        entity_kind: &str,
    ) -> Result<Vec<i64>, PortableMemoryError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT local_id FROM portable_entity_bindings
              WHERE household_id = ?1 AND entity_kind = ?2",
        )?;
        let local_ids = statement
            .query_map(params![household_id, entity_kind], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(local_ids)
    }

    fn bound_local_id(
        &self,
        household_id: &str,
        entity_kind: &str,
        portable_reference: &str,
    ) -> Result<Option<i64>, PortableMemoryError> {
        Ok(self
            .connect()?
            .query_row(
                "SELECT local_id FROM portable_entity_bindings
                  WHERE household_id = ?1 AND entity_kind = ?2 AND portable_reference = ?3",
                params![household_id, entity_kind, portable_reference],
                |row| row.get(0),
            )
            .optional()?)
    }

    fn bind_imported_entity(
        &self,
        household_id: &str,
        entity_kind: &str,
        local_id: i64,
        portable_reference: &str,
    ) -> Result<(), PortableMemoryError> {
        self.connect()?.execute(
            "INSERT INTO portable_entity_bindings (
                household_id, entity_kind, local_id, portable_reference
             ) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(household_id, entity_kind, local_id) DO UPDATE SET
                portable_reference = excluded.portable_reference",
            params![household_id, entity_kind, local_id, portable_reference],
        )?;
        Ok(())
    }

    fn connect(&self) -> Result<Connection, PortableMemoryError> {
        Ok(Connection::open(&self.database)?)
    }

    fn subject_head_event_ids(
        &self,
        household_id: &str,
    ) -> Result<HashMap<String, String>, PortableMemoryError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT subject_reference, event_id FROM portable_subject_heads
              WHERE household_id = ?1",
        )?;
        let heads = statement
            .query_map(params![household_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<HashMap<_, _>, _>>()?;
        Ok(heads)
    }

    fn deliver_local_records(
        &self,
        household_id: &str,
        event_area: &Path,
    ) -> Result<(), PortableMemoryError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT event_id, portable_record FROM portable_events
              WHERE household_id = ?1 AND portable_record IS NOT NULL
              ORDER BY signer_device_id, sequence",
        )?;
        let records = statement
            .query_map(params![household_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        for (event_id, record) in records {
            let path = event_area.join(format!(
                "{}.{PORTABLE_EVENT_EXTENSION}",
                sha256_hex(event_id.as_bytes())
            ));
            append_create_only(&path, &record)?;
        }
        Ok(())
    }

    fn conflicted_projection_subjects(
        &self,
        household_id: &str,
        conflicts: &[PortableConflict],
    ) -> Result<HashSet<String>, PortableMemoryError> {
        let connection = self.connect()?;
        let mut subjects = HashSet::new();
        for conflict in conflicts {
            subjects.insert(conflict.subject_reference.clone());
            for resolution_event_id in [
                conflict.existing_event_id.as_str(),
                conflict.conflicting_event_id.as_str(),
            ] {
                let resolved_subject: Option<String> = connection
                    .query_row(
                        "SELECT subject_reference FROM portable_resolution_choices
                          WHERE household_id = ?1 AND resolution_event_id = ?2",
                        params![household_id, resolution_event_id],
                        |row| row.get(0),
                    )
                    .optional()?;
                if let Some(resolved_subject) = resolved_subject {
                    subjects.insert(resolved_subject);
                }
            }
        }
        Ok(subjects)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ImportOutcome {
    Imported(Option<PortableConflict>),
    Duplicate,
}

fn deterministic_reference(
    kind: &str,
    household_id: &str,
    namespace: &str,
    value: &str,
) -> Result<PortableReference, PortableMemoryError> {
    let digest = Sha256::digest(
        format!("luna:portable-reference:v1:{household_id}:{namespace}:{value}").as_bytes(),
    );
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    PortableReference::new(format!(
        "{kind}:{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        bytes[4],
        bytes[5],
        bytes[6],
        bytes[7],
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15]
    ))
}

fn portable_now() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_owned())
}

fn portable_provider(provider_id: &str) -> Result<PortableConsentProvider, PortableMemoryError> {
    match provider_id {
        "openai" | "luna-managed-openai" | "luna-managed" => {
            Ok(PortableConsentProvider::LunaManaged)
        }
        "openai-byok" => Ok(PortableConsentProvider::OpenAi),
        "anthropic" | "anthropic-byok" => Ok(PortableConsentProvider::Anthropic),
        _ => Err(PortableMemoryError::InvalidEvent),
    }
}

fn portable_consent_field(field: &str) -> Result<PortableConsentField, PortableMemoryError> {
    let normalized = match field
        .trim()
        .to_ascii_lowercase()
        .replace(['_', '-', ' '], "")
        .as_str()
    {
        "documenttype" => PortableConsentField::DocumentType,
        "serviceprovider" => PortableConsentField::ServiceProvider,
        "addressee" => PortableConsentField::Addressee,
        "property" => PortableConsentField::Property,
        "account" => PortableConsentField::Account,
        "amount" => PortableConsentField::Amount,
        "relevantdates" => PortableConsentField::RelevantDates,
        _ if portable_text(field, 256) => PortableConsentField::Additional(field.trim().to_owned()),
        _ => return Err(PortableMemoryError::SensitiveMaterial),
    };
    Ok(normalized)
}

fn portable_column_exists(
    connection: &Connection,
    column: &str,
) -> Result<bool, PortableMemoryError> {
    table_column_exists(connection, "portable_events", column)
}

fn table_column_exists(
    connection: &Connection,
    table: &str,
    column: &str,
) -> Result<bool, PortableMemoryError> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table})"))?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        if row.get::<_, String>(1)? == column {
            return Ok(true);
        }
    }
    Ok(false)
}

fn store_local_event(
    connection: &Connection,
    event: &PortableEvent,
    protected: &ProtectedHouseholdState,
    record: Option<&[u8]>,
) -> Result<(), PortableMemoryError> {
    let protected = serde_json::to_string(protected)?;
    let inserted = connection.execute(
        "INSERT OR IGNORE INTO portable_events (
            household_id, event_id, event_digest, signer_device_id, key_epoch, sequence,
            protected_payload, portable_record
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            event.household_id,
            event.event_id,
            event.digest,
            event.signer_device_id,
            event.key_epoch,
            event.sequence,
            protected,
            record
        ],
    )?;
    if inserted == 0 {
        let existing: Option<(String, String)> = connection
            .query_row(
                "SELECT event_id, event_digest FROM portable_events
                  WHERE household_id = ?1 AND (
                    event_id = ?2 OR (signer_device_id = ?3 AND sequence = ?4)
                  )",
                params![
                    event.household_id,
                    event.event_id,
                    event.signer_device_id,
                    event.sequence
                ],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        if !existing.is_some_and(|(id, digest)| id == event.event_id && digest == event.digest) {
            return Err(PortableMemoryError::RecordConflict);
        }
    }
    Ok(())
}

fn advance_subject_head(
    connection: &Connection,
    event: &PortableEvent,
    record_conflict: bool,
) -> Result<Option<PortableConflict>, PortableMemoryError> {
    if let PortableFact::ConflictResolution {
        existing_event_id,
        conflicting_event_id,
        chosen_event_id,
    } = &event.fact
    {
        return apply_conflict_resolution(
            connection,
            &event.household_id,
            existing_event_id.as_str(),
            conflicting_event_id.as_str(),
            chosen_event_id.as_str(),
            &event.event_id,
        );
    }
    let Some(subject_reference) = portable_subject_reference(&event.fact) else {
        return Ok(None);
    };
    let head: Option<(String, String)> = connection
        .query_row(
            "SELECT event_id, event_digest FROM portable_subject_heads
              WHERE household_id = ?1 AND subject_reference = ?2",
            params![event.household_id, subject_reference],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    match head {
        None if event.supersedes_event_digest.is_none() => {
            connection.execute(
                "INSERT INTO portable_subject_heads (
                    household_id, subject_reference, event_id, event_digest
                 ) VALUES (?1, ?2, ?3, ?4)",
                params![
                    event.household_id,
                    subject_reference,
                    event.event_id,
                    event.digest
                ],
            )?;
            Ok(None)
        }
        Some((_, head_digest))
            if event.supersedes_event_digest.as_deref() == Some(head_digest.as_str()) =>
        {
            connection.execute(
                "UPDATE portable_subject_heads SET event_id = ?1, event_digest = ?2
                  WHERE household_id = ?3 AND subject_reference = ?4",
                params![
                    event.event_id,
                    event.digest,
                    event.household_id,
                    subject_reference
                ],
            )?;
            Ok(None)
        }
        Some((existing_event_id, _)) if record_conflict => {
            let (first_event_id, second_event_id) =
                canonical_conflict_pair(&existing_event_id, &event.event_id);
            connection.execute(
                "INSERT OR IGNORE INTO portable_conflicts (
                    household_id, subject_reference, existing_event_id, conflicting_event_id
                 ) VALUES (?1, ?2, ?3, ?4)",
                params![
                    event.household_id,
                    subject_reference,
                    first_event_id,
                    second_event_id
                ],
            )?;
            let conflict = connection.query_row(
                "SELECT id, subject_reference, existing_event_id, conflicting_event_id
                   FROM portable_conflicts
                  WHERE household_id = ?1 AND subject_reference = ?2
                    AND existing_event_id = ?3 AND conflicting_event_id = ?4",
                params![
                    event.household_id,
                    subject_reference,
                    first_event_id,
                    second_event_id
                ],
                |row| {
                    Ok(PortableConflict {
                        id: row.get(0)?,
                        subject_reference: row.get(1)?,
                        existing_event_id: row.get(2)?,
                        conflicting_event_id: row.get(3)?,
                    })
                },
            )?;
            Ok(Some(conflict))
        }
        _ => Err(PortableMemoryError::RecordConflict),
    }
}

fn canonical_conflict_pair<'a>(left: &'a str, right: &'a str) -> (&'a str, &'a str) {
    if left <= right {
        (left, right)
    } else {
        (right, left)
    }
}

fn apply_conflict_resolution(
    connection: &Connection,
    household_id: &str,
    existing_event_id: &str,
    conflicting_event_id: &str,
    chosen_event_id: &str,
    resolution_event_id: &str,
) -> Result<Option<PortableConflict>, PortableMemoryError> {
    if chosen_event_id != existing_event_id && chosen_event_id != conflicting_event_id {
        return Err(PortableMemoryError::RejectedRecord);
    }
    let conflict: (i64, String, Option<String>, Option<String>) = connection
        .query_row(
            "SELECT id, subject_reference, resolved_event_id, resolution_event_id
               FROM portable_conflicts
              WHERE household_id = ?1
                AND existing_event_id = ?2 AND conflicting_event_id = ?3",
            params![household_id, existing_event_id, conflicting_event_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()?
        .ok_or(PortableMemoryError::RejectedRecord)?;
    let (conflict_id, subject_reference, resolved_event_id, first_resolution_event_id) = conflict;
    connection.execute(
        "INSERT OR REPLACE INTO portable_resolution_choices (
            household_id, resolution_event_id, conflict_id, subject_reference, chosen_event_id
         ) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            household_id,
            resolution_event_id,
            conflict_id,
            subject_reference,
            chosen_event_id
        ],
    )?;
    if let Some(resolved_event_id) = resolved_event_id {
        if resolved_event_id == chosen_event_id {
            return Ok(None);
        }
        let first_resolution_event_id =
            first_resolution_event_id.ok_or(PortableMemoryError::RejectedRecord)?;
        let meta_subject = format!("conflictResolution:{existing_event_id}:{conflicting_event_id}");
        let (first, second) =
            canonical_conflict_pair(&first_resolution_event_id, resolution_event_id);
        connection.execute(
            "INSERT OR IGNORE INTO portable_conflicts (
                household_id, subject_reference, existing_event_id, conflicting_event_id
             ) VALUES (?1, ?2, ?3, ?4)",
            params![household_id, meta_subject, first, second],
        )?;
        let conflict = connection.query_row(
            "SELECT id, subject_reference, existing_event_id, conflicting_event_id
               FROM portable_conflicts
              WHERE household_id = ?1 AND subject_reference = ?2
                AND existing_event_id = ?3 AND conflicting_event_id = ?4",
            params![household_id, meta_subject, first, second],
            |row| {
                Ok(PortableConflict {
                    id: row.get(0)?,
                    subject_reference: row.get(1)?,
                    existing_event_id: row.get(2)?,
                    conflicting_event_id: row.get(3)?,
                })
            },
        )?;
        return Ok(Some(conflict));
    }
    project_resolution_choice(
        connection,
        household_id,
        &subject_reference,
        chosen_event_id,
    )?;
    connection.execute(
        "UPDATE portable_conflicts
            SET resolved_event_id = ?1, resolution_event_id = ?2
          WHERE household_id = ?3 AND id = ?4",
        params![
            chosen_event_id,
            resolution_event_id,
            household_id,
            conflict_id
        ],
    )?;
    Ok(None)
}

fn project_resolution_choice(
    connection: &Connection,
    household_id: &str,
    subject_reference: &str,
    chosen_event_id: &str,
) -> Result<(), PortableMemoryError> {
    let nested: Option<(i64, String, String)> = connection
        .query_row(
            "SELECT conflict_id, subject_reference, chosen_event_id
               FROM portable_resolution_choices
              WHERE household_id = ?1 AND resolution_event_id = ?2",
            params![household_id, chosen_event_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    if let Some((nested_conflict_id, nested_subject, nested_choice)) = nested {
        connection.execute(
            "UPDATE portable_conflicts
                SET resolved_event_id = ?1, resolution_event_id = ?2
              WHERE household_id = ?3 AND id = ?4",
            params![
                nested_choice,
                chosen_event_id,
                household_id,
                nested_conflict_id
            ],
        )?;
        return project_resolution_choice(
            connection,
            household_id,
            &nested_subject,
            &nested_choice,
        );
    }
    let chosen_digest: String = connection
        .query_row(
            "SELECT event_digest FROM portable_events
              WHERE household_id = ?1 AND event_id = ?2",
            params![household_id, chosen_event_id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or(PortableMemoryError::RejectedRecord)?;
    connection.execute(
        "INSERT INTO portable_subject_heads (
            household_id, subject_reference, event_id, event_digest
         ) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(household_id, subject_reference) DO UPDATE SET
            event_id = excluded.event_id,
            event_digest = excluded.event_digest",
        params![
            household_id,
            subject_reference,
            chosen_event_id,
            chosen_digest
        ],
    )?;
    Ok(())
}

fn event_area(cabinet_root: &Path) -> Result<PathBuf, PortableMemoryError> {
    if !cabinet_root.is_dir() {
        return Err(PortableMemoryError::CabinetUnavailable);
    }
    let area = cabinet_root
        .join(PORTABLE_MEMORY_DIRECTORY)
        .join(format!("v{PORTABLE_MEMORY_VERSION}"))
        .join("events");
    fs::create_dir_all(&area)?;
    Ok(area)
}

fn validate_draft(draft: &PortableEventDraft) -> Result<(), PortableMemoryError> {
    let event_id = PortableReference::new(draft.event_id.clone())?;
    if event_id.kind() != "event"
        || !valid_conversation_reference(&draft.conversation_reference)
        || !valid_fact_reference_kinds(&draft.fact)
        || !valid_portable_timestamp(&draft.occurred_at)
        || draft.sequence == 0
        || (draft.sequence == 1 && draft.previous_event_digest.is_some())
        || (draft.sequence > 1 && draft.previous_event_digest.is_none())
        || !valid_optional_digest(&draft.previous_event_digest)
        || !valid_optional_digest(&draft.supersedes_event_digest)
    {
        return Err(PortableMemoryError::InvalidEvent);
    }
    Ok(())
}

fn valid_conversation_reference(reference: &Option<PortableConversationReference>) -> bool {
    reference.as_ref().is_none_or(|reference| {
        reference.conversation_id.kind() == "conversation"
            && reference.message_id.kind() == "message"
    })
}

fn valid_fact_reference_kinds(fact: &PortableFact) -> bool {
    match fact {
        PortableFact::FilingRule {
            rule_reference,
            definition,
            ..
        } => {
            rule_reference.kind() == "filing-rule"
                && definition.taught_by.kind() == "subject"
                && valid_filing_rule_definition(definition)
        }
        PortableFact::DocumentRelationship {
            document_reference,
            related_document_reference,
            ..
        } => {
            document_reference.kind() == "document"
                && related_document_reference.kind() == "document"
        }
        PortableFact::MemberDirection {
            subject_reference, ..
        } => subject_reference.kind() == "subject",
        PortableFact::AuthorityGrant {
            grant_reference,
            subject_reference,
            scope,
        } => {
            grant_reference.kind() == "grant"
                && subject_reference.kind() == "subject"
                && scope.iter().all(|reference| reference.kind() == "field")
        }
        PortableFact::ConsentGrant {
            grant_reference,
            scope,
            details,
            ..
        } => {
            grant_reference.kind() == "grant"
                && details.granted_by.kind() == "subject"
                && valid_portable_identifier(&details.model_id)
                && valid_portable_timestamp(&details.created_at)
                && details
                    .consumed_at
                    .as_deref()
                    .is_none_or(valid_portable_timestamp)
                && details
                    .revoked_at
                    .as_deref()
                    .is_none_or(valid_portable_timestamp)
                && scope
                    .document_type
                    .as_ref()
                    .is_none_or(|reference| reference.kind() == "document-type")
                && scope
                    .document_reference
                    .as_ref()
                    .is_none_or(|reference| reference.kind() == "document")
                && scope
                    .future_scope
                    .as_deref()
                    .is_none_or(|value| portable_text(value, 512))
                && scope.future_scope_evidence.len() <= 32
                && scope.future_scope_evidence.iter().all(|evidence| {
                    valid_portable_identifier(&evidence.field)
                        && portable_text(&evidence.value, 1024)
                })
                && scope.fields.iter().all(|field| match field {
                    PortableConsentField::Additional(field) => portable_text(field, 256),
                    _ => true,
                })
        }
        PortableFact::ExecutionOutcome {
            subject_reference, ..
        } => subject_reference.kind() == "subject",
        PortableFact::AuditEvent {
            event_kind,
            subject_reference,
            ..
        } => match event_kind {
            PortableAuditEventKind::DocumentFiled
            | PortableAuditEventKind::ExactMatchHandledAutomatically => {
                subject_reference.kind() == "document"
            }
            PortableAuditEventKind::FilingRuleChanged => subject_reference.kind() == "filing-rule",
            PortableAuditEventKind::ConsentChanged => subject_reference.kind() == "grant",
            PortableAuditEventKind::ExecutionCompleted => subject_reference.kind() == "subject",
        },
        PortableFact::ConflictResolution {
            existing_event_id,
            conflicting_event_id,
            chosen_event_id,
        } => {
            existing_event_id.kind() == "event"
                && conflicting_event_id.kind() == "event"
                && chosen_event_id.kind() == "event"
        }
    }
}

fn valid_filing_rule_definition(definition: &PortableFilingRuleDefinition) -> bool {
    let destination = Path::new(&definition.cabinet_destination);
    portable_text(&definition.document_type, 128)
        && portable_text(&definition.service_provider, 128)
        && portable_text(&definition.addressee, 128)
        && definition
            .property
            .as_deref()
            .is_none_or(|value| portable_text(value, 256))
        && definition
            .account
            .as_deref()
            .is_none_or(|value| portable_text(value, 128))
        && portable_text(&definition.file_name, 255)
        && !definition.file_name.contains(['/', '\\'])
        && portable_text(&definition.cabinet_destination, 1024)
        && destination.is_relative()
        && destination
            .components()
            .all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
        && destination
            .file_name()
            .is_some_and(|value| value == definition.file_name.as_str())
        && valid_portable_timestamp(&definition.created_at)
}

fn portable_text(value: &str, maximum_length: usize) -> bool {
    let value = value.trim();
    !value.is_empty()
        && value.len() <= maximum_length
        && !value.contains(['\0', '\r', '\n'])
        && !looks_like_sensitive_material(value)
}

fn valid_portable_identifier(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'/'))
        && !looks_like_sensitive_material(value)
}

fn looks_like_sensitive_material(value: &str) -> bool {
    let value = value.to_ascii_lowercase();
    [
        "bearer ",
        "api_key",
        "api-key",
        "private key",
        "begin private",
        "token=",
        "secret=",
        "sk-",
    ]
    .iter()
    .any(|marker| value.contains(marker))
}

fn event_matches_draft(event: &PortableEvent, draft: &PortableEventDraft) -> bool {
    event.event_id == draft.event_id
        && event.sequence == draft.sequence
        && event.previous_event_digest == draft.previous_event_digest
        && event.supersedes_event_digest == draft.supersedes_event_digest
        && event.occurred_at == draft.occurred_at
        && event.conversation_reference == draft.conversation_reference
        && event.fact == draft.fact
}

fn valid_portable_timestamp(value: &str) -> bool {
    let unix_seconds =
        (1..=20).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_digit());
    let iso_8601 = (20..=35).contains(&value.len())
        && value.contains('T')
        && value.bytes().all(|byte| {
            byte.is_ascii_digit() || matches!(byte, b'-' | b':' | b'.' | b'+' | b'T' | b'Z')
        });
    unix_seconds || iso_8601
}

fn valid_optional_digest(value: &Option<String>) -> bool {
    value.as_deref().is_none_or(|digest| {
        digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
    })
}

fn event_digest(event: &UnsignedPortableEvent<'_>) -> Result<String, PortableMemoryError> {
    let canonical = serde_json::to_vec(event)?;
    Ok(sha256_hex(&canonical))
}

fn verify_envelope(
    envelope: &PortableEnvelope,
    expected_key: &[u8; 32],
) -> Result<(), PortableMemoryError> {
    if envelope.version != PORTABLE_MEMORY_VERSION {
        return Err(PortableMemoryError::RejectedRecord);
    }
    let unsigned = UnsignedPortableEnvelope {
        version: envelope.version,
        signer_device_id: envelope.signer_device_id.clone(),
        key_epoch: envelope.key_epoch,
        sequence: envelope.sequence,
        previous_event_digest: envelope.previous_event_digest.clone(),
        event_digest: envelope.event_digest.clone(),
        protected_event: envelope.protected_event.clone(),
    };
    let signature_bytes: [u8; 64] = BASE64
        .decode(&envelope.signature)
        .map_err(|_| PortableMemoryError::RejectedRecord)?
        .try_into()
        .map_err(|_| PortableMemoryError::RejectedRecord)?;
    let verifier =
        VerifyingKey::from_bytes(expected_key).map_err(|_| PortableMemoryError::RejectedRecord)?;
    verifier
        .verify(
            &serde_json::to_vec(&unsigned)?,
            &Signature::from_bytes(&signature_bytes),
        )
        .map_err(|_| PortableMemoryError::RejectedRecord)
}

fn verify_event(
    household_id: &str,
    event: &PortableEvent,
    envelope: &PortableEnvelope,
) -> Result<(), PortableMemoryError> {
    if !matches!(
        PortableReference::new(event.event_id.clone()),
        Ok(reference) if reference.kind() == "event"
    ) || !valid_conversation_reference(&event.conversation_reference)
        || !valid_fact_reference_kinds(&event.fact)
        || !valid_portable_timestamp(&event.occurred_at)
        || event.sequence == 0
        || (event.sequence == 1 && event.previous_event_digest.is_some())
        || (event.sequence > 1 && event.previous_event_digest.is_none())
        || !valid_optional_digest(&event.previous_event_digest)
        || !valid_optional_digest(&event.supersedes_event_digest)
    {
        return Err(PortableMemoryError::RejectedRecord);
    }
    let expected_digest = event_digest(&UnsignedPortableEvent {
        household_id: &event.household_id,
        event_id: &event.event_id,
        signer_device_id: &event.signer_device_id,
        key_epoch: event.key_epoch,
        sequence: event.sequence,
        previous_event_digest: &event.previous_event_digest,
        supersedes_event_digest: &event.supersedes_event_digest,
        occurred_at: &event.occurred_at,
        conversation_reference: &event.conversation_reference,
        fact: &event.fact,
    })?;
    if event.household_id != household_id
        || event.signer_device_id != envelope.signer_device_id
        || event.key_epoch != envelope.key_epoch
        || event.sequence != envelope.sequence
        || event.previous_event_digest != envelope.previous_event_digest
        || event.digest != envelope.event_digest
        || event.digest != expected_digest
    {
        return Err(PortableMemoryError::RejectedRecord);
    }
    Ok(())
}

fn verify_envelope_authorization_window(
    envelope: &PortableEnvelope,
    authorization: &TrustedDeviceAuthorization,
) -> Result<(), PortableMemoryError> {
    if authorization.activated_key_epoch == 0
        || envelope.key_epoch < authorization.activated_key_epoch
    {
        return Err(PortableMemoryError::UntrustedDevice);
    }
    if let Some(cutoff) = &authorization.revoked_after {
        if cutoff.key_epoch == 0
            || cutoff.sequence == 0
            || !valid_optional_digest(&Some(cutoff.event_digest.clone()))
            || envelope.key_epoch > cutoff.key_epoch
            || envelope.sequence > cutoff.sequence
            || (envelope.sequence == cutoff.sequence
                && envelope.event_digest != cutoff.event_digest)
        {
            return Err(PortableMemoryError::UntrustedDevice);
        }
    }
    Ok(())
}

#[cfg(test)]
fn verify_authorization_window(
    event: &PortableEvent,
    authorization: &TrustedDeviceAuthorization,
) -> Result<(), PortableMemoryError> {
    if authorization.activated_key_epoch == 0 || event.key_epoch < authorization.activated_key_epoch
    {
        return Err(PortableMemoryError::UntrustedDevice);
    }
    if let Some(cutoff) = &authorization.revoked_after {
        if cutoff.key_epoch == 0
            || cutoff.sequence == 0
            || !valid_optional_digest(&Some(cutoff.event_digest.clone()))
            || event.key_epoch > cutoff.key_epoch
            || event.sequence > cutoff.sequence
            || (event.sequence == cutoff.sequence && event.digest != cutoff.event_digest)
        {
            return Err(PortableMemoryError::UntrustedDevice);
        }
    }
    Ok(())
}

fn verify_envelope_chains(
    connection: &Connection,
    household_id: &str,
    records: &[(PortableEnvelope, Vec<u8>)],
) -> Result<(), PortableMemoryError> {
    let mut incoming = HashMap::new();
    for (envelope, _) in records {
        if envelope.sequence == 0
            || (envelope.sequence == 1 && envelope.previous_event_digest.is_some())
            || (envelope.sequence > 1 && envelope.previous_event_digest.is_none())
            || !valid_optional_digest(&envelope.previous_event_digest)
            || !valid_optional_digest(&Some(envelope.event_digest.clone()))
        {
            return Err(PortableMemoryError::RejectedRecord);
        }
        let key = (envelope.signer_device_id.as_str(), envelope.sequence);
        if incoming
            .insert(key, envelope.event_digest.as_str())
            .is_some_and(|digest| digest != envelope.event_digest)
        {
            return Err(PortableMemoryError::RejectedRecord);
        }
    }
    for (envelope, _) in records {
        if envelope.sequence == 1 {
            continue;
        }
        let predecessor = match incoming.get(&(
            envelope.signer_device_id.as_str(),
            envelope.sequence.saturating_sub(1),
        )) {
            Some(digest) => Some((*digest).to_owned()),
            None => local_event_digest(
                connection,
                household_id,
                &envelope.signer_device_id,
                envelope.sequence - 1,
            )?,
        };
        if predecessor.as_deref() != envelope.previous_event_digest.as_deref() {
            return Err(PortableMemoryError::RejectedRecord);
        }
    }
    Ok(())
}

fn local_event_digest(
    connection: &Connection,
    household_id: &str,
    signer_device_id: &str,
    sequence: u64,
) -> Result<Option<String>, PortableMemoryError> {
    Ok(connection
        .query_row(
            "SELECT event_digest FROM portable_events
              WHERE household_id = ?1 AND signer_device_id = ?2 AND sequence = ?3",
            params![household_id, signer_device_id, sequence],
            |row| row.get(0),
        )
        .optional()?)
}

fn event_dependencies_available(
    connection: &Connection,
    event: &PortableEvent,
) -> Result<bool, PortableMemoryError> {
    let existing: Option<String> = connection
        .query_row(
            "SELECT event_digest FROM portable_events
              WHERE household_id = ?1 AND event_id = ?2",
            params![event.household_id, event.event_id],
            |row| row.get(0),
        )
        .optional()?;
    if let Some(existing) = existing {
        return Ok(existing == event.digest);
    }
    let predecessor_ready = match event.sequence {
        1 => event.previous_event_digest.is_none(),
        sequence => {
            connection
                .query_row(
                    "SELECT event_digest FROM portable_events
                  WHERE household_id = ?1 AND signer_device_id = ?2 AND sequence = ?3",
                    params![event.household_id, event.signer_device_id, sequence - 1],
                    |row| row.get::<_, String>(0),
                )
                .optional()?
                .as_deref()
                == event.previous_event_digest.as_deref()
        }
    };
    if !predecessor_ready {
        return Ok(false);
    }
    if let Some(supersedes) = &event.supersedes_event_digest {
        let dependency_exists: bool = connection.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM portable_events
                 WHERE household_id = ?1 AND event_digest = ?2
             )",
            params![event.household_id, supersedes],
            |row| row.get(0),
        )?;
        if !dependency_exists {
            return Ok(false);
        }
    }
    if let PortableFact::ConflictResolution {
        existing_event_id,
        conflicting_event_id,
        ..
    } = &event.fact
    {
        return Ok(connection.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM portable_conflicts
                 WHERE household_id = ?1
                   AND existing_event_id = ?2 AND conflicting_event_id = ?3
             )",
            params![
                event.household_id,
                existing_event_id.as_str(),
                conflicting_event_id.as_str()
            ],
            |row| row.get(0),
        )?);
    }
    Ok(true)
}

fn validate_sequence(
    connection: &Connection,
    event: &PortableEvent,
) -> Result<(), PortableMemoryError> {
    let head: Option<(u64, String, u32)> = connection
        .query_row(
            "SELECT sequence, event_digest, key_epoch FROM portable_events
              WHERE household_id = ?1 AND signer_device_id = ?2
              ORDER BY sequence DESC LIMIT 1",
            params![event.household_id, event.signer_device_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    match head {
        None if event.sequence == 1 && event.previous_event_digest.is_none() => Ok(()),
        Some((sequence, digest, key_epoch))
            if event.sequence == sequence + 1
                && event.previous_event_digest.as_deref() == Some(digest.as_str()) =>
        {
            if event.key_epoch < key_epoch {
                Err(PortableMemoryError::RejectedRecord)
            } else {
                Ok(())
            }
        }
        _ => Err(PortableMemoryError::RejectedRecord),
    }
}

fn append_create_only(path: &Path, record: &[u8]) -> Result<(), PortableMemoryError> {
    if path.is_file() {
        return if fs::read(path)? == record {
            Ok(())
        } else {
            Err(PortableMemoryError::RecordConflict)
        };
    }
    for attempt in 0..100_u8 {
        let temporary = path.with_extension(format!(
            "{PORTABLE_EVENT_EXTENSION}.tmp-{}-{attempt}",
            std::process::id()
        ));
        let mut file = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        };
        if let Err(error) = file.write_all(record).and_then(|_| file.sync_all()) {
            let _ = fs::remove_file(&temporary);
            return Err(error.into());
        }
        drop(file);
        match fs::hard_link(&temporary, path) {
            Ok(()) => {
                fs::remove_file(temporary)?;
                return Ok(());
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                fs::remove_file(temporary)?;
                return if fs::read(path)? == record {
                    Ok(())
                } else {
                    Err(PortableMemoryError::RecordConflict)
                };
            }
            Err(error) => {
                let _ = fs::remove_file(temporary);
                return Err(error.into());
            }
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "portable-memory temporary record namespace is exhausted",
    )
    .into())
}

fn sha256_hex(value: &[u8]) -> String {
    format!("{:x}", Sha256::digest(value))
}

fn portable_subject_reference(fact: &PortableFact) -> Option<String> {
    match fact {
        PortableFact::FilingRule { rule_reference, .. } => {
            Some(format!("filingRule:{rule_reference}"))
        }
        PortableFact::DocumentRelationship {
            document_reference,
            related_document_reference,
            ..
        } => Some(format!(
            "documentRelationship:{document_reference}:{related_document_reference}"
        )),
        PortableFact::AuthorityGrant {
            grant_reference, ..
        } => Some(format!("authorityGrant:{grant_reference}")),
        PortableFact::ConsentGrant {
            grant_reference, ..
        } => Some(format!("consentGrant:{grant_reference}")),
        PortableFact::MemberDirection { .. }
        | PortableFact::ExecutionOutcome { .. }
        | PortableFact::AuditEvent { .. }
        | PortableFact::ConflictResolution { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference(kind: &str, suffix: u8) -> PortableReference {
        PortableReference::new(format!("{kind}:00000000-0000-0000-0000-{suffix:012x}"))
            .expect("construct a typed test reference")
    }

    fn audit_event(key_epoch: u32, sequence: u64, digest: &str) -> PortableEvent {
        PortableEvent {
            household_id: "household".to_owned(),
            event_id: reference("event", sequence as u8).to_string(),
            signer_device_id: "device".to_owned(),
            key_epoch,
            sequence,
            previous_event_digest: (sequence > 1).then(|| "a".repeat(64)),
            supersedes_event_digest: None,
            occurred_at: "2026-07-24T18:00:00+10:00".to_owned(),
            conversation_reference: None,
            fact: PortableFact::AuditEvent {
                event_kind: PortableAuditEventKind::DocumentFiled,
                authority: PortableAuthority::MemberDirection,
                subject_reference: reference("document", 1),
                outcome: PortableExecutionOutcomeKind::FiledAndVerified,
                candidate_disposition: None,
            },
            digest: digest.to_owned(),
        }
    }

    fn filing_rule_definition() -> PortableFilingRuleDefinition {
        PortableFilingRuleDefinition {
            document_type: "Electricity bill".to_owned(),
            service_provider: "AGL".to_owned(),
            addressee: "Sam Rivera".to_owned(),
            property: Some("12 Seabreeze Avenue".to_owned()),
            account: Some("12345678".to_owned()),
            file_name: "2026-07-15 - AGL bill.pdf".to_owned(),
            cabinet_destination:
                "Bills & Services/12 Seabreeze Avenue/AGL/2026/2026-07-15 - AGL bill.pdf".to_owned(),
            taught_by: reference("subject", 8),
            created_at: "2026-07-24T17:31:00+10:00".to_owned(),
        }
    }

    #[test]
    fn revocation_cutoff_and_device_chain_reject_key_epoch_rollback() {
        let authorization = TrustedDeviceAuthorization {
            device_id: "device".to_owned(),
            authorization_public_key: [0; 32],
            activated_key_epoch: 1,
            revoked_after: Some(PortableAuthorizationCutoff {
                key_epoch: 2,
                sequence: 1,
                event_digest: "a".repeat(64),
            }),
        };
        assert!(matches!(
            verify_authorization_window(&audit_event(1, 2, &"b".repeat(64)), &authorization),
            Err(PortableMemoryError::UntrustedDevice)
        ));

        let connection = Connection::open_in_memory().expect("open an in-memory event chain");
        connection
            .execute_batch(
                "CREATE TABLE portable_events (
                    household_id TEXT NOT NULL,
                    event_id TEXT NOT NULL,
                    event_digest TEXT NOT NULL,
                    signer_device_id TEXT NOT NULL,
                    key_epoch INTEGER NOT NULL,
                    sequence INTEGER NOT NULL
                );
                INSERT INTO portable_events VALUES (
                    'household', 'event:00000000-0000-0000-0000-000000000001',
                    'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
                    'device', 2, 1
                );",
            )
            .expect("seed an epoch-two device-chain head");
        assert!(matches!(
            validate_sequence(&connection, &audit_event(1, 2, &"b".repeat(64))),
            Err(PortableMemoryError::RejectedRecord)
        ));
    }

    #[test]
    fn each_portable_field_enforces_its_owning_domain_reference_kind() {
        assert!(!valid_fact_reference_kinds(&PortableFact::FilingRule {
            rule_reference: reference("message", 1),
            state: PortableFilingRuleState::Active,
            definition: filing_rule_definition(),
        }));
        assert!(!valid_conversation_reference(&Some(
            PortableConversationReference {
                conversation_id: reference("message", 1),
                message_id: reference("conversation", 2),
            }
        )));
        assert!(valid_fact_reference_kinds(&PortableFact::AuditEvent {
            event_kind: PortableAuditEventKind::FilingRuleChanged,
            authority: PortableAuthority::MemberDirection,
            subject_reference: reference("filing-rule", 3),
            outcome: PortableExecutionOutcomeKind::FilingRuleChanged,
            candidate_disposition: None,
        }));
        assert!(valid_fact_reference_kinds(&PortableFact::AuditEvent {
            event_kind: PortableAuditEventKind::ConsentChanged,
            authority: PortableAuthority::AuthorityGrant,
            subject_reference: reference("grant", 4),
            outcome: PortableExecutionOutcomeKind::FiledAndVerified,
            candidate_disposition: None,
        }));
        assert!(!valid_fact_reference_kinds(&PortableFact::AuditEvent {
            event_kind: PortableAuditEventKind::ConsentChanged,
            authority: PortableAuthority::AuthorityGrant,
            subject_reference: reference("document", 4),
            outcome: PortableExecutionOutcomeKind::FiledAndVerified,
            candidate_disposition: None,
        }));
    }
}
