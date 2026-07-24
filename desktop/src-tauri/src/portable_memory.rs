use std::{
    collections::HashMap,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{CredentialVault, ProtectedHouseholdState, TrustedDeviceError, TrustedDeviceManager};

const PORTABLE_MEMORY_DIRECTORY: &str = ".luna-memory";
const PORTABLE_MEMORY_VERSION: u8 = 1;
const PORTABLE_EVENT_EXTENSION: &str = "luna-event";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortableConversationReference {
    pub conversation_id: String,
    pub message_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PortableFact {
    FilingRule {
        rule_reference: String,
        state: String,
    },
    DocumentRelationship {
        document_reference: String,
        related_document_reference: String,
        relationship: String,
    },
    MemberDirection {
        direction: String,
        subject_reference: String,
    },
    AuthorityGrant {
        grant_reference: String,
        subject_reference: String,
        scope: String,
    },
    ConsentGrant {
        grant_reference: String,
        provider: String,
        scope: String,
        state: String,
    },
    ExecutionOutcome {
        subject_reference: String,
        outcome: String,
    },
    AuditEvent {
        event_kind: String,
        authority: String,
        subject_reference: String,
        outcome: String,
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
        store.connect()?.execute_batch(
            "CREATE TABLE IF NOT EXISTS portable_events (
                household_id TEXT NOT NULL,
                event_id TEXT NOT NULL,
                event_digest TEXT NOT NULL,
                signer_device_id TEXT NOT NULL,
                sequence INTEGER NOT NULL,
                protected_payload TEXT NOT NULL,
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
                resolved_event_id TEXT
            );
            CREATE UNIQUE INDEX IF NOT EXISTS portable_conflict_pair
                ON portable_conflicts(
                    household_id, subject_reference, existing_event_id, conflicting_event_id
                );",
        )?;
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
        validate_sequence(&self.connect()?, &event)?;
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
        let event_area = event_area(cabinet_root.as_ref())?;
        let path = event_area.join(format!(
            "{}.{PORTABLE_EVENT_EXTENSION}",
            sha256_hex(event.event_id.as_bytes())
        ));
        append_create_only(&path, &record)?;
        self.store_local_event(&event, &envelope.protected_event)?;
        self.advance_subject_head(&event, false)?;
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
            .map(|device| (device.device_id.as_str(), device.authorization_public_key))
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
            let expected_key = trusted_devices
                .get(envelope.signer_device_id.as_str())
                .ok_or(PortableMemoryError::UntrustedDevice)?;
            verify_envelope(&envelope, expected_key)?;
            let plaintext = self
                .trusted_device
                .open_household_state(household_id, &envelope.protected_event)
                .map_err(|_| PortableMemoryError::RejectedRecord)?;
            let event: PortableEvent = serde_json::from_slice(&plaintext)
                .map_err(|_| PortableMemoryError::RejectedRecord)?;
            verify_event(household_id, &event, &envelope)?;
            events.push((event, envelope.protected_event));
        }
        events.sort_by(|(left, _), (right, _)| {
            left.signer_device_id
                .cmp(&right.signer_device_id)
                .then(left.sequence.cmp(&right.sequence))
        });

        let mut report = PortableImportReport::default();
        for (event, protected) in events {
            match self.import_event(&event, &protected)? {
                ImportOutcome::Imported(conflict) => {
                    report.imported += 1;
                    if let Some(conflict) = conflict {
                        report.conflicts.push(conflict);
                    }
                }
                ImportOutcome::Duplicate => report.duplicates += 1,
            }
        }
        Ok(report)
    }

    pub fn list_events(
        &self,
        household_id: &str,
    ) -> Result<Vec<PortableEvent>, PortableMemoryError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT protected_payload FROM portable_events
              WHERE household_id = ?1 ORDER BY signer_device_id, sequence",
        )?;
        let protected = statement
            .query_map(params![household_id], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        protected
            .into_iter()
            .map(|value| {
                let protected: ProtectedHouseholdState = serde_json::from_str(&value)?;
                let plaintext = self
                    .trusted_device
                    .open_household_state(household_id, &protected)?;
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
        conflict_id: i64,
        chosen_event_id: &str,
    ) -> Result<(), PortableMemoryError> {
        let mut connection = self.connect()?;
        let conflict: Option<(String, String, String)> = connection
            .query_row(
                "SELECT subject_reference, existing_event_id, conflicting_event_id
                   FROM portable_conflicts
                  WHERE household_id = ?1 AND id = ?2 AND resolved_event_id IS NULL",
                params![household_id, conflict_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        let (subject_reference, existing_event_id, conflicting_event_id) =
            conflict.ok_or(PortableMemoryError::InvalidEvent)?;
        if chosen_event_id != existing_event_id && chosen_event_id != conflicting_event_id {
            return Err(PortableMemoryError::InvalidEvent);
        }
        let digest: String = connection
            .query_row(
                "SELECT event_digest FROM portable_events
                  WHERE household_id = ?1 AND event_id = ?2",
                params![household_id, chosen_event_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or(PortableMemoryError::InvalidEvent)?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "INSERT INTO portable_subject_heads (
                household_id, subject_reference, event_id, event_digest
             ) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(household_id, subject_reference) DO UPDATE SET
                event_id = excluded.event_id,
                event_digest = excluded.event_digest",
            params![household_id, subject_reference, chosen_event_id, digest],
        )?;
        transaction.execute(
            "UPDATE portable_conflicts SET resolved_event_id = ?1
              WHERE household_id = ?2 AND id = ?3",
            params![chosen_event_id, household_id, conflict_id],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn current_subject_event(
        &self,
        household_id: &str,
        subject_reference: &str,
    ) -> Result<Option<PortableEvent>, PortableMemoryError> {
        let connection = self.connect()?;
        let protected: Option<String> = connection
            .query_row(
                "SELECT events.protected_payload
                   FROM portable_subject_heads AS heads
                   JOIN portable_events AS events
                     ON events.household_id = heads.household_id
                    AND events.event_id = heads.event_id
                  WHERE heads.household_id = ?1 AND heads.subject_reference = ?2",
                params![household_id, subject_reference],
                |row| row.get(0),
            )
            .optional()?;
        protected
            .map(|value| {
                let protected: ProtectedHouseholdState = serde_json::from_str(&value)?;
                let plaintext = self
                    .trusted_device
                    .open_household_state(household_id, &protected)?;
                Ok(serde_json::from_slice(&plaintext)?)
            })
            .transpose()
    }

    fn import_event(
        &self,
        event: &PortableEvent,
        protected: &ProtectedHouseholdState,
    ) -> Result<ImportOutcome, PortableMemoryError> {
        let connection = self.connect()?;
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
        self.store_local_event(event, protected)?;
        Ok(ImportOutcome::Imported(
            self.advance_subject_head(event, true)?,
        ))
    }

    fn store_local_event(
        &self,
        event: &PortableEvent,
        protected: &ProtectedHouseholdState,
    ) -> Result<(), PortableMemoryError> {
        let protected = serde_json::to_string(protected)?;
        let connection = self.connect()?;
        let inserted = connection.execute(
            "INSERT OR IGNORE INTO portable_events (
                household_id, event_id, event_digest, signer_device_id, sequence, protected_payload
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                event.household_id,
                event.event_id,
                event.digest,
                event.signer_device_id,
                event.sequence,
                protected
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
            if !existing.is_some_and(|(id, digest)| id == event.event_id && digest == event.digest)
            {
                return Err(PortableMemoryError::RecordConflict);
            }
        }
        Ok(())
    }

    fn connect(&self) -> Result<Connection, PortableMemoryError> {
        Ok(Connection::open(&self.database)?)
    }

    fn advance_subject_head(
        &self,
        event: &PortableEvent,
        record_conflict: bool,
    ) -> Result<Option<PortableConflict>, PortableMemoryError> {
        let Some(subject_reference) = portable_subject_reference(&event.fact) else {
            return Ok(None);
        };
        let connection = self.connect()?;
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
                connection.execute(
                    "INSERT OR IGNORE INTO portable_conflicts (
                        household_id, subject_reference, existing_event_id, conflicting_event_id
                     ) VALUES (?1, ?2, ?3, ?4)",
                    params![
                        event.household_id,
                        subject_reference,
                        existing_event_id,
                        event.event_id
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
                        existing_event_id,
                        event.event_id
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ImportOutcome {
    Imported(Option<PortableConflict>),
    Duplicate,
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
    if draft.event_id.trim().is_empty()
        || draft.occurred_at.trim().is_empty()
        || draft.sequence == 0
        || (draft.sequence == 1 && draft.previous_event_digest.is_some())
        || (draft.sequence > 1 && draft.previous_event_digest.is_none())
        || (draft.sequence == 1 && draft.supersedes_event_digest.as_deref() == Some(""))
    {
        return Err(PortableMemoryError::InvalidEvent);
    }
    let serialized = serde_json::to_string(&(
        &draft.event_id,
        &draft.occurred_at,
        &draft.conversation_reference,
        &draft.fact,
    ))?
    .to_ascii_lowercase();
    if [
        "bearer ",
        "sk-proj-",
        "sk-ant-",
        "api_key=",
        "api-key:",
        "access_token",
        "refresh_token",
        "token=",
        "secret=",
        "-----begin private key-----",
    ]
    .iter()
    .any(|marker| serialized.contains(marker))
    {
        return Err(PortableMemoryError::SensitiveMaterial);
    }
    Ok(())
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

fn validate_sequence(
    connection: &Connection,
    event: &PortableEvent,
) -> Result<(), PortableMemoryError> {
    let head: Option<(u64, String)> = connection
        .query_row(
            "SELECT sequence, event_digest FROM portable_events
              WHERE household_id = ?1 AND signer_device_id = ?2
              ORDER BY sequence DESC LIMIT 1",
            params![event.household_id, event.signer_device_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    match head {
        None if event.sequence == 1 && event.previous_event_digest.is_none() => Ok(()),
        Some((sequence, digest))
            if event.sequence == sequence + 1
                && event.previous_event_digest.as_deref() == Some(digest.as_str()) =>
        {
            Ok(())
        }
        _ => Err(PortableMemoryError::RejectedRecord),
    }
}

fn append_create_only(path: &Path, record: &[u8]) -> Result<(), PortableMemoryError> {
    match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(mut file) => {
            if let Err(error) = file.write_all(record).and_then(|_| file.sync_all()) {
                let _ = fs::remove_file(path);
                return Err(error.into());
            }
            Ok(())
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            if fs::read(path)? == record {
                Ok(())
            } else {
                Err(PortableMemoryError::RecordConflict)
            }
        }
        Err(error) => Err(error.into()),
    }
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
        | PortableFact::AuditEvent { .. } => None,
    }
}
