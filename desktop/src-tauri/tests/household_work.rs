use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, Mutex},
};

use luna_core::{
    AvailableHouseholdTool, CloudIntelligenceStore, ConversationStore, CredentialVault,
    DeterministicIntelligenceGateway, HouseholdAdministrationMessage,
    HouseholdAdministrationRequest, HouseholdAdministrationSource, HouseholdContextItem,
    HouseholdWorkOperation, HouseholdWorkStatus, HouseholdWorkSummary,
    IntelligenceExecutionConstraints, IntelligenceModelDescriptor, IntelligenceProviderDescriptor,
    TrustedDeviceManager, VaultError,
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
        }),
        household_context: vec![HouseholdContextItem {
            category: "property".to_owned(),
            value: "12 Seabreeze Avenue".to_owned(),
            source_reference: "document-1".to_owned(),
        }],
        active_household_work: Vec::<HouseholdWorkSummary>::new(),
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
