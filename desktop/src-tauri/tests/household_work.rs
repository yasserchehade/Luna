use std::{
    collections::{BTreeMap, HashMap},
    fs,
    sync::{Arc, Mutex},
};

use luna_core::{
    ActionApproval, ActionExecution, AvailableHouseholdTool, CloudIntelligenceStore,
    ConversationStore, CredentialVault, DeterministicIntelligenceGateway, HouseholdActionProposal,
    HouseholdAdministrationMessage, HouseholdAdministrationRequest, HouseholdAdministrationResult,
    HouseholdAdministrationSource, HouseholdContextItem, HouseholdWork, HouseholdWorkKind,
    HouseholdWorkOperation, HouseholdWorkProposal, HouseholdWorkStatus, HouseholdWorkSummary,
    IntelligenceExecutionConstraints, IntelligenceModelDescriptor, IntelligenceProviderDescriptor,
    IntelligenceUsage, ProposedAction, ProposedActionKind, TrustedDeviceManager,
    ValidatedHouseholdWorkDirection, VaultError, WorkFact, WorkFactCertainty, WorkFactKey,
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

fn open_conversation_store(database: &std::path::Path) -> ConversationStore<MemoryCredentialVault> {
    let vault = MemoryCredentialVault::default();
    let trusted_device = TrustedDeviceManager::new(vault);
    let enrollment = trusted_device
        .enrol_first_device("rivera-household")
        .expect("enrol device");
    trusted_device
        .confirm_recovery_key(
            "rivera-household",
            &enrollment.recovery_key,
            &enrollment.recovery_envelope,
        )
        .expect("confirm recovery key");
    trusted_device
        .set_current_key_epoch("rivera-household", 1)
        .expect("set key epoch");
    trusted_device
        .configure_device_pin("rivera-household", "246810")
        .expect("unlock device");
    ConversationStore::open(database, trusted_device).expect("open store")
}

fn digital_pdf_with_text(text: &str) -> Vec<u8> {
    let content = format!("BT\n/F1 12 Tf\n72 720 Td\n({text}) Tj\nET\n");
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        "<< /Type /Page /Parent 2 0 R /Resources << /Font << /F1 5 0 R >> >> /MediaBox [0 0 612 792] /Contents 4 0 R >>".to_owned(),
        format!("<< /Length {} >>\nstream\n{content}endstream", content.len()),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned(),
    ];
    let mut pdf = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (index, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend_from_slice(format!("{} 0 obj\n{object}\nendobj\n", index + 1).as_bytes());
    }
    let xref_offset = pdf.len();
    pdf.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
    );
    for offset in offsets {
        pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_offset}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    pdf
}

#[test]
fn household_work_result_is_persisted_and_reused_for_follow_up_updates() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let database = temporary.path().join("luna.db");
    let vault = MemoryCredentialVault::default();
    let trusted_device = TrustedDeviceManager::new(vault.clone());
    let enrollment = trusted_device
        .enrol_first_device("rivera-household")
        .expect("enrol device");
    trusted_device
        .confirm_recovery_key(
            "rivera-household",
            &enrollment.recovery_key,
            &enrollment.recovery_envelope,
        )
        .expect("confirm recovery key");
    trusted_device
        .set_current_key_epoch("rivera-household", 1)
        .expect("set key epoch");
    trusted_device
        .configure_device_pin("rivera-household", "246810")
        .expect("unlock device");
    let store = ConversationStore::open(&database, trusted_device.clone()).expect("open store");
    let conversation = store
        .create_conversation("rivera-household", "Uploaded bill")
        .expect("create conversation");
    let gateway = DeterministicIntelligenceGateway::new(
        "openai",
        "gpt-4.1-mini",
        BTreeMap::from([
            ("serviceProvider".to_owned(), "AGL".to_owned()),
            ("property".to_owned(), "12 Seabreeze Avenue".to_owned()),
            ("amount".to_owned(), "$184.72".to_owned()),
        ]),
    );
    let intelligence = CloudIntelligenceStore::open_with_gateway(
        &database,
        trusted_device,
        gateway.clone(),
        vec![IntelligenceProviderDescriptor {
            id: "openai".to_owned(),
            name: "OpenAI".to_owned(),
            description: "Deterministic test route".to_owned(),
            models: vec![IntelligenceModelDescriptor {
                id: "gpt-4.1-mini".to_owned(),
                name: "GPT-4.1 mini".to_owned(),
            }],
            managed_by_luna: true,
            auth_url: None,
        }],
    )
    .expect("open intelligence");
    let request = HouseholdAdministrationRequest {
        request_id: "request-1".to_owned(),
        conversation_id: conversation.id,
        current_message: "Take care of this.".to_owned(),
        relevant_conversation: vec![HouseholdAdministrationMessage {
            author: "member".to_owned(),
            body: "Take care of this.".to_owned(),
        }],
        source: Some(HouseholdAdministrationSource {
            reference: "document-1".to_owned(),
            filename: "bill.pdf".to_owned(),
            media_type: "application/pdf".to_owned(),
            original_base64: "cGRm".to_owned(),
            extracted_text: Some("AGL $184.72".to_owned()),
            original_size_bytes: 3,
            extracted_text_truncated: false,
        }),
        household_context: vec![HouseholdContextItem {
            category: "property".to_owned(),
            value: "12 Seabreeze Avenue".to_owned(),
            source_reference: "document-1".to_owned(),
        }],
        active_household_work: Vec::<HouseholdWorkSummary>::new(),
        source_linked_household_work: None,
        available_tools: vec![AvailableHouseholdTool {
            name: "reminder".to_owned(),
            description: "Propose a reminder".to_owned(),
        }],
        authority_and_approval_constraints: "Luna validates proposals.".to_owned(),
        response_schema_version: "household-administration.v1".to_owned(),
        constraints: IntelligenceExecutionConstraints {
            timeout_ms: 1_000,
            max_output_tokens: 500,
        },
    };
    let result = intelligence
        .reason_about_household_administration("rivera-household", request.clone())
        .expect("reason about upload");
    assert_eq!(result.work.operation, HouseholdWorkOperation::Create);
    let work = store
        .apply_household_administration_result(
            "rivera-household",
            conversation.id,
            Some(1),
            &result,
            "1-1",
        )
        .expect("persist work")
        .expect("created work");
    assert_eq!(work.status, HouseholdWorkStatus::AwaitingApproval);
    assert!(work.facts.iter().any(|fact| fact.value == "AGL"));
    assert!(work.source_refs.contains(&"document-1".to_owned()));

    let mut follow_up = request;
    follow_up.request_id = "request-2".to_owned();
    follow_up.current_message = "I already paid it.".to_owned();
    follow_up.active_household_work = vec![HouseholdWorkSummary::from(&work)];
    let result = intelligence
        .reason_about_household_administration("rivera-household", follow_up)
        .expect("reason about completion");
    assert_eq!(result.work.operation, HouseholdWorkOperation::Update);
    assert_eq!(result.work.work_id.as_deref(), Some(work.id.as_str()));
    assert_eq!(result.work.status, Some(HouseholdWorkStatus::Completed));
    assert_eq!(
        result.validated_member_direction,
        Some(ValidatedHouseholdWorkDirection::Complete)
    );
    let completed = store
        .apply_household_administration_result(
            "rivera-household",
            conversation.id,
            Some(1),
            &result,
            "2-2",
        )
        .expect("persist completion")
        .expect("updated work");
    assert_eq!(completed.id, work.id);
    assert_eq!(completed.status, HouseholdWorkStatus::Completed);
    assert_eq!(
        store
            .list_household_work("rivera-household")
            .expect("list work")
            .len(),
        1
    );
    assert_eq!(gateway.requests().len(), 2);
}

#[test]
fn a_none_operation_preserves_existing_awaiting_approval_work_exactly() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let store = open_conversation_store(&temporary.path().join("luna.db"));
    let conversation = store
        .create_conversation("rivera-household", "Read-only question")
        .expect("create conversation");
    let mut work = HouseholdWork::new(
        "work-1",
        "rivera-household",
        HouseholdWorkKind::Bill,
        "Electricity bill",
        "1-1",
    );
    work.status = HouseholdWorkStatus::AwaitingApproval;
    work.facts.push(WorkFact {
        key: WorkFactKey::Amount,
        value: "$184.72".to_owned(),
        evidence_refs: vec!["document-1".to_owned()],
        certainty: WorkFactCertainty::Confirmed,
    });
    work.proposed_actions.push(ProposedAction {
        id: "work-1-action-1".to_owned(),
        kind: ProposedActionKind::Reminder,
        summary: "Remind the household before the due date.".to_owned(),
        arguments: BTreeMap::new(),
        approval: ActionApproval::Required,
        execution: ActionExecution::NotStarted,
    });
    store
        .save_household_work("rivera-household", &work)
        .expect("save awaiting approval work");
    let before = work.clone();
    let no_op = HouseholdAdministrationResult {
        request_id: "request-read-only".to_owned(),
        provider_id: "openai".to_owned(),
        model_id: "gpt-4.1-mini".to_owned(),
        reply: "The bill is due on 15 August.".to_owned(),
        work: HouseholdWorkProposal {
            operation: HouseholdWorkOperation::None,
            work_id: Some(work.id.clone()),
            kind: Some(HouseholdWorkKind::Other),
            summary: Some("This must not replace the summary".to_owned()),
            status: Some(HouseholdWorkStatus::Active),
            facts: vec![WorkFact {
                key: WorkFactKey::Provider,
                value: "Untrusted replacement".to_owned(),
                evidence_refs: vec!["document-1".to_owned()],
                certainty: WorkFactCertainty::Likely,
            }],
            due_at: None,
            urgency: None,
        },
        clarification: None,
        proposed_actions: vec![HouseholdActionProposal {
            kind: ProposedActionKind::DraftReply,
            summary: "Untrusted replacement action".to_owned(),
            arguments: BTreeMap::new(),
            approval_required: false,
        }],
        usage: IntelligenceUsage::default(),
        validated_member_direction: None,
    };

    let returned = store
        .apply_household_administration_result(
            "rivera-household",
            conversation.id,
            None,
            &no_op,
            "2-2",
        )
        .expect("apply read-only result")
        .expect("return existing work without changing it");
    assert_eq!(returned, before);
    assert_eq!(
        store
            .find_household_work("rivera-household", "work-1")
            .expect("reload work"),
        before
    );
}

#[test]
fn terminal_household_work_is_removed_from_attention_for_every_terminal_state() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let database = temporary.path().join("luna.db");
    let cabinet = temporary.path().join("Cabinet");
    fs::create_dir_all(cabinet.join("Incoming")).expect("create Incoming folder");
    let document = temporary.path().join("bill.pdf");
    fs::write(&document, digital_pdf_with_text("AGL electricity bill")).expect("write PDF fixture");
    let store = open_conversation_store(&database);
    let conversation = store
        .create_conversation("rivera-household", "Terminal attention")
        .expect("create conversation");
    let arrival = store
        .attach_document("rivera-household", conversation.id, &document, &cabinet)
        .expect("attach document");
    let mut work = HouseholdWork::new(
        "work-1",
        "rivera-household",
        HouseholdWorkKind::Bill,
        "Electricity bill",
        "1-1",
    );
    work.source_refs.push(format!("document-{}", arrival.id));
    store
        .save_household_work("rivera-household", &work)
        .expect("save active work");
    assert_eq!(
        store
            .list_todo_items("rivera-household")
            .expect("list active attention")
            .len(),
        1
    );

    for terminal_status in [
        HouseholdWorkStatus::Completed,
        HouseholdWorkStatus::Dismissed,
        HouseholdWorkStatus::NoLongerRelevant,
    ] {
        work.status = terminal_status;
        work.closed_at = Some("2-2".to_owned());
        store
            .save_household_work("rivera-household", &work)
            .expect("save terminal work");
        assert!(
            store
                .list_todo_items("rivera-household")
                .expect("list terminal attention")
                .is_empty(),
            "{terminal_status:?} must not remain in To do"
        );
    }
}
