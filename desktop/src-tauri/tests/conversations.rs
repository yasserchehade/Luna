use std::{
    collections::HashMap,
    fs,
    path::Path,
    sync::{Arc, Mutex},
};

use luna_core::{
    ConversationStore, CredentialVault, DocumentProcessingState, TrustedDeviceManager, VaultError,
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

type TestConversationStore = ConversationStore<MemoryCredentialVault>;

fn open_conversation_store(
    database: impl AsRef<Path>,
) -> (
    TestConversationStore,
    TrustedDeviceManager<MemoryCredentialVault>,
) {
    let household_id = "rivera-household";
    let trusted_device = TrustedDeviceManager::new(MemoryCredentialVault::default());
    let enrollment = trusted_device
        .enrol_first_device(household_id)
        .expect("enrol test Trusted Device");
    trusted_device
        .confirm_recovery_key(
            household_id,
            &enrollment.recovery_key,
            &enrollment.recovery_envelope,
        )
        .expect("confirm test Recovery Key");
    trusted_device
        .set_current_key_epoch(household_id, 1)
        .expect("set test key epoch");
    trusted_device
        .configure_device_pin(household_id, "246810")
        .expect("unlock test Trusted Device");
    let store =
        ConversationStore::open(database, trusted_device.clone()).expect("open Conversation store");
    (store, trusted_device)
}

#[test]
fn a_conversation_survives_reopening_the_local_database() {
    let directory = tempfile::tempdir().expect("temporary device directory");
    let database = directory.path().join("luna.db");
    let (store, trusted_device) = open_conversation_store(&database);

    let conversation = store
        .create_conversation("rivera-household", "Electricity bill")
        .expect("create Conversation");

    let reopened =
        ConversationStore::open(&database, trusted_device).expect("reopen Conversation store");
    assert_eq!(
        reopened
            .list_conversations("rivera-household", None, false)
            .expect("list Conversations"),
        vec![conversation]
    );
}

#[test]
fn a_member_can_rename_search_archive_and_delete_conversations() {
    let directory = tempfile::tempdir().expect("temporary device directory");
    let (store, _) = open_conversation_store(directory.path().join("luna.db"));
    let electricity = store
        .create_conversation("rivera-household", "Electricity bill")
        .expect("create electricity Conversation");
    let insurance = store
        .create_conversation("rivera-household", "Insurance renewal")
        .expect("create insurance Conversation");

    store
        .rename_conversation("rivera-household", electricity.id, "AGL electricity bill")
        .expect("rename Conversation");
    assert_eq!(
        store
            .list_conversations("rivera-household", Some("agl"), false)
            .expect("search Conversations")[0]
            .title,
        "AGL electricity bill"
    );

    store
        .archive_conversation("rivera-household", electricity.id, true)
        .expect("archive Conversation");
    assert_eq!(
        store
            .list_conversations("rivera-household", None, false)
            .expect("list active Conversations"),
        vec![insurance.clone()]
    );
    assert!(store
        .list_conversations("rivera-household", None, true)
        .expect("list archived Conversations")
        .iter()
        .any(|conversation| conversation.id == electricity.id && conversation.archived));

    store
        .delete_conversation("rivera-household", insurance.id)
        .expect("delete Conversation");
    assert!(!store
        .list_conversations("rivera-household", None, true)
        .expect("list after deletion")
        .iter()
        .any(|conversation| conversation.id == insurance.id));
}

#[test]
fn a_document_arrival_and_one_todo_survive_conversation_deletion() {
    let directory = tempfile::tempdir().expect("temporary device directory");
    let document = directory.path().join("AGL bill.pdf");
    fs::write(&document, b"%PDF-1.7 fixture").expect("write document fixture");
    let database = directory.path().join("luna.db");
    let (store, trusted_device) = open_conversation_store(&database);
    let conversation = store
        .create_conversation("rivera-household", "Electricity bill")
        .expect("create Conversation");

    let arrival = store
        .attach_document("rivera-household", conversation.id, &document)
        .expect("attach supported document");

    assert_eq!(arrival.original_name, "AGL bill.pdf");
    assert_eq!(
        arrival.processing_state,
        DocumentProcessingState::NeedsMemberDirection
    );
    let todos = store
        .list_todo_items("rivera-household")
        .expect("list To-do Items");
    assert_eq!(todos.len(), 1);
    assert_eq!(todos[0].arrival_id, arrival.id);
    assert_eq!(todos[0].conversation_id, conversation.id);

    store
        .delete_conversation("rivera-household", conversation.id)
        .expect("delete Conversation");
    let reopened =
        ConversationStore::open(&database, trusted_device).expect("reopen Conversation store");
    assert_eq!(
        reopened
            .list_document_arrivals("rivera-household")
            .expect("list durable Document Arrivals"),
        vec![arrival.clone()]
    );
    let todos = reopened
        .list_todo_items("rivera-household")
        .expect("list To-do Items after Conversation deletion");
    assert_eq!(todos.len(), 1);
    assert_eq!(todos[0].arrival_id, arrival.id);
    assert!(todos[0].conversation_deleted);
}

#[test]
fn messages_are_durable_parts_of_a_conversation() {
    let directory = tempfile::tempdir().expect("temporary device directory");
    let database = directory.path().join("luna.db");
    let (store, trusted_device) = open_conversation_store(&database);
    let conversation = store
        .create_conversation("rivera-household", "Electricity bill")
        .expect("create Conversation");

    let message = store
        .add_member_message(
            "rivera-household",
            conversation.id,
            "Please organise this electricity bill.",
        )
        .expect("add member message");

    let reopened =
        ConversationStore::open(&database, trusted_device).expect("reopen Conversation store");
    assert_eq!(
        reopened
            .list_messages("rivera-household", conversation.id)
            .expect("list Conversation messages"),
        vec![message]
    );
}

#[test]
fn resolving_work_from_any_surface_updates_the_same_document_handling_state() {
    let directory = tempfile::tempdir().expect("temporary device directory");
    let document = directory.path().join("rates.png");
    fs::write(&document, b"\x89PNG\r\n\x1a\nfixture").expect("write document fixture");
    let (store, _) = open_conversation_store(directory.path().join("luna.db"));
    let conversation = store
        .create_conversation("rivera-household", "Council rates")
        .expect("create Conversation");
    let arrival = store
        .attach_document("rivera-household", conversation.id, &document)
        .expect("attach supported document");

    store
        .dismiss_document_arrival("rivera-household", arrival.id)
        .expect("dismiss Document Handling");

    assert!(store
        .list_todo_items("rivera-household")
        .expect("list resolved To-do Items")
        .is_empty());
    assert_eq!(
        store
            .list_document_arrivals("rivera-household")
            .expect("list Document Arrivals")[0]
            .processing_state,
        DocumentProcessingState::Dismissed
    );
}

#[test]
fn conversation_work_is_scoped_to_its_household() {
    let directory = tempfile::tempdir().expect("temporary device directory");
    let (store, _) = open_conversation_store(directory.path().join("luna.db"));
    let conversation = store
        .create_conversation("rivera-household", "Private records")
        .expect("create Conversation");

    assert!(store
        .rename_conversation("other-household", conversation.id, "Exposed")
        .is_err());
    assert!(store
        .add_member_message("other-household", conversation.id, "Read this")
        .is_err());
    assert!(store
        .list_messages("other-household", conversation.id)
        .is_err());
}

#[test]
fn only_pdf_jpg_and_png_files_can_enter_the_document_workflow() {
    let directory = tempfile::tempdir().expect("temporary device directory");
    let (store, _) = open_conversation_store(directory.path().join("luna.db"));
    let conversation = store
        .create_conversation("rivera-household", "Supported documents")
        .expect("create Conversation");

    for (filename, contents) in [
        ("bill.pdf", b"%PDF-1.7 fixture".as_slice()),
        ("photo.jpg", b"\xFF\xD8\xFFfixture".as_slice()),
        ("scan.jpeg", b"\xFF\xD8\xFFfixture".as_slice()),
        ("letter.png", b"\x89PNG\r\n\x1a\nfixture".as_slice()),
    ] {
        let document = directory.path().join(filename);
        fs::write(&document, contents).expect("write supported fixture");
        store
            .attach_document("rivera-household", conversation.id, document)
            .expect("attach supported document type");
    }

    let unsupported = directory.path().join("notes.txt");
    fs::write(&unsupported, b"fixture").expect("write unsupported fixture");
    assert!(store
        .attach_document("rivera-household", conversation.id, unsupported)
        .is_err());
}

#[test]
fn a_document_arrival_rejects_a_file_whose_bytes_do_not_match_its_claimed_type() {
    let directory = tempfile::tempdir().expect("temporary device directory");
    let (store, _) = open_conversation_store(directory.path().join("luna.db"));
    let conversation = store
        .create_conversation("rivera-household", "Validate documents")
        .expect("create Conversation");
    let renamed_text_file = directory.path().join("statement.pdf");
    fs::write(&renamed_text_file, b"This is not a PDF.").expect("write malformed document");

    assert!(store
        .attach_document("rivera-household", conversation.id, renamed_text_file)
        .is_err());
    assert!(store
        .list_document_arrivals("rivera-household")
        .expect("list Document Arrivals")
        .is_empty());
}

#[test]
fn conversation_content_is_protected_and_requires_an_unlocked_trusted_device() {
    let directory = tempfile::tempdir().expect("temporary device directory");
    let database = directory.path().join("luna.db");
    let document = directory.path().join("private rates notice.pdf");
    fs::write(&document, b"%PDF-1.7 fixture").expect("write private document fixture");
    let (store, trusted_device) = open_conversation_store(&database);
    let conversation = store
        .create_conversation("rivera-household", "Private rates Conversation")
        .expect("create protected Conversation");
    store
        .add_member_message(
            "rivera-household",
            conversation.id,
            "My private account number is 12345.",
        )
        .expect("add protected message");
    store
        .attach_document("rivera-household", conversation.id, &document)
        .expect("attach protected document");

    let database_bytes =
        fs::read(&database).expect("read local database for sensitive output check");
    let database_text = String::from_utf8_lossy(&database_bytes);
    let document_path = document.to_string_lossy().into_owned();
    for plaintext in [
        "Private rates Conversation",
        "My private account number is 12345.",
        "private rates notice.pdf",
        document_path.as_str(),
        "needsMemberDirection",
    ] {
        assert!(
            !database_text.contains(plaintext),
            "local database must not expose protected Household content: {plaintext}"
        );
    }

    trusted_device.lock_device("rivera-household");
    assert!(store
        .list_conversations("rivera-household", None, false)
        .is_err());
    assert!(store.list_todo_items("rivera-household").is_err());
}
