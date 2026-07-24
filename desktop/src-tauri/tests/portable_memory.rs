use std::{
    collections::HashMap,
    fs,
    path::Path,
    sync::{Arc, Mutex},
};

use luna_core::{
    CredentialVault, PortableConversationReference, PortableEventDraft, PortableFact,
    PortableMemoryError, PortableMemoryStore, TrustedDeviceAuthorization, TrustedDeviceManager,
    VaultError,
};

#[derive(Clone, Default)]
struct MemoryCredentialVault {
    secrets: Arc<Mutex<HashMap<String, Vec<u8>>>>,
}

impl CredentialVault for MemoryCredentialVault {
    fn set_secret(&self, name: &str, secret: &[u8]) -> Result<(), VaultError> {
        self.secrets
            .lock()
            .map_err(|_| VaultError::Unavailable)?
            .insert(name.to_owned(), secret.to_vec());
        Ok(())
    }

    fn get_secret(&self, name: &str) -> Result<Option<Vec<u8>>, VaultError> {
        Ok(self
            .secrets
            .lock()
            .map_err(|_| VaultError::Unavailable)?
            .get(name)
            .cloned())
    }

    fn delete_secret(&self, name: &str) -> Result<(), VaultError> {
        self.secrets
            .lock()
            .map_err(|_| VaultError::Unavailable)?
            .remove(name);
        Ok(())
    }
}

fn enrol_two_devices(
    household_id: &str,
) -> (
    TrustedDeviceManager<MemoryCredentialVault>,
    TrustedDeviceManager<MemoryCredentialVault>,
    TrustedDeviceAuthorization,
    TrustedDeviceAuthorization,
) {
    let first = TrustedDeviceManager::new(MemoryCredentialVault::default());
    let enrollment = first
        .enrol_first_device(household_id)
        .expect("enrol the first Trusted Device");
    first
        .confirm_recovery_key(
            household_id,
            &enrollment.recovery_key,
            &enrollment.recovery_envelope,
        )
        .expect("confirm the Recovery Key");
    first
        .set_current_key_epoch(household_id, 1)
        .expect("record the Household key epoch");
    first
        .configure_device_pin(household_id, "246810")
        .expect("unlock the first Trusted Device");

    let second = TrustedDeviceManager::new(MemoryCredentialVault::default());
    let recovered = second
        .recover_device(
            household_id,
            &enrollment.recovery_key,
            &enrollment.recovery_envelope,
            1,
        )
        .expect("recover a second Trusted Device");
    second
        .finalize_recovered_device(household_id, 1)
        .expect("complete service registration for the recovered device");
    second
        .configure_device_pin(household_id, "135790")
        .expect("unlock the recovered Trusted Device");

    let authorization = TrustedDeviceAuthorization {
        device_id: first
            .current_device_public_key(household_id)
            .expect("read the first Trusted Device identity"),
        authorization_public_key: enrollment.device_authorization_public_key,
    };
    let recovered_authorization = TrustedDeviceAuthorization {
        device_id: second
            .current_device_public_key(household_id)
            .expect("read the recovered Trusted Device identity"),
        authorization_public_key: recovered.device_authorization_public_key,
    };
    (first, second, authorization, recovered_authorization)
}

fn only_portable_record(cabinet: &Path) -> Vec<u8> {
    let events = cabinet.join(".luna-memory").join("v1").join("events");
    let paths = fs::read_dir(events)
        .expect("read the reserved portable-memory event area")
        .map(|entry| entry.expect("read a portable-memory entry").path())
        .collect::<Vec<_>>();
    assert_eq!(paths.len(), 1, "one append should create one record");
    fs::read(&paths[0]).expect("read the encrypted portable record")
}

fn only_portable_record_path(cabinet: &Path) -> std::path::PathBuf {
    let events = cabinet.join(".luna-memory").join("v1").join("events");
    let paths = fs::read_dir(events)
        .expect("read the reserved portable-memory event area")
        .map(|entry| entry.expect("read a portable-memory entry").path())
        .collect::<Vec<_>>();
    assert_eq!(paths.len(), 1, "one append should create one record");
    paths[0].clone()
}

#[test]
fn a_recovered_trusted_device_imports_a_signed_encrypted_audit_event() {
    let household_id = "rivera-household";
    let cabinet = tempfile::tempdir().expect("create a temporary Cabinet");
    let first_local = tempfile::tempdir().expect("create the first local database directory");
    let second_local = tempfile::tempdir().expect("create the second local database directory");
    fs::write(
        cabinet.path().join("ordinary household note.txt"),
        b"kept readable",
    )
    .expect("create an ordinary Cabinet file");
    let (first_device, second_device, first_authorization, _) = enrol_two_devices(household_id);
    let first_store = PortableMemoryStore::open(first_local.path().join("luna.db"), first_device)
        .expect("open portable memory on the first device");
    let second_store =
        PortableMemoryStore::open(second_local.path().join("luna.db"), second_device)
            .expect("open portable memory on the recovered device");

    let appended = first_store
        .append(
            household_id,
            cabinet.path(),
            PortableEventDraft {
                event_id: "audit-event-0001".to_owned(),
                sequence: 1,
                previous_event_digest: None,
                supersedes_event_digest: None,
                occurred_at: "2026-07-24T17:30:00+10:00".to_owned(),
                conversation_reference: Some(PortableConversationReference {
                    conversation_id: "conversation-41".to_owned(),
                    message_id: "message-87".to_owned(),
                }),
                fact: PortableFact::AuditEvent {
                    event_kind: "documentFiled".to_owned(),
                    authority: "memberDirection".to_owned(),
                    subject_reference: "document-arrival-23".to_owned(),
                    outcome: "filedAndVerified".to_owned(),
                },
            },
        )
        .expect("append a portable Audit Event");

    assert_eq!(appended.event_id, "audit-event-0001");
    assert_eq!(
        fs::read(cabinet.path().join("ordinary household note.txt"))
            .expect("read the ordinary Cabinet file"),
        b"kept readable"
    );
    let portable_record = only_portable_record(cabinet.path());
    let portable_text = String::from_utf8_lossy(&portable_record);
    assert!(!portable_text.contains("documentFiled"));
    assert!(!portable_text.contains("document-arrival-23"));
    assert!(!portable_text.contains("filedAndVerified"));
    assert!(!cabinet.path().join(".luna-memory").join("luna.db").exists());

    let report = second_store
        .import(
            household_id,
            cabinet.path(),
            std::slice::from_ref(&first_authorization),
        )
        .expect("verify and import portable memory");
    assert_eq!(report.imported, 1);
    assert_eq!(report.duplicates, 0);
    assert!(report.conflicts.is_empty());

    let imported = second_store
        .list_events(household_id)
        .expect("list rebuilt portable events");
    assert_eq!(imported, vec![appended]);

    let duplicate = second_store
        .import(
            household_id,
            cabinet.path(),
            std::slice::from_ref(&first_authorization),
        )
        .expect("re-deliver the same portable event");
    assert_eq!(duplicate.imported, 0);
    assert_eq!(duplicate.duplicates, 1);
    assert!(duplicate.conflicts.is_empty());
}

#[test]
fn secret_shaped_content_is_rejected_before_portable_memory_is_written() {
    let household_id = "rivera-household";
    let cabinet = tempfile::tempdir().expect("create a temporary Cabinet");
    let local = tempfile::tempdir().expect("create a local database directory");
    let (first_device, _, _, _) = enrol_two_devices(household_id);
    let store = PortableMemoryStore::open(local.path().join("luna.db"), first_device)
        .expect("open portable memory");

    let error = store
        .append(
            household_id,
            cabinet.path(),
            PortableEventDraft {
                event_id: "consent-event-with-secret".to_owned(),
                sequence: 1,
                previous_event_digest: None,
                supersedes_event_digest: None,
                occurred_at: "2026-07-24T17:40:00+10:00".to_owned(),
                conversation_reference: None,
                fact: PortableFact::ConsentGrant {
                    grant_reference: "consent-grant-1".to_owned(),
                    provider: "OpenAI".to_owned(),
                    scope: "Bearer luna-test-token-should-never-sync".to_owned(),
                    state: "granted".to_owned(),
                },
            },
        )
        .expect_err("credential-shaped content must not enter portable memory");

    assert!(matches!(error, PortableMemoryError::SensitiveMaterial));
    assert!(!cabinet.path().join(".luna-memory").exists());
    assert!(store
        .list_events(household_id)
        .expect("list local portable events")
        .is_empty());
}

#[test]
fn concurrent_filing_rule_events_create_a_resolvable_conflict() {
    let household_id = "rivera-household";
    let cabinet = tempfile::tempdir().expect("create a temporary Cabinet");
    let first_local = tempfile::tempdir().expect("create the first local database directory");
    let second_local = tempfile::tempdir().expect("create the second local database directory");
    let (first_device, second_device, first_authorization, second_authorization) =
        enrol_two_devices(household_id);
    let first_store = PortableMemoryStore::open(first_local.path().join("luna.db"), first_device)
        .expect("open portable memory on the first device");
    let second_store =
        PortableMemoryStore::open(second_local.path().join("luna.db"), second_device)
            .expect("open portable memory on the recovered device");

    let paused = first_store
        .append(
            household_id,
            cabinet.path(),
            PortableEventDraft {
                event_id: "rule-event-paused".to_owned(),
                sequence: 1,
                previous_event_digest: None,
                supersedes_event_digest: None,
                occurred_at: "2026-07-24T18:00:00+10:00".to_owned(),
                conversation_reference: None,
                fact: PortableFact::FilingRule {
                    rule_reference: "filing-rule-7".to_owned(),
                    state: "paused".to_owned(),
                },
            },
        )
        .expect("append the first Filing Rule event");
    second_store
        .append(
            household_id,
            cabinet.path(),
            PortableEventDraft {
                event_id: "rule-event-deleted".to_owned(),
                sequence: 1,
                previous_event_digest: None,
                supersedes_event_digest: None,
                occurred_at: "2026-07-24T18:00:01+10:00".to_owned(),
                conversation_reference: None,
                fact: PortableFact::FilingRule {
                    rule_reference: "filing-rule-7".to_owned(),
                    state: "deleted".to_owned(),
                },
            },
        )
        .expect("append a concurrent Filing Rule event");

    let report = second_store
        .import(
            household_id,
            cabinet.path(),
            &[first_authorization, second_authorization],
        )
        .expect("import concurrent portable events");
    assert_eq!(report.imported, 1);
    assert_eq!(report.duplicates, 1);
    assert_eq!(report.conflicts.len(), 1);

    let conflicts = second_store
        .list_conflicts(household_id)
        .expect("list unresolved portable-memory conflicts");
    assert_eq!(conflicts, report.conflicts);
    let conflict = &conflicts[0];
    assert_eq!(conflict.subject_reference, "filingRule:filing-rule-7");

    second_store
        .resolve_conflict(household_id, conflict.id, &paused.event_id)
        .expect("choose the retained Filing Rule event");
    assert!(second_store
        .list_conflicts(household_id)
        .expect("list resolved portable-memory conflicts")
        .is_empty());
    assert_eq!(
        second_store
            .current_subject_event(household_id, "filingRule:filing-rule-7")
            .expect("read the resolved Filing Rule projection")
            .expect("the resolved Filing Rule event should exist")
            .event_id,
        paused.event_id
    );
}

#[test]
fn a_portable_event_with_a_substituted_device_predecessor_is_rejected() {
    let household_id = "rivera-household";
    let cabinet = tempfile::tempdir().expect("create a temporary Cabinet");
    let local = tempfile::tempdir().expect("create a local database directory");
    let (first_device, _, _, _) = enrol_two_devices(household_id);
    let store = PortableMemoryStore::open(local.path().join("luna.db"), first_device)
        .expect("open portable memory");
    let first = store
        .append(
            household_id,
            cabinet.path(),
            PortableEventDraft {
                event_id: "audit-event-first".to_owned(),
                sequence: 1,
                previous_event_digest: None,
                supersedes_event_digest: None,
                occurred_at: "2026-07-24T18:10:00+10:00".to_owned(),
                conversation_reference: None,
                fact: PortableFact::AuditEvent {
                    event_kind: "documentFiled".to_owned(),
                    authority: "memberDirection".to_owned(),
                    subject_reference: "document-arrival-55".to_owned(),
                    outcome: "filedAndVerified".to_owned(),
                },
            },
        )
        .expect("append the first device event");

    let error = store
        .append(
            household_id,
            cabinet.path(),
            PortableEventDraft {
                event_id: "audit-event-replayed".to_owned(),
                sequence: 2,
                previous_event_digest: Some("substituted-predecessor".to_owned()),
                supersedes_event_digest: None,
                occurred_at: "2026-07-24T18:11:00+10:00".to_owned(),
                conversation_reference: None,
                fact: PortableFact::AuditEvent {
                    event_kind: "documentFiled".to_owned(),
                    authority: "memberDirection".to_owned(),
                    subject_reference: "document-arrival-56".to_owned(),
                    outcome: "filedAndVerified".to_owned(),
                },
            },
        )
        .expect_err("the device event chain must reject a substituted predecessor");

    assert!(matches!(error, PortableMemoryError::RejectedRecord));
    assert_eq!(
        store
            .list_events(household_id)
            .expect("list accepted portable events"),
        vec![first]
    );
    let events = cabinet
        .path()
        .join(".luna-memory")
        .join("v1")
        .join("events");
    assert_eq!(
        fs::read_dir(events)
            .expect("read portable event records")
            .count(),
        1
    );
}

#[test]
fn a_modified_portable_record_is_rejected_before_local_import() {
    let household_id = "rivera-household";
    let cabinet = tempfile::tempdir().expect("create a temporary Cabinet");
    let first_local = tempfile::tempdir().expect("create the first local database directory");
    let second_local = tempfile::tempdir().expect("create the second local database directory");
    let (first_device, second_device, first_authorization, _) = enrol_two_devices(household_id);
    let first_store = PortableMemoryStore::open(first_local.path().join("luna.db"), first_device)
        .expect("open portable memory on the first device");
    let second_store =
        PortableMemoryStore::open(second_local.path().join("luna.db"), second_device)
            .expect("open portable memory on the recovered device");
    first_store
        .append(
            household_id,
            cabinet.path(),
            PortableEventDraft {
                event_id: "audit-event-tamper-target".to_owned(),
                sequence: 1,
                previous_event_digest: None,
                supersedes_event_digest: None,
                occurred_at: "2026-07-24T18:20:00+10:00".to_owned(),
                conversation_reference: None,
                fact: PortableFact::AuditEvent {
                    event_kind: "documentFiled".to_owned(),
                    authority: "memberDirection".to_owned(),
                    subject_reference: "document-arrival-60".to_owned(),
                    outcome: "filedAndVerified".to_owned(),
                },
            },
        )
        .expect("append an event before external modification");

    let path = only_portable_record_path(cabinet.path());
    let mut record: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).expect("read the portable record"))
            .expect("decode the public portable envelope");
    record["eventDigest"] = serde_json::Value::String("0".repeat(64));
    fs::write(
        &path,
        serde_json::to_vec(&record).expect("encode the modified portable envelope"),
    )
    .expect("simulate an externally modified portable record");

    let error = second_store
        .import(household_id, cabinet.path(), &[first_authorization])
        .expect_err("a modified portable record must be rejected");
    assert!(matches!(error, PortableMemoryError::RejectedRecord));
    assert!(second_store
        .list_events(household_id)
        .expect("list imported portable events")
        .is_empty());
}
