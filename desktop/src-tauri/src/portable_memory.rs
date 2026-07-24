use std::{
    collections::HashMap,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{CredentialVault, ProtectedHouseholdState, TrustedDeviceError, TrustedDeviceManager};

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
    WaitingForConnectivity,
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
    pub fields: Vec<PortableReference>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PortableFact {
    FilingRule {
        rule_reference: PortableReference,
        state: PortableFilingRuleState,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortableAuthorizationCutoff {
    pub key_epoch: u32,
    pub sequence: u64,
    pub event_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortableConflict {
    pub id: i64,
    pub subject_reference: String,
    pub existing_event_id: String,
    pub conflicting_event_id: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PortableImportReport {
    pub imported: usize,
    pub duplicates: usize,
    pub conflicts: Vec<PortableConflict>,
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
    event_digest: String,
    protected_event: ProtectedHouseholdState,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PortableEnvelope {
    version: u8,
    signer_device_id: String,
    key_epoch: u32,
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
        let event_area = event_area(cabinet_root.as_ref())?;
        let signer_device_id = self
            .trusted_device
            .current_device_public_key(household_id)?;
        let path = event_area.join(format!(
            "{}.{PORTABLE_EVENT_EXTENSION}",
            sha256_hex(draft.event_id.as_bytes())
        ));
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
        append_create_only(&path, &record)?;
        Ok(event)
    }

    pub fn import(
        &self,
        household_id: &str,
        cabinet_root: impl AsRef<Path>,
        trusted_devices: &[TrustedDeviceAuthorization],
    ) -> Result<PortableImportReport, PortableMemoryError> {
        let event_area = cabinet_root
            .as_ref()
            .join(PORTABLE_MEMORY_DIRECTORY)
            .join(format!("v{PORTABLE_MEMORY_VERSION}"))
            .join("events");
        if !event_area.is_dir() {
            return Ok(PortableImportReport::default());
        }
        let trusted_devices = trusted_devices
            .iter()
            .map(|device| (device.device_id.as_str(), device))
            .collect::<HashMap<_, _>>();
        let mut paths = fs::read_dir(event_area)?
            .map(|entry| entry.map(|value| value.path()))
            .collect::<Result<Vec<_>, _>>()?;
        paths.sort();

        let mut events = Vec::new();
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
            verify_authorization_window(&event, authorization)?;
            events.push((event, envelope.protected_event, record));
        }
        for authorization in trusted_devices.values() {
            if let Some(cutoff) = &authorization.revoked_after {
                let cutoff_is_present = events.iter().any(|(event, _, _)| {
                    event.signer_device_id == authorization.device_id
                        && event.sequence == cutoff.sequence
                        && event.digest == cutoff.event_digest
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

    fn connect(&self) -> Result<Connection, PortableMemoryError> {
        Ok(Connection::open(&self.database)?)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ImportOutcome {
    Imported(Option<PortableConflict>),
    Duplicate,
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
        PortableFact::FilingRule { rule_reference, .. } => rule_reference.kind() == "filing-rule",
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
            ..
        } => {
            grant_reference.kind() == "grant"
                && scope
                    .document_type
                    .as_ref()
                    .is_none_or(|reference| reference.kind() == "document-type")
                && scope
                    .fields
                    .iter()
                    .all(|reference| reference.kind() == "field")
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
    (20..=35).contains(&value.len())
        && value.contains('T')
        && value.bytes().all(|byte| {
            byte.is_ascii_digit() || matches!(byte, b'-' | b':' | b'.' | b'+' | b'T' | b'Z')
        })
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
        || event.digest != envelope.event_digest
        || event.digest != expected_digest
    {
        return Err(PortableMemoryError::RejectedRecord);
    }
    Ok(())
}

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
            },
            digest: digest.to_owned(),
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
            outcome: PortableExecutionOutcomeKind::FiledAndVerified,
        }));
        assert!(valid_fact_reference_kinds(&PortableFact::AuditEvent {
            event_kind: PortableAuditEventKind::ConsentChanged,
            authority: PortableAuthority::AuthorityGrant,
            subject_reference: reference("grant", 4),
            outcome: PortableExecutionOutcomeKind::FiledAndVerified,
        }));
        assert!(!valid_fact_reference_kinds(&PortableFact::AuditEvent {
            event_kind: PortableAuditEventKind::ConsentChanged,
            authority: PortableAuthority::AuthorityGrant,
            subject_reference: reference("document", 4),
            outcome: PortableExecutionOutcomeKind::FiledAndVerified,
        }));
    }
}
