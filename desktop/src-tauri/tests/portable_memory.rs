use std::{
    collections::{BTreeMap, HashMap},
    fs,
    io::Cursor,
    path::Path,
    sync::{Arc, Mutex},
};

use luna_core::{
    CandidateDisposition, CloudConsentDecision, CloudIntelligenceStore, ContextRelevanceDirection,
    ConversationStore, CredentialVault, DeterministicIntelligenceGateway, DocumentContextDirection,
    DocumentProcessingState, FilingDecisionDirection, IntelligenceCapability, IntelligenceEvidence,
    IntelligenceExecutionConstraints, IntelligenceFailure, IntelligenceModelDescriptor,
    IntelligenceProviderDescriptor, IntelligenceRequest, IntelligenceResponseSchema,
    IntelligenceSelection, LocalOcr, PortableAuditEventKind, PortableAuthority,
    PortableAuthorizationCutoff, PortableCandidateDisposition, PortableConflictResolutionDraft,
    PortableConsentDetails, PortableConsentField, PortableConsentGrantKind,
    PortableConsentProvider, PortableConsentPurpose, PortableConsentScope, PortableConsentState,
    PortableConversationReference, PortableDocumentRelationshipKind, PortableEvent,
    PortableEventDraft, PortableExecutionOutcomeKind, PortableFact, PortableFilingRuleDefinition,
    PortableFilingRuleState, PortableIntelligenceCapability, PortableMemberDirectionKind,
    PortableMemoryError, PortableMemoryStore, PortableReference, TrustedDeviceAuthorization,
    TrustedDeviceManager, VaultError,
};
use sha2::{Digest, Sha256};

#[derive(Clone, Default)]
struct MemoryCredentialVault {
    secrets: Arc<Mutex<HashMap<String, Vec<u8>>>>,
}

struct FixedLocalOcr(&'static str);

impl LocalOcr for FixedLocalOcr {
    fn extract_text(&self, _original: &Path, _media_type: &str) -> Option<String> {
        Some(self.0.to_owned())
    }
}

fn png_fixture() -> Vec<u8> {
    let image = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
        1,
        1,
        image::Rgb([255, 255, 255]),
    ));
    let mut bytes = Cursor::new(Vec::new());
    image
        .write_to(&mut bytes, image::ImageFormat::Png)
        .expect("encode PNG fixture");
    bytes.into_inner()
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

fn reference(value: &str) -> PortableReference {
    let kind = if value.starts_with("conversation") {
        "conversation"
    } else if value.starts_with("message") {
        "message"
    } else if value.starts_with("filing-rule") {
        "filing-rule"
    } else {
        "document"
    };
    PortableReference::new(format!("{kind}:{}", uuid_for(value)))
        .expect("test references should satisfy the portable schema")
}

fn event_id(value: &str) -> String {
    format!("event:{}", uuid_for(value))
}

fn uuid_for(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        digest[0],
        digest[1],
        digest[2],
        digest[3],
        digest[4],
        digest[5],
        digest[6],
        digest[7],
        digest[8],
        digest[9],
        digest[10],
        digest[11],
        digest[12],
        digest[13],
        digest[14],
        digest[15]
    )
}

fn filing_rule_subject(value: &str) -> String {
    format!("filingRule:{}", reference(value))
}

fn typed_reference(kind: &str, value: &str) -> PortableReference {
    PortableReference::new(format!("{kind}:{}", uuid_for(value)))
        .expect("test references should satisfy the portable schema")
}

fn filing_rule_definition() -> PortableFilingRuleDefinition {
    PortableFilingRuleDefinition {
        document_type: "Electricity bill".to_owned(),
        service_provider: "AGL".to_owned(),
        addressee: "Sam Rivera".to_owned(),
        property: Some("12 Seabreeze Avenue".to_owned()),
        account: Some("12345678".to_owned()),
        file_name: "2026-07-15 - AGL - Electricity bill - Sam Rivera.pdf".to_owned(),
        cabinet_destination:
            "Bills & Services/12 Seabreeze Avenue/AGL/2026/2026-07-15 - AGL - Electricity bill - Sam Rivera.pdf"
                .to_owned(),
        taught_by: PortableReference::new(format!("subject:{}", uuid_for("member-sam")))
            .expect("construct the teaching Member reference"),
        created_at: "2026-07-24T17:31:00+10:00".to_owned(),
    }
}

fn append_fact(
    store: &PortableMemoryStore<MemoryCredentialVault>,
    household_id: &str,
    cabinet: &Path,
    name: &str,
    sequence: u64,
    previous: Option<&PortableEvent>,
    fact: PortableFact,
) -> PortableEvent {
    store
        .append(
            household_id,
            cabinet,
            PortableEventDraft {
                event_id: event_id(name),
                sequence,
                previous_event_digest: previous.map(|event| event.digest.clone()),
                supersedes_event_digest: None,
                occurred_at: format!("2026-07-24T18:{sequence:02}:00+10:00"),
                conversation_reference: Some(PortableConversationReference {
                    conversation_id: reference("conversation-portable-rebuild"),
                    message_id: typed_reference("message", name),
                }),
                fact,
            },
        )
        .expect("append a typed portable fact")
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
        activated_key_epoch: 1,
        revoked_after: None,
    };
    let recovered_authorization = TrustedDeviceAuthorization {
        device_id: second
            .current_device_public_key(household_id)
            .expect("read the recovered Trusted Device identity"),
        authorization_public_key: recovered.device_authorization_public_key,
        activated_key_epoch: 1,
        revoked_after: None,
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
                event_id: event_id("audit-event-0001"),
                sequence: 1,
                previous_event_digest: None,
                supersedes_event_digest: None,
                occurred_at: "2026-07-24T17:30:00+10:00".to_owned(),
                conversation_reference: Some(PortableConversationReference {
                    conversation_id: reference("conversation-41"),
                    message_id: reference("message-87"),
                }),
                fact: PortableFact::AuditEvent {
                    event_kind: PortableAuditEventKind::DocumentFiled,
                    authority: PortableAuthority::MemberDirection,
                    subject_reference: reference("document-arrival-23"),
                    outcome: PortableExecutionOutcomeKind::FiledAndVerified,
                    candidate_disposition: None,
                },
            },
        )
        .expect("append a portable Audit Event");

    assert_eq!(appended.event_id, event_id("audit-event-0001"));
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
fn a_recovered_trusted_device_rebuilds_the_current_filing_rule_projection() {
    let household_id = "rivera-household";
    let cabinet = tempfile::tempdir().expect("create a temporary Cabinet");
    let first_local = tempfile::tempdir().expect("create the first local database directory");
    let second_local = tempfile::tempdir().expect("create the second local database directory");
    let (first_device, second_device, first_authorization, _) = enrol_two_devices(household_id);
    let first_store =
        PortableMemoryStore::open(first_local.path().join("luna.db"), first_device.clone())
            .expect("open portable memory on the first device");
    let second_database = second_local.path().join("luna.db");
    let second_store = PortableMemoryStore::open(&second_database, second_device.clone())
        .expect("open portable memory on the recovered device");
    let rule_reference = reference("filing-rule-electricity");

    let appended = first_store
        .append(
            household_id,
            cabinet.path(),
            PortableEventDraft {
                event_id: event_id("portable-filing-rule"),
                sequence: 1,
                previous_event_digest: None,
                supersedes_event_digest: None,
                occurred_at: "2026-07-24T17:32:00+10:00".to_owned(),
                conversation_reference: Some(PortableConversationReference {
                    conversation_id: reference("conversation-filing-rule"),
                    message_id: reference("message-filing-rule"),
                }),
                fact: PortableFact::FilingRule {
                    rule_reference: rule_reference.clone(),
                    state: PortableFilingRuleState::Active,
                    definition: PortableFilingRuleDefinition {
                        document_type: "Electricity bill".to_owned(),
                        service_provider: "AGL".to_owned(),
                        addressee: "Sam Rivera".to_owned(),
                        property: Some("12 Seabreeze Avenue".to_owned()),
                        account: Some("12345678".to_owned()),
                        file_name: "2026-07-15 - AGL - Electricity bill - Sam Rivera.pdf"
                            .to_owned(),
                        cabinet_destination:
                            "Bills & Services/12 Seabreeze Avenue/AGL/2026/2026-07-15 - AGL - Electricity bill - Sam Rivera.pdf"
                                .to_owned(),
                        taught_by: PortableReference::new(format!(
                            "subject:{}",
                            uuid_for("member-sam")
                        ))
                        .expect("construct the teaching Member reference"),
                        created_at: "2026-07-24T17:31:00+10:00".to_owned(),
                    },
                },
            },
        )
        .expect("append the portable Filing Rule");

    second_store
        .import(
            household_id,
            cabinet.path(),
            std::slice::from_ref(&first_authorization),
        )
        .expect("import the portable Filing Rule");
    let projection = second_store
        .household_projection(household_id)
        .expect("read the rebuilt Household projection");

    assert_eq!(projection.filing_rules, vec![appended]);
    assert!(projection.conflicts.is_empty());
}

#[test]
fn a_recovered_trusted_device_rebuilds_relationship_consent_and_history_projections() {
    let household_id = "rivera-household";
    let cabinet = tempfile::tempdir().expect("create a temporary Cabinet");
    let first_local = tempfile::tempdir().expect("create the first local database directory");
    let second_local = tempfile::tempdir().expect("create the second local database directory");
    let (first_device, second_device, first_authorization, _) = enrol_two_devices(household_id);
    let first_store =
        PortableMemoryStore::open(first_local.path().join("luna.db"), first_device.clone())
            .expect("open portable memory on the first device");
    let second_database = second_local.path().join("luna.db");
    let second_store = PortableMemoryStore::open(&second_database, second_device.clone())
        .expect("open portable memory on the recovered device");
    let document = reference("document-primary");
    let related_document = reference("document-updated");
    let grant = typed_reference("grant", "consent-denied");
    let subject = typed_reference("subject", "document-handling");

    let relationship = append_fact(
        &first_store,
        household_id,
        cabinet.path(),
        "portable-relationship",
        1,
        None,
        PortableFact::DocumentRelationship {
            document_reference: document.clone(),
            related_document_reference: related_document,
            relationship: PortableDocumentRelationshipKind::UpdatedVersion,
        },
    );
    let direction = append_fact(
        &first_store,
        household_id,
        cabinet.path(),
        "portable-direction",
        2,
        Some(&relationship),
        PortableFact::MemberDirection {
            direction: PortableMemberDirectionKind::KeepBoth,
            subject_reference: subject.clone(),
        },
    );
    let authority = append_fact(
        &first_store,
        household_id,
        cabinet.path(),
        "portable-authority",
        3,
        Some(&direction),
        PortableFact::AuthorityGrant {
            grant_reference: typed_reference("grant", "authority-file"),
            subject_reference: subject.clone(),
            scope: vec![typed_reference("field", "filing-destination")],
        },
    );
    let consent = append_fact(
        &first_store,
        household_id,
        cabinet.path(),
        "portable-consent",
        4,
        Some(&authority),
        PortableFact::ConsentGrant {
            grant_reference: grant.clone(),
            provider: PortableConsentProvider::OpenAi,
            scope: PortableConsentScope {
                document_type: Some(typed_reference("document-type", "electricity-bill")),
                document_reference: Some(typed_reference("document", "arrival-42")),
                future_scope: None,
                future_scope_evidence: Vec::new(),
                fields: vec![PortableConsentField::Amount],
            },
            state: PortableConsentState::Denied,
            details: PortableConsentDetails {
                model_id: "gpt-5.6-luna".to_owned(),
                capability: PortableIntelligenceCapability::DirectionInterpretation,
                purpose: PortableConsentPurpose::DocumentEvaluation,
                kind: PortableConsentGrantKind::OneTime,
                granted_by: typed_reference("subject", "member-sam"),
                created_at: "2026-07-24T18:03:30+10:00".to_owned(),
                consumed_at: None,
                revoked_at: None,
            },
        },
    );
    let outcome = append_fact(
        &first_store,
        household_id,
        cabinet.path(),
        "portable-provider-outcome",
        5,
        Some(&consent),
        PortableFact::ExecutionOutcome {
            subject_reference: subject,
            outcome: PortableExecutionOutcomeKind::ProviderUnavailable,
        },
    );
    let offline = append_fact(
        &first_store,
        household_id,
        cabinet.path(),
        "portable-offline-outcome",
        6,
        Some(&outcome),
        PortableFact::ExecutionOutcome {
            subject_reference: typed_reference("subject", "offline-document-handling"),
            outcome: PortableExecutionOutcomeKind::WaitingForCloudAssistance,
        },
    );
    let audit = append_fact(
        &first_store,
        household_id,
        cabinet.path(),
        "portable-audit",
        7,
        Some(&offline),
        PortableFact::AuditEvent {
            event_kind: PortableAuditEventKind::ConsentChanged,
            authority: PortableAuthority::MemberDirection,
            subject_reference: grant,
            outcome: PortableExecutionOutcomeKind::Failed,
            candidate_disposition: None,
        },
    );

    let rebuilt_conversations = ConversationStore::open(&second_database, second_device.clone())
        .expect("open the recovered Conversation owner");
    let rebuilt_intelligence = CloudIntelligenceStore::open(&second_database, second_device)
        .expect("open the recovered Consent owner");
    second_store
        .synchronize_owned_state(
            household_id,
            cabinet.path(),
            std::slice::from_ref(&first_authorization),
            &rebuilt_conversations,
            &rebuilt_intelligence,
        )
        .expect("import all owning-domain facts");
    let projection = second_store
        .household_projection(household_id)
        .expect("read the rebuilt owning-domain projections");

    assert_eq!(projection.document_relationships, vec![relationship]);
    assert_eq!(projection.member_directions, vec![direction]);
    assert_eq!(projection.authority_grants, vec![authority]);
    assert_eq!(projection.consent_grants, vec![consent]);
    assert_eq!(projection.execution_outcomes, vec![outcome, offline]);
    assert_eq!(projection.audit_events, vec![audit]);
    assert!(projection.conflicts.is_empty());
    assert_eq!(
        rebuilt_conversations
            .list_rebuilt_document_relationships(household_id)
            .expect("list the relationship in its owning local store")
            .len(),
        1
    );
    assert_eq!(
        rebuilt_conversations
            .list_rebuilt_portable_history(household_id)
            .expect("list History in its owning local store")
            .len(),
        1
    );
}

#[test]
fn retry_repairs_a_missing_cabinet_record_without_duplicating_local_projection() {
    let household_id = "rivera-household";
    let cabinet = tempfile::tempdir().expect("create a temporary Cabinet");
    let local = tempfile::tempdir().expect("create a local database directory");
    let (first_device, _, _, _) = enrol_two_devices(household_id);
    let repair_device = first_device.clone();
    let store = PortableMemoryStore::open(local.path().join("luna.db"), first_device)
        .expect("open portable memory");
    let draft = PortableEventDraft {
        event_id: event_id("repairable-event"),
        sequence: 1,
        previous_event_digest: None,
        supersedes_event_digest: None,
        occurred_at: "2026-07-24T17:35:00+10:00".to_owned(),
        conversation_reference: None,
        fact: PortableFact::AuditEvent {
            event_kind: PortableAuditEventKind::DocumentFiled,
            authority: PortableAuthority::MemberDirection,
            subject_reference: reference("document-arrival-repair"),
            outcome: PortableExecutionOutcomeKind::FiledAndVerified,
            candidate_disposition: None,
        },
    };
    let appended = store
        .append(household_id, cabinet.path(), draft.clone())
        .expect("append the repairable event");
    let record_path = only_portable_record_path(cabinet.path());
    fs::remove_file(&record_path).expect("simulate interruption before Cabinet persistence");
    repair_device
        .set_current_key_epoch(household_id, 2)
        .expect("advance the Household key epoch before retry");

    let repaired = store
        .append(household_id, cabinet.path(), draft)
        .expect("retry should restore the old-epoch exact portable record");

    assert_eq!(repaired, appended);
    assert!(record_path.is_file());
    assert_eq!(
        store
            .list_events(household_id)
            .expect("list the idempotent local projection"),
        vec![appended]
    );
}

#[test]
fn an_unavailable_cabinet_keeps_the_portable_fact_local_for_exact_retry() {
    let household_id = "rivera-household";
    let cabinet_parent = tempfile::tempdir().expect("create a temporary Cabinet parent");
    let unavailable_cabinet = cabinet_parent.path().join("remembered-cabinet");
    let local = tempfile::tempdir().expect("create a local database directory");
    let (first_device, _, _, _) = enrol_two_devices(household_id);
    let store = PortableMemoryStore::open(local.path().join("luna.db"), first_device)
        .expect("open portable memory");
    let draft = PortableEventDraft {
        event_id: event_id("cabinet-unavailable-event"),
        sequence: 1,
        previous_event_digest: None,
        supersedes_event_digest: None,
        occurred_at: "2026-07-24T17:40:00+10:00".to_owned(),
        conversation_reference: None,
        fact: PortableFact::ExecutionOutcome {
            subject_reference: PortableReference::new(format!(
                "subject:{}",
                uuid_for("cabinet-unavailable-subject")
            ))
            .expect("construct the portable subject reference"),
            outcome: PortableExecutionOutcomeKind::CabinetUnavailable,
        },
    };

    assert!(matches!(
        store.append(household_id, &unavailable_cabinet, draft.clone()),
        Err(PortableMemoryError::CabinetUnavailable)
    ));
    let local_events = store
        .list_events(household_id)
        .expect("the unavailable Cabinet must not lose the local fact");
    assert_eq!(local_events.len(), 1);
    assert_eq!(local_events[0].event_id, draft.event_id);

    fs::create_dir(&unavailable_cabinet).expect("restore the remembered Cabinet");
    let delivered = store
        .append(household_id, &unavailable_cabinet, draft)
        .expect("retry the exact locally committed record");
    assert_eq!(delivered, local_events[0]);
    assert!(only_portable_record(&unavailable_cabinet).len() > 32);
}

#[test]
fn import_distinguishes_an_unavailable_cabinet_from_an_empty_memory_area() {
    let household_id = "rivera-household";
    let cabinet_parent = tempfile::tempdir().expect("create a temporary Cabinet parent");
    let unavailable_cabinet = cabinet_parent.path().join("remembered-cabinet");
    let local = tempfile::tempdir().expect("create a local database directory");
    let (first_device, _, first_authorization, _) = enrol_two_devices(household_id);
    let store = PortableMemoryStore::open(local.path().join("luna.db"), first_device)
        .expect("open portable memory");

    assert!(matches!(
        store.import(
            household_id,
            &unavailable_cabinet,
            std::slice::from_ref(&first_authorization),
        ),
        Err(PortableMemoryError::CabinetUnavailable)
    ));

    fs::create_dir(&unavailable_cabinet).expect("restore an empty remembered Cabinet");
    assert_eq!(
        store
            .import(
                household_id,
                &unavailable_cabinet,
                std::slice::from_ref(&first_authorization),
            )
            .expect("an available Cabinet with no portable records is valid"),
        Default::default()
    );
}

#[test]
fn synchronization_delivers_a_locally_committed_fact_when_the_cabinet_returns() {
    let household_id = "rivera-household";
    let cabinet_parent = tempfile::tempdir().expect("create a temporary Cabinet parent");
    let remembered_cabinet = cabinet_parent.path().join("remembered-cabinet");
    let first_local = tempfile::tempdir().expect("create the first local database directory");
    let second_local = tempfile::tempdir().expect("create the second local database directory");
    let (first_device, second_device, first_authorization, _) = enrol_two_devices(household_id);
    let first_store = PortableMemoryStore::open(first_local.path().join("luna.db"), first_device)
        .expect("open portable memory on the first device");
    let second_store =
        PortableMemoryStore::open(second_local.path().join("luna.db"), second_device)
            .expect("open portable memory on the recovered device");
    let draft = PortableEventDraft {
        event_id: event_id("pending-cabinet-delivery"),
        sequence: 1,
        previous_event_digest: None,
        supersedes_event_digest: None,
        occurred_at: "2026-07-24T17:42:00+10:00".to_owned(),
        conversation_reference: None,
        fact: PortableFact::ExecutionOutcome {
            subject_reference: typed_reference("subject", "pending-cabinet-delivery"),
            outcome: PortableExecutionOutcomeKind::CabinetUnavailable,
        },
    };

    assert!(matches!(
        first_store.append(household_id, &remembered_cabinet, draft),
        Err(PortableMemoryError::CabinetUnavailable)
    ));
    fs::create_dir(&remembered_cabinet).expect("restore the remembered Cabinet");
    first_store
        .import(
            household_id,
            &remembered_cabinet,
            std::slice::from_ref(&first_authorization),
        )
        .expect("synchronize pending local records after Cabinet recovery");

    assert!(only_portable_record(&remembered_cabinet).len() > 32);
    let recovered = second_store
        .import(
            household_id,
            &remembered_cabinet,
            std::slice::from_ref(&first_authorization),
        )
        .expect("import the record delivered after recovery");
    assert_eq!(recovered.imported, 1);
    assert_eq!(
        second_store
            .household_projection(household_id)
            .expect("read recovered History")
            .execution_outcomes
            .len(),
        1
    );
}

#[test]
fn secret_shaped_content_is_rejected_before_portable_memory_is_written() {
    let household_id = "rivera-household";
    let cabinet = tempfile::tempdir().expect("create a temporary Cabinet");
    let local = tempfile::tempdir().expect("create a local database directory");
    let (first_device, _, _, _) = enrol_two_devices(household_id);
    let store = PortableMemoryStore::open(local.path().join("luna.db"), first_device)
        .expect("open portable memory");

    let error = PortableReference::new("Bearer luna-test-token-should-never-sync")
        .expect_err("credential-shaped content must not enter the typed portable schema");

    assert!(matches!(error, PortableMemoryError::SensitiveMaterial));
    for excluded in [
        "246810",
        "AKIAIOSFODNN7EXAMPLE",
        "dGVzdC1wcml2YXRlLWtleS1ieXRlcw",
        "subject:not-a-domain-uuid",
    ] {
        assert!(
            matches!(
                PortableReference::new(excluded),
                Err(PortableMemoryError::SensitiveMaterial)
            ),
            "{excluded} must not satisfy the owning-domain reference grammar"
        );
    }
    let mut unsafe_rule = filing_rule_definition();
    unsafe_rule.service_provider = "Bearer portable-provider-token".to_owned();
    assert!(matches!(
        store.append(
            household_id,
            cabinet.path(),
            PortableEventDraft {
                event_id: event_id("unsafe-portable-rule"),
                sequence: 1,
                previous_event_digest: None,
                supersedes_event_digest: None,
                occurred_at: "2026-07-24T17:45:00+10:00".to_owned(),
                conversation_reference: None,
                fact: PortableFact::FilingRule {
                    rule_reference: reference("filing-rule-unsafe"),
                    state: PortableFilingRuleState::Active,
                    definition: unsafe_rule,
                },
            },
        ),
        Err(PortableMemoryError::InvalidEvent)
    ));
    assert!(!cabinet.path().join(".luna-memory").exists());
    assert!(store
        .list_events(household_id)
        .expect("list local portable events")
        .is_empty());
}

#[test]
fn synchronization_rebuilds_an_owning_filing_rule_that_handles_a_new_document() {
    let household_id = "rivera-household";
    let cabinet = tempfile::tempdir().expect("create the user-owned Cabinet");
    fs::create_dir_all(cabinet.path().join("Household records"))
        .expect("create the filing section");
    let first_local = tempfile::tempdir().expect("create the first local database");
    let second_local = tempfile::tempdir().expect("create the recovered local database");
    let (first_device, second_device, first_authorization, _) = enrol_two_devices(household_id);
    let document_text = "Document Type: Electricity bill; Service Provider: AGL Energy; \
        Addressee: Sam Rivera; Property: 12 Seabreeze Avenue; Account: 12345678; \
        Relevant Date: 2026-07-15";

    let first_database = first_local.path().join("luna.db");
    let first_conversations = ConversationStore::open_with_ocr(
        &first_database,
        first_device.clone(),
        FixedLocalOcr(document_text),
    )
    .expect("open Conversation behavior on the first device");
    let first_intelligence = CloudIntelligenceStore::open(&first_database, first_device.clone())
        .expect("open Cloud Assistance behavior on the first device");
    first_intelligence
        .set_provider_credential(
            household_id,
            "openai-byok",
            b"sk-live-must-remain-in-the-device-vault",
        )
        .expect("store a provider credential only in the device vault");
    let conversation = first_conversations
        .create_conversation(household_id, "Electricity bills")
        .expect("create a Conversation");
    let first_source = first_local.path().join("agl-july.png");
    fs::write(&first_source, png_fixture()).expect("write the first document");
    let arrival = first_conversations
        .attach_document(household_id, conversation.id, &first_source, cabinet.path())
        .expect("attach the first document");
    first_conversations
        .record_member_direction(
            household_id,
            arrival.id,
            DocumentContextDirection {
                document_type: Some("Electricity bill".to_owned()),
                document_type_resolved: true,
                service_provider: Some("AGL Energy".to_owned()),
                service_provider_resolved: true,
                addressee: Some("Sam Rivera".to_owned()),
                addressee_resolved: true,
                property: Some("12 Seabreeze Avenue".to_owned()),
                property_resolved: true,
                account: Some("12345678".to_owned()),
                account_resolved: true,
                amount: None,
                amount_resolved: true,
                relevant_dates: vec!["2026-07-15".to_owned()],
                relevant_dates_resolved: true,
                service_provider_relevance: Some(ContextRelevanceDirection {
                    subject: "AGL Energy".to_owned(),
                    explanation: "Supplies electricity to the Household".to_owned(),
                }),
                property_relevance: Some(ContextRelevanceDirection {
                    subject: "12 Seabreeze Avenue".to_owned(),
                    explanation: "The Household's primary residence".to_owned(),
                }),
            },
            "Household records",
        )
        .expect("record the owning Member Direction");
    first_conversations
        .confirm_filing_decision(
            household_id,
            arrival.id,
            FilingDecisionDirection {
                file_name: "AGL bill July 2026.png".to_owned(),
                cabinet_destination: "Household records/AGL bill July 2026.png".to_owned(),
            },
        )
        .expect("confirm the Filing Decision");
    first_conversations
        .file_document(household_id, arrival.id, cabinet.path())
        .expect("file the first document");
    first_conversations
        .learn_filing_rule(household_id, arrival.id)
        .expect("explicitly learn the filing rule");

    let first_memory = PortableMemoryStore::open(&first_database, first_device)
        .expect("open portable memory on the first device");
    first_memory
        .capture_owned_state(
            household_id,
            cabinet.path(),
            &first_conversations,
            &first_intelligence,
        )
        .expect("capture facts produced by owning behavior");
    let portable_plaintext = serde_json::to_string(
        &first_memory
            .list_events(household_id)
            .expect("open locally protected portable events"),
    )
    .expect("serialize portable events for the secret-boundary assertion");
    assert!(!portable_plaintext.contains("sk-live-must-remain"));

    let second_database = second_local.path().join("luna.db");
    let second_conversations = ConversationStore::open_with_ocr(
        &second_database,
        second_device.clone(),
        FixedLocalOcr(document_text),
    )
    .expect("open Conversation behavior on the recovered device");
    let second_intelligence = CloudIntelligenceStore::open(&second_database, second_device.clone())
        .expect("open Cloud Assistance behavior on the recovered device");
    let second_memory = PortableMemoryStore::open(&second_database, second_device)
        .expect("open portable memory on the recovered device");
    second_memory
        .synchronize_owned_state(
            household_id,
            cabinet.path(),
            &[first_authorization],
            &second_conversations,
            &second_intelligence,
        )
        .expect("rebuild the owning stores from portable memory");

    let rebuilt_rules = second_conversations
        .list_filing_rules(household_id)
        .expect("list rebuilt Filing Rules");
    assert_eq!(rebuilt_rules.len(), 1);
    assert_eq!(rebuilt_rules[0].service_provider, "AGL Energy");
    assert!(
        !second_memory
            .list_portable_history(household_id)
            .expect("list rebuilt History")
            .is_empty(),
        "the recovered device should expose imported Audit History",
    );

    let recovered_conversation = second_conversations
        .create_conversation(household_id, "August electricity bill")
        .expect("create a Conversation on the recovered device");
    let second_source = second_local.path().join("agl-august.png");
    fs::write(&second_source, png_fixture()).expect("write a new matching document");
    let automatically_filed = second_conversations
        .attach_document(
            household_id,
            recovered_conversation.id,
            &second_source,
            cabinet.path(),
        )
        .expect("handle the new document with the rebuilt rule");
    assert_eq!(
        automatically_filed.processing_state,
        DocumentProcessingState::Filed,
        "the imported rule must be active owning behavior, not a display-only projection",
    );
}

#[test]
fn synchronization_rebuilds_a_reusable_consent_grant_that_authorizes_matching_evidence() {
    let household_id = "rivera-household";
    let cabinet = tempfile::tempdir().expect("create the user-owned Cabinet");
    let first_local = tempfile::tempdir().expect("create the first local database");
    let second_local = tempfile::tempdir().expect("create the recovered local database");
    let (first_device, second_device, first_authorization, _) = enrol_two_devices(household_id);
    let provider = IntelligenceProviderDescriptor {
        id: "openai".to_owned(),
        name: "OpenAI".to_owned(),
        description: "Managed Cloud Assistance".to_owned(),
        models: vec![IntelligenceModelDescriptor {
            id: "gpt-5.6-luna".to_owned(),
            name: "GPT-5.6 Luna".to_owned(),
        }],
        managed_by_luna: true,
        auth_url: None,
    };
    let selection = IntelligenceSelection {
        provider_id: "openai".to_owned(),
        model_id: "gpt-5.6-luna".to_owned(),
    };
    let scope_evidence = vec![IntelligenceEvidence {
        field: "mediaType".to_owned(),
        value: "image/png".to_owned(),
        source: "local-document-metadata".to_owned(),
    }];

    let first_database = first_local.path().join("luna.db");
    let first_conversations = ConversationStore::open(&first_database, first_device.clone())
        .expect("open the first Conversation owner");
    let first_intelligence = CloudIntelligenceStore::open_with_gateway(
        &first_database,
        first_device.clone(),
        DeterministicIntelligenceGateway::new(
            "openai",
            "gpt-5.6-luna",
            BTreeMap::from([("documentType".to_owned(), "Electricity bill".to_owned())]),
        ),
        vec![provider.clone()],
    )
    .expect("open the first Consent owner");
    first_intelligence
        .grant_scope(
            household_id,
            &selection,
            IntelligenceCapability::DirectionInterpretation,
            "Evaluate a difficult document",
            vec!["documentType".to_owned(), "mediaType".to_owned()],
            scope_evidence.clone(),
            "account-sam",
        )
        .expect("grant reusable consent");
    let first_memory = PortableMemoryStore::open(&first_database, first_device)
        .expect("open portable memory on the first device");
    first_memory
        .capture_owned_state(
            household_id,
            cabinet.path(),
            &first_conversations,
            &first_intelligence,
        )
        .expect("capture the reusable Consent Grant");

    let second_database = second_local.path().join("luna.db");
    let second_conversations = ConversationStore::open(&second_database, second_device.clone())
        .expect("open the recovered Conversation owner");
    let second_intelligence = CloudIntelligenceStore::open_with_gateway(
        &second_database,
        second_device.clone(),
        DeterministicIntelligenceGateway::new(
            "openai",
            "gpt-5.6-luna",
            BTreeMap::from([("documentType".to_owned(), "Electricity bill".to_owned())]),
        ),
        vec![provider],
    )
    .expect("open the recovered Consent owner");
    let second_memory = PortableMemoryStore::open(&second_database, second_device)
        .expect("open portable memory on the recovered device");
    second_memory
        .synchronize_owned_state(
            household_id,
            cabinet.path(),
            &[first_authorization],
            &second_conversations,
            &second_intelligence,
        )
        .expect("rebuild reusable Consent on the recovered device");

    let recovered_scope = second_intelligence
        .list_consent_scopes(household_id)
        .expect("list recovered Consent Grants")
        .into_iter()
        .next()
        .expect("recover the reusable Consent Grant");
    assert_eq!(recovered_scope.kind, luna_core::ConsentGrantKind::Reusable);
    assert_eq!(recovered_scope.fields, vec!["documentType", "mediaType"]);
    assert!(recovered_scope.future_scope.is_some());

    let result = second_intelligence
        .evaluate_document(
            household_id,
            selection,
            IntelligenceRequest {
                request_id: "recovered-consent-request".to_owned(),
                document_arrival_id: "arrival-recovered".to_owned(),
                capability: IntelligenceCapability::DirectionInterpretation,
                provider_id: "openai".to_owned(),
                model_id: "gpt-5.6-luna".to_owned(),
                evidence: scope_evidence,
                content_excerpts: Vec::new(),
                expected_response: IntelligenceResponseSchema {
                    allowed_fields: vec!["documentType".to_owned()],
                    allow_candidate_direction: true,
                },
                consent_grant_id: Some(recovered_scope.id),
                constraints: IntelligenceExecutionConstraints {
                    timeout_ms: 1_000,
                    max_output_tokens: 64,
                },
            },
            CloudConsentDecision::UseExistingScope,
            "account-sam",
            Some(recovered_scope.id),
        )
        .expect("use the recovered reusable Consent Grant");
    assert_eq!(
        result.fields.get("documentType").map(String::as_str),
        Some("Electricity bill"),
    );
}

#[test]
fn owning_filing_records_cabinet_unavailability_locally_and_captures_the_resumed_result() {
    let household_id = "rivera-household";
    let cabinet = tempfile::tempdir().expect("create the user-owned Cabinet");
    fs::create_dir_all(cabinet.path().join("Household records"))
        .expect("create the filing section");
    let local = tempfile::tempdir().expect("create the local database");
    let (device, _, _, _) = enrol_two_devices(household_id);
    let database = local.path().join("luna.db");
    let conversations = ConversationStore::open_with_ocr(
        &database,
        device.clone(),
        FixedLocalOcr(
            "Document Type: Electricity bill; Service Provider: AGL Energy; \
             Addressee: Sam Rivera; Property: 12 Seabreeze Avenue; Account: 12345678; \
             Relevant Date: 2026-07-15",
        ),
    )
    .expect("open Conversation behavior");
    let intelligence = CloudIntelligenceStore::open(&database, device.clone())
        .expect("open Cloud Assistance behavior");
    let conversation = conversations
        .create_conversation(household_id, "Unavailable Cabinet")
        .expect("create a Conversation");
    let source = local.path().join("unavailable-cabinet.png");
    fs::write(&source, png_fixture()).expect("write the document");
    let arrival = conversations
        .attach_document(household_id, conversation.id, &source, cabinet.path())
        .expect("attach the document");
    conversations
        .record_member_direction(
            household_id,
            arrival.id,
            DocumentContextDirection {
                document_type: Some("Electricity bill".to_owned()),
                document_type_resolved: true,
                service_provider: Some("AGL Energy".to_owned()),
                service_provider_resolved: true,
                addressee: Some("Sam Rivera".to_owned()),
                addressee_resolved: true,
                property: Some("12 Seabreeze Avenue".to_owned()),
                property_resolved: true,
                account: Some("12345678".to_owned()),
                account_resolved: true,
                amount: None,
                amount_resolved: true,
                relevant_dates: vec!["2026-07-15".to_owned()],
                relevant_dates_resolved: true,
                service_provider_relevance: Some(ContextRelevanceDirection {
                    subject: "AGL Energy".to_owned(),
                    explanation: "Supplies electricity to the Household".to_owned(),
                }),
                property_relevance: Some(ContextRelevanceDirection {
                    subject: "12 Seabreeze Avenue".to_owned(),
                    explanation: "The Household's primary residence".to_owned(),
                }),
            },
            "Household records",
        )
        .expect("record Member Direction");
    conversations
        .confirm_filing_decision(
            household_id,
            arrival.id,
            FilingDecisionDirection {
                file_name: "AGL bill unavailable.png".to_owned(),
                cabinet_destination: "Household records/AGL bill unavailable.png".to_owned(),
            },
        )
        .expect("confirm the Filing Decision");
    let unavailable_root = local.path().join("missing-cabinet");
    fs::write(
        &unavailable_root,
        b"this path is deliberately not a directory",
    )
    .expect("create an unavailable Cabinet path");
    let unavailable = conversations
        .file_document(household_id, arrival.id, &unavailable_root)
        .expect("stage the filing retry without redirecting the Cabinet");
    assert_eq!(
        unavailable.processing_state,
        DocumentProcessingState::CabinetUnavailable,
    );

    let memory =
        PortableMemoryStore::open(&database, device).expect("open portable memory locally");
    assert!(matches!(
        memory.capture_owned_state(
            household_id,
            &unavailable_root,
            &conversations,
            &intelligence,
        ),
        Err(PortableMemoryError::CabinetUnavailable)
    ));
    assert!(memory
        .list_events(household_id)
        .expect("read local-first portable events")
        .iter()
        .any(|event| matches!(
            event.fact,
            PortableFact::ExecutionOutcome {
                outcome: PortableExecutionOutcomeKind::CabinetUnavailable,
                ..
            }
        )));

    conversations
        .resume_document_filings(household_id, cabinet.path())
        .expect("resume filing when the Cabinet returns");
    memory
        .capture_owned_state(household_id, cabinet.path(), &conversations, &intelligence)
        .expect("capture the resumed filing result");
    let projection = memory
        .household_projection(household_id)
        .expect("project the resumed filing result");
    assert!(projection.execution_outcomes.iter().any(|event| matches!(
        event.fact,
        PortableFact::ExecutionOutcome {
            outcome: PortableExecutionOutcomeKind::FiledAndVerified,
            ..
        }
    )));
    assert!(projection.audit_events.iter().any(|event| matches!(
        event.fact,
        PortableFact::AuditEvent {
            event_kind: PortableAuditEventKind::DocumentFiled,
            outcome: PortableExecutionOutcomeKind::FiledAndVerified,
            ..
        }
    )));
}

#[test]
fn owning_cloud_denial_and_provider_failure_become_portable_resilience_history() {
    let household_id = "rivera-household";
    let cabinet = tempfile::tempdir().expect("create the user-owned Cabinet");
    let local = tempfile::tempdir().expect("create the local database");
    let (device, _, _, _) = enrol_two_devices(household_id);
    let database = local.path().join("luna.db");
    let conversations =
        ConversationStore::open(&database, device.clone()).expect("open Conversation behavior");
    let gateway = DeterministicIntelligenceGateway::new("openai", "gpt-5.6-luna", BTreeMap::new());
    let intelligence = CloudIntelligenceStore::open_with_gateway(
        &database,
        device.clone(),
        gateway.clone(),
        vec![IntelligenceProviderDescriptor {
            id: "openai".to_owned(),
            name: "OpenAI".to_owned(),
            description: "Managed Cloud Assistance".to_owned(),
            models: vec![IntelligenceModelDescriptor {
                id: "gpt-5.6-luna".to_owned(),
                name: "GPT-5.6 Luna".to_owned(),
            }],
            managed_by_luna: true,
            auth_url: None,
        }],
    )
    .expect("open Cloud Assistance behavior");
    let selection = IntelligenceSelection {
        provider_id: "openai".to_owned(),
        model_id: "gpt-5.6-luna".to_owned(),
    };
    let request = |request_id: &str| IntelligenceRequest {
        request_id: request_id.to_owned(),
        document_arrival_id: format!(
            "arrival-{}",
            match request_id {
                "denied" => 1,
                "provider-failure" => 2,
                "completed" => 3,
                _ => 4,
            }
        ),
        capability: IntelligenceCapability::DirectionInterpretation,
        provider_id: "openai".to_owned(),
        model_id: "gpt-5.6-luna".to_owned(),
        evidence: Vec::new(),
        content_excerpts: Vec::new(),
        expected_response: IntelligenceResponseSchema {
            allowed_fields: Vec::new(),
            allow_candidate_direction: false,
        },
        consent_grant_id: None,
        constraints: IntelligenceExecutionConstraints {
            timeout_ms: 1_000,
            max_output_tokens: 64,
        },
    };

    assert_eq!(
        intelligence.evaluate_document(
            household_id,
            selection.clone(),
            request("denied"),
            CloudConsentDecision::KeepLocal,
            "account-sam",
            None,
        ),
        Err(IntelligenceFailure::LocalOnlyPolicy),
    );
    gateway.fail_next(IntelligenceFailure::ProviderUnavailable);
    gateway.fail_next(IntelligenceFailure::ProviderUnavailable);
    assert_eq!(
        intelligence.evaluate_document(
            household_id,
            selection.clone(),
            request("provider-failure"),
            CloudConsentDecision::AllowOnce,
            "account-sam",
            None,
        ),
        Err(IntelligenceFailure::ProviderUnavailable),
    );
    intelligence
        .evaluate_document(
            household_id,
            selection,
            request("completed"),
            CloudConsentDecision::AllowOnce,
            "account-sam",
            None,
        )
        .expect("complete a deterministic Cloud Assistance request");
    intelligence
        .record_candidate_disposition(household_id, "completed", CandidateDisposition::Accepted)
        .expect("record the member-accepted candidate");

    let memory = PortableMemoryStore::open(&database, device).expect("open portable memory");
    memory
        .capture_owned_state(household_id, cabinet.path(), &conversations, &intelligence)
        .expect("capture owning Cloud Assistance outcomes");
    let projection = memory
        .household_projection(household_id)
        .expect("project portable resilience history");
    assert!(!projection.consent_grants.iter().any(|event| matches!(
        event.fact,
        PortableFact::ConsentGrant {
            state: PortableConsentState::Denied,
            ..
        }
    )));
    assert!(projection.consent_grants.iter().all(|event| matches!(
        event.fact,
        PortableFact::ConsentGrant {
            scope: PortableConsentScope {
                document_reference: Some(_),
                ..
            },
            details: PortableConsentDetails {
                kind: PortableConsentGrantKind::OneTime,
                ..
            },
            ..
        }
    )));
    assert!(projection.execution_outcomes.iter().any(|event| matches!(
        event.fact,
        PortableFact::ExecutionOutcome {
            outcome: PortableExecutionOutcomeKind::ProviderUnavailable,
            ..
        }
    )));
    assert!(projection.execution_outcomes.iter().any(|event| matches!(
        event.fact,
        PortableFact::ExecutionOutcome {
            outcome: PortableExecutionOutcomeKind::KeptLocal,
            ..
        }
    )));
    assert!(projection.execution_outcomes.iter().any(|event| matches!(
        event.fact,
        PortableFact::ExecutionOutcome {
            outcome: PortableExecutionOutcomeKind::CloudAssistanceCompleted,
            ..
        }
    )));
    assert!(projection.audit_events.iter().any(|event| matches!(
        event.fact,
        PortableFact::AuditEvent {
            candidate_disposition: Some(PortableCandidateDisposition::Accepted),
            authority: PortableAuthority::MemberDirection,
            ..
        }
    )));
    assert!(projection.audit_events.iter().any(|event| matches!(
        event.fact,
        PortableFact::AuditEvent {
            outcome: PortableExecutionOutcomeKind::ProviderUnavailable,
            authority: PortableAuthority::ConsentGrant,
            ..
        }
    )));
    assert!(projection.audit_events.iter().any(|event| matches!(
        event.fact,
        PortableFact::AuditEvent {
            outcome: PortableExecutionOutcomeKind::KeptLocal,
            authority: PortableAuthority::MemberDirection,
            ..
        }
    )));
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
                event_id: event_id("rule-event-paused"),
                sequence: 1,
                previous_event_digest: None,
                supersedes_event_digest: None,
                occurred_at: "2026-07-24T18:00:00+10:00".to_owned(),
                conversation_reference: None,
                fact: PortableFact::FilingRule {
                    rule_reference: reference("filing-rule-7"),
                    state: PortableFilingRuleState::Paused,
                    definition: filing_rule_definition(),
                },
            },
        )
        .expect("append the first Filing Rule event");
    let deleted = second_store
        .append(
            household_id,
            cabinet.path(),
            PortableEventDraft {
                event_id: event_id("rule-event-deleted"),
                sequence: 1,
                previous_event_digest: None,
                supersedes_event_digest: None,
                occurred_at: "2026-07-24T18:00:01+10:00".to_owned(),
                conversation_reference: None,
                fact: PortableFact::FilingRule {
                    rule_reference: reference("filing-rule-7"),
                    state: PortableFilingRuleState::Deleted,
                    definition: filing_rule_definition(),
                },
            },
        )
        .expect("append a concurrent Filing Rule event");

    let report = second_store
        .import(
            household_id,
            cabinet.path(),
            &[first_authorization.clone(), second_authorization.clone()],
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
    assert_eq!(
        conflict.subject_reference,
        filing_rule_subject("filing-rule-7")
    );

    let first_conflict_report = first_store
        .import(
            household_id,
            cabinet.path(),
            &[first_authorization.clone(), second_authorization.clone()],
        )
        .expect("make the same base conflict visible on the first device");
    assert_eq!(first_conflict_report.conflicts.len(), 1);
    let first_conflict = first_conflict_report.conflicts[0].clone();

    let paused_resolution = second_store
        .resolve_conflict(
            household_id,
            cabinet.path(),
            conflict.id,
            PortableConflictResolutionDraft {
                event_id: event_id("conflict-resolution-1"),
                sequence: 2,
                previous_event_digest: Some(
                    second_store
                        .list_events(household_id)
                        .expect("list the recovered device chain")
                        .into_iter()
                        .find(|event| event.signer_device_id == second_authorization.device_id)
                        .expect("find the recovered device event")
                        .digest,
                ),
                occurred_at: "2026-07-24T18:01:00+10:00".to_owned(),
                chosen_event_id: paused.event_id.clone(),
            },
        )
        .expect("choose the retained Filing Rule event");
    let deleted_resolution = first_store
        .resolve_conflict(
            household_id,
            cabinet.path(),
            first_conflict.id,
            PortableConflictResolutionDraft {
                event_id: event_id("conflict-resolution-2"),
                sequence: 2,
                previous_event_digest: Some(paused.digest.clone()),
                occurred_at: "2026-07-24T18:01:01+10:00".to_owned(),
                chosen_event_id: deleted.event_id.clone(),
            },
        )
        .expect("record the concurrent opposing resolution");

    let opposing = second_store
        .import(
            household_id,
            cabinet.path(),
            &[first_authorization.clone(), second_authorization.clone()],
        )
        .expect("opposing signed resolutions should import as a new detectable conflict");
    assert_eq!(opposing.imported, 1);
    assert_eq!(opposing.conflicts.len(), 1);
    let resolution_conflict = &opposing.conflicts[0];
    assert!(resolution_conflict
        .subject_reference
        .starts_with("conflictResolution:"));
    assert!(
        second_store
            .household_projection(household_id)
            .expect("read the projection while resolutions conflict")
            .filing_rules
            .is_empty(),
        "an opposing resolution must not leave one Filing Rule silently active"
    );

    second_store
        .resolve_conflict(
            household_id,
            cabinet.path(),
            resolution_conflict.id,
            PortableConflictResolutionDraft {
                event_id: event_id("conflict-resolution-final"),
                sequence: 3,
                previous_event_digest: Some(paused_resolution.digest.clone()),
                occurred_at: "2026-07-24T18:02:00+10:00".to_owned(),
                chosen_event_id: deleted_resolution.event_id.clone(),
            },
        )
        .expect("resolve the competing resolution events in favor of the second resolution");
    let final_report = first_store
        .import(
            household_id,
            cabinet.path(),
            &[first_authorization, second_authorization],
        )
        .expect("replay both resolutions and their final resolution");
    assert!(final_report.conflicts.is_empty());
    assert!(first_store
        .list_conflicts(household_id)
        .expect("list first-device conflicts after final portable resolution")
        .is_empty());
    assert_eq!(
        first_store
            .current_subject_event(household_id, &filing_rule_subject("filing-rule-7"))
            .expect("read the rebuilt first-device Filing Rule projection")
            .expect("the resolved Filing Rule event should exist")
            .event_id,
        deleted.event_id
    );

    first_store
        .append(
            household_id,
            cabinet.path(),
            PortableEventDraft {
                event_id: event_id("conflict-resolution-late"),
                sequence: 3,
                previous_event_digest: Some(deleted_resolution.digest),
                supersedes_event_digest: None,
                occurred_at: "2026-07-24T18:03:00+10:00".to_owned(),
                conversation_reference: None,
                fact: PortableFact::ConflictResolution {
                    existing_event_id: PortableReference::new(first_conflict.existing_event_id)
                        .expect("the conflict should contain a portable event reference"),
                    conflicting_event_id: PortableReference::new(
                        first_conflict.conflicting_event_id,
                    )
                    .expect("the conflict should contain a portable event reference"),
                    chosen_event_id: PortableReference::new(paused.event_id)
                        .expect("the chosen event should be a portable event reference"),
                },
            },
        )
        .expect("record a later resolution against the now-authoritative second choice");
    let late_conflicts = first_store
        .list_conflicts(household_id)
        .expect("surface the later competing resolution");
    assert_eq!(late_conflicts.len(), 1);
    assert!(late_conflicts[0]
        .subject_reference
        .starts_with("conflictResolution:"));
}

#[test]
fn import_rebuilds_causal_subject_updates_even_when_the_update_file_sorts_first() {
    let household_id = "rivera-household";
    let cabinet = tempfile::tempdir().expect("create a temporary Cabinet");
    let first_local = tempfile::tempdir().expect("create the first local database directory");
    let second_local = tempfile::tempdir().expect("create the second local database directory");
    let rebuilt_local = tempfile::tempdir().expect("create a rebuilt local database directory");
    let (first_device, second_device, first_authorization, second_authorization) =
        enrol_two_devices(household_id);
    let rebuilt_device = second_device.clone();
    let first_store = PortableMemoryStore::open(first_local.path().join("luna.db"), first_device)
        .expect("open portable memory on the first device");
    let second_store =
        PortableMemoryStore::open(second_local.path().join("luna.db"), second_device)
            .expect("open portable memory on the second device");
    let rebuilt_store =
        PortableMemoryStore::open(rebuilt_local.path().join("luna.db"), rebuilt_device)
            .expect("open empty portable memory for a causal rebuild");

    let base = first_store
        .append(
            household_id,
            cabinet.path(),
            PortableEventDraft {
                event_id: event_id("causal-update"),
                sequence: 1,
                previous_event_digest: None,
                supersedes_event_digest: None,
                occurred_at: "2026-07-24T18:05:00+10:00".to_owned(),
                conversation_reference: None,
                fact: PortableFact::FilingRule {
                    rule_reference: reference("filing-rule-causal"),
                    state: PortableFilingRuleState::Active,
                    definition: filing_rule_definition(),
                },
            },
        )
        .expect("append the causal base event");
    second_store
        .import(
            household_id,
            cabinet.path(),
            std::slice::from_ref(&first_authorization),
        )
        .expect("import the causal base before updating it");
    let update = second_store
        .append(
            household_id,
            cabinet.path(),
            PortableEventDraft {
                event_id: event_id("causal-base"),
                sequence: 1,
                previous_event_digest: None,
                supersedes_event_digest: Some(base.digest),
                occurred_at: "2026-07-24T18:06:00+10:00".to_owned(),
                conversation_reference: None,
                fact: PortableFact::FilingRule {
                    rule_reference: reference("filing-rule-causal"),
                    state: PortableFilingRuleState::Paused,
                    definition: filing_rule_definition(),
                },
            },
        )
        .expect("append the causal update");

    let mut event_paths = fs::read_dir(
        cabinet
            .path()
            .join(".luna-memory")
            .join("v1")
            .join("events"),
    )
    .expect("read causal event files")
    .map(|entry| entry.expect("read a causal event file").file_name())
    .collect::<Vec<_>>();
    event_paths.sort();
    assert_eq!(event_paths.len(), 2);
    assert!(
        event_paths[0]
            .to_string_lossy()
            .starts_with(&format!("{:x}", Sha256::digest(update.event_id.as_bytes()))),
        "the update file should sort before its dependency for this regression"
    );

    let report = rebuilt_store
        .import(
            household_id,
            cabinet.path(),
            &[first_authorization, second_authorization],
        )
        .expect("rebuild events in dependency order rather than filename order");
    assert_eq!(report.imported, 2);
    assert!(report.conflicts.is_empty());
    assert_eq!(
        rebuilt_store
            .current_subject_event(household_id, &filing_rule_subject("filing-rule-causal"),)
            .expect("read the causally rebuilt Filing Rule")
            .expect("the rebuilt Filing Rule should exist"),
        update
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
                event_id: event_id("audit-event-first"),
                sequence: 1,
                previous_event_digest: None,
                supersedes_event_digest: None,
                occurred_at: "2026-07-24T18:10:00+10:00".to_owned(),
                conversation_reference: None,
                fact: PortableFact::AuditEvent {
                    event_kind: PortableAuditEventKind::DocumentFiled,
                    authority: PortableAuthority::MemberDirection,
                    subject_reference: reference("document-arrival-55"),
                    outcome: PortableExecutionOutcomeKind::FiledAndVerified,
                    candidate_disposition: None,
                },
            },
        )
        .expect("append the first device event");

    let error = store
        .append(
            household_id,
            cabinet.path(),
            PortableEventDraft {
                event_id: event_id("audit-event-replayed"),
                sequence: 2,
                previous_event_digest: Some("0".repeat(64)),
                supersedes_event_digest: None,
                occurred_at: "2026-07-24T18:11:00+10:00".to_owned(),
                conversation_reference: None,
                fact: PortableFact::AuditEvent {
                    event_kind: PortableAuditEventKind::DocumentFiled,
                    authority: PortableAuthority::MemberDirection,
                    subject_reference: reference("document-arrival-56"),
                    outcome: PortableExecutionOutcomeKind::FiledAndVerified,
                    candidate_disposition: None,
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
fn revoked_device_authorization_accepts_history_only_through_its_signed_cutoff() {
    let household_id = "rivera-household";
    let cabinet = tempfile::tempdir().expect("create a temporary Cabinet");
    let first_local = tempfile::tempdir().expect("create the first local database directory");
    let second_local = tempfile::tempdir().expect("create the second local database directory");
    let (first_device, second_device, first_authorization, _) = enrol_two_devices(household_id);
    let revoked_device = first_device.clone();
    let retained_device = second_device.clone();
    let first_store = PortableMemoryStore::open(first_local.path().join("luna.db"), first_device)
        .expect("open portable memory on the device that will be revoked");
    let second_store =
        PortableMemoryStore::open(second_local.path().join("luna.db"), second_device)
            .expect("open portable memory on a retained device");
    revoked_device
        .set_current_key_epoch(household_id, 2)
        .expect("advance to the cutoff key epoch");
    retained_device
        .set_current_key_epoch(household_id, 2)
        .expect("give the retained device the cutoff key epoch");
    let before_revocation = first_store
        .append(
            household_id,
            cabinet.path(),
            PortableEventDraft {
                event_id: event_id("revocation-cutoff"),
                sequence: 1,
                previous_event_digest: None,
                supersedes_event_digest: None,
                occurred_at: "2026-07-24T18:15:00+10:00".to_owned(),
                conversation_reference: None,
                fact: PortableFact::AuditEvent {
                    event_kind: PortableAuditEventKind::DocumentFiled,
                    authority: PortableAuthority::MemberDirection,
                    subject_reference: reference("document-before-revocation"),
                    outcome: PortableExecutionOutcomeKind::FiledAndVerified,
                    candidate_disposition: None,
                },
            },
        )
        .expect("append legitimate pre-revocation history");
    let revoked_authorization = TrustedDeviceAuthorization {
        revoked_after: Some(PortableAuthorizationCutoff {
            key_epoch: 2,
            sequence: 1,
            event_digest: before_revocation.digest.clone(),
        }),
        ..first_authorization
    };
    second_store
        .import(
            household_id,
            cabinet.path(),
            std::slice::from_ref(&revoked_authorization),
        )
        .expect("import legitimate history through the revocation cutoff");
    first_store
        .append(
            household_id,
            cabinet.path(),
            PortableEventDraft {
                event_id: event_id("after-revocation-forgery"),
                sequence: 2,
                previous_event_digest: Some(before_revocation.digest),
                supersedes_event_digest: None,
                occurred_at: "2026-07-24T18:16:00+10:00".to_owned(),
                conversation_reference: None,
                fact: PortableFact::AuditEvent {
                    event_kind: PortableAuditEventKind::DocumentFiled,
                    authority: PortableAuthority::MemberDirection,
                    subject_reference: reference("document-after-revocation"),
                    outcome: PortableExecutionOutcomeKind::FiledAndVerified,
                    candidate_disposition: None,
                },
            },
        )
        .expect("simulate a revoked device signing beyond its authorization cutoff");

    assert!(matches!(
        second_store.import(household_id, cabinet.path(), &[revoked_authorization],),
        Err(PortableMemoryError::UntrustedDevice)
    ));
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
                event_id: event_id("audit-event-tamper-target"),
                sequence: 1,
                previous_event_digest: None,
                supersedes_event_digest: None,
                occurred_at: "2026-07-24T18:20:00+10:00".to_owned(),
                conversation_reference: None,
                fact: PortableFact::AuditEvent {
                    event_kind: PortableAuditEventKind::DocumentFiled,
                    authority: PortableAuthority::MemberDirection,
                    subject_reference: reference("document-arrival-60"),
                    outcome: PortableExecutionOutcomeKind::FiledAndVerified,
                    candidate_disposition: None,
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
