use std::{
    collections::HashMap,
    fs,
    io::Cursor,
    path::Path,
    sync::{Arc, Mutex},
};

use luna_core::{
    ConfidenceState, ConversationStore, CredentialVault, DocumentProcessingState, LocalOcr,
    TrustedDeviceManager, VaultError,
};
use rusqlite::{params, Connection};
use serde_json::json;

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

struct FixedLocalOcr;

impl LocalOcr for FixedLocalOcr {
    fn extract_text(&self, _original: &Path, _media_type: &str) -> Option<String> {
        Some("Council rates notice".to_owned())
    }
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

fn image_fixture(format: image::ImageFormat) -> Vec<u8> {
    let image = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
        1,
        1,
        image::Rgb([255, 255, 255]),
    ));
    let mut bytes = Cursor::new(Vec::new());
    image
        .write_to(&mut bytes, format)
        .expect("encode image fixture");
    bytes.into_inner()
}

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

fn open_conversation_store_with_ocr(
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
    let store = ConversationStore::open_with_ocr(database, trusted_device.clone(), FixedLocalOcr)
        .expect("open Conversation store");
    (store, trusted_device)
}

#[cfg(unix)]
fn create_directory_link(target: &Path, link: &Path) {
    std::os::unix::fs::symlink(target, link).expect("create linked Incoming folder");
}

#[cfg(windows)]
fn create_directory_link(target: &Path, link: &Path) {
    if std::os::windows::fs::symlink_dir(target, link).is_ok() {
        return;
    }
    let status = std::process::Command::new("cmd")
        .arg("/C")
        .arg("mklink")
        .arg("/J")
        .arg(link)
        .arg(target)
        .status()
        .expect("create Incoming folder junction");
    assert!(status.success(), "create Incoming folder junction");
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
    fs::write(&document, digital_pdf_with_text("AGL bill")).expect("write document fixture");
    let database = directory.path().join("luna.db");
    let (store, trusted_device) = open_conversation_store(&database);
    let conversation = store
        .create_conversation("rivera-household", "Electricity bill")
        .expect("create Conversation");

    let arrival = store
        .attach_document(
            "rivera-household",
            conversation.id,
            &document,
            directory.path(),
        )
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
    fs::write(&document, image_fixture(image::ImageFormat::Png)).expect("write document fixture");
    let (store, _) = open_conversation_store(directory.path().join("luna.db"));
    let conversation = store
        .create_conversation("rivera-household", "Council rates")
        .expect("create Conversation");
    let arrival = store
        .attach_document(
            "rivera-household",
            conversation.id,
            &document,
            directory.path(),
        )
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
        ("bill.pdf", digital_pdf_with_text("Electricity bill")),
        ("photo.jpg", image_fixture(image::ImageFormat::Jpeg)),
        ("scan.jpeg", image_fixture(image::ImageFormat::Jpeg)),
        ("letter.png", image_fixture(image::ImageFormat::Png)),
    ] {
        let document = directory.path().join(filename);
        fs::write(&document, contents).expect("write supported fixture");
        store
            .attach_document(
                "rivera-household",
                conversation.id,
                document,
                directory.path(),
            )
            .expect("attach supported document type");
    }

    let unsupported = directory.path().join("notes.txt");
    fs::write(&unsupported, b"fixture").expect("write unsupported fixture");
    assert!(store
        .attach_document(
            "rivera-household",
            conversation.id,
            unsupported,
            directory.path()
        )
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
        .attach_document(
            "rivera-household",
            conversation.id,
            renamed_text_file,
            directory.path()
        )
        .is_err());
    assert!(store
        .list_document_arrivals("rivera-household")
        .expect("list Document Arrivals")
        .is_empty());
    assert!(
        !directory.path().join("document-arrivals").exists(),
        "malformed documents must not leave preserved Originals"
    );
}

#[test]
fn a_document_arrival_rejects_a_malformed_pdf() {
    let directory = tempfile::tempdir().expect("temporary device directory");
    let (store, _) = open_conversation_store(directory.path().join("luna.db"));
    let conversation = store
        .create_conversation("rivera-household", "Validate documents")
        .expect("create Conversation");
    let malformed_pdf = directory.path().join("statement.pdf");
    fs::write(&malformed_pdf, b"%PDF-1.7 not a complete PDF").expect("write malformed PDF");

    assert!(store
        .attach_document(
            "rivera-household",
            conversation.id,
            malformed_pdf,
            directory.path()
        )
        .is_err());
    assert!(store
        .list_document_arrivals("rivera-household")
        .expect("list Document Arrivals")
        .is_empty());
    assert!(
        !directory.path().join("document-arrivals").exists(),
        "malformed documents must not leave preserved Originals"
    );
}

#[test]
fn a_document_arrival_rejects_a_malformed_png() {
    let directory = tempfile::tempdir().expect("temporary device directory");
    let (store, _) = open_conversation_store(directory.path().join("luna.db"));
    let conversation = store
        .create_conversation("rivera-household", "Validate documents")
        .expect("create Conversation");
    let malformed_png = directory.path().join("statement.png");
    fs::write(&malformed_png, b"\x89PNG\r\n\x1a\nnot an image").expect("write malformed PNG");

    assert!(store
        .attach_document(
            "rivera-household",
            conversation.id,
            malformed_png,
            directory.path()
        )
        .is_err());
    assert!(store
        .list_document_arrivals("rivera-household")
        .expect("list Document Arrivals")
        .is_empty());
}

#[test]
fn a_document_arrival_preserves_the_exact_original_and_its_checksum() {
    let directory = tempfile::tempdir().expect("temporary device directory");
    let source_document = directory.path().join("AGL bill.pdf");
    let original_bytes = digital_pdf_with_text("AGL bill");
    fs::write(&source_document, &original_bytes).expect("write source document");
    let (store, _) = open_conversation_store(directory.path().join("luna.db"));
    let conversation = store
        .create_conversation("rivera-household", "Electricity bill")
        .expect("create Conversation");

    let arrival = store
        .attach_document(
            "rivera-household",
            conversation.id,
            &source_document,
            directory.path(),
        )
        .expect("attach supported document");

    assert_eq!(
        arrival.checksum,
        "6efc42d3b61e3ea0a01ad7b717f8745065894d8c94fa9fb9ef8db718d341e16f"
    );
    assert_eq!(
        arrival.original_path.parent(),
        Some(
            directory
                .path()
                .join("Incoming")
                .join("6efc42d3b61e3ea0a01ad7b717f8745065894d8c94fa9fb9ef8db718d341e16f")
                .as_path()
        )
    );
    assert_eq!(
        arrival
            .original_path
            .file_name()
            .and_then(|name| name.to_str()),
        Some("AGL bill.pdf")
    );
    assert_eq!(
        fs::read(&arrival.original_path).expect("read preserved Original"),
        original_bytes
    );
    fs::write(&source_document, b"%PDF-1.7 changed source").expect("change source document");
    assert_eq!(
        fs::read(&arrival.original_path).expect("read preserved Original after source change"),
        original_bytes
    );
}

#[test]
fn a_legacy_document_arrival_remains_readable_and_dismissible() {
    let directory = tempfile::tempdir().expect("temporary device directory");
    let database = directory.path().join("luna.db");
    let source_path = directory.path().join("legacy bill.pdf");
    let (store, trusted_device) = open_conversation_store(&database);
    let conversation = store
        .create_conversation("rivera-household", "Electricity bill")
        .expect("create Conversation");
    let legacy_payload = json!({
        "original_name": "legacy bill.pdf",
        "source_path": source_path,
        "media_type": "application/pdf",
        "extracted_text": null,
        "processing_state": "needsMemberDirection",
    });
    let protected = trusted_device
        .protect_household_state(
            "rivera-household",
            &serde_json::to_vec(&legacy_payload).expect("serialize legacy arrival"),
        )
        .expect("protect legacy arrival");
    let connection = Connection::open(&database).expect("open Conversation database");
    connection
        .execute(
            "INSERT INTO document_arrivals (household_id, conversation_id, protected_payload)
             VALUES (?1, ?2, ?3)",
            params![
                "rivera-household",
                conversation.id,
                serde_json::to_string(&protected).expect("serialize protected arrival")
            ],
        )
        .expect("store legacy arrival");

    let arrival = store
        .list_document_arrivals("rivera-household")
        .expect("read legacy Document Arrival")
        .pop()
        .expect("legacy arrival");
    assert_eq!(arrival.original_path, source_path);
    assert!(arrival.checksum.is_empty());
    assert_eq!(
        store
            .list_todo_items("rivera-household")
            .expect("list legacy To-do Item")
            .len(),
        1
    );

    store
        .dismiss_document_arrival("rivera-household", arrival.id)
        .expect("dismiss legacy Document Arrival");
    assert!(store
        .list_todo_items("rivera-household")
        .expect("list resolved legacy To-do Item")
        .is_empty());
}

#[test]
fn a_document_arrival_rejects_a_redirected_incoming_folder() {
    let directory = tempfile::tempdir().expect("temporary device directory");
    let document = directory.path().join("AGL bill.pdf");
    fs::write(&document, digital_pdf_with_text("AGL bill")).expect("write source document");
    let cabinet = directory.path().join("Cabinet");
    let outside = directory.path().join("outside");
    fs::create_dir(&cabinet).expect("create Cabinet");
    fs::create_dir(&outside).expect("create outside directory");
    create_directory_link(&outside, &cabinet.join("Incoming"));
    let (store, _) = open_conversation_store(directory.path().join("luna.db"));
    let conversation = store
        .create_conversation("rivera-household", "Electricity bill")
        .expect("create Conversation");

    assert!(store
        .attach_document("rivera-household", conversation.id, &document, &cabinet)
        .is_err());
    assert!(
        fs::read_dir(&outside)
            .expect("read outside directory")
            .next()
            .is_none(),
        "a redirected Incoming folder must not receive the Original"
    );
}

#[test]
fn a_document_arrival_extracts_text_from_a_digital_pdf_locally() {
    let directory = tempfile::tempdir().expect("temporary device directory");
    let document = directory.path().join("electricity bill.pdf");
    fs::write(&document, digital_pdf_with_text("AGL electricity bill")).expect("write digital PDF");
    let (store, _) = open_conversation_store(directory.path().join("luna.db"));
    let conversation = store
        .create_conversation("rivera-household", "Electricity bill")
        .expect("create Conversation");

    let arrival = store
        .attach_document(
            "rivera-household",
            conversation.id,
            document,
            directory.path(),
        )
        .expect("attach digital PDF");

    assert_eq!(
        arrival.extracted_text.as_deref(),
        Some("AGL electricity bill")
    );
    assert_eq!(
        arrival.review_card.confidence_state,
        ConfidenceState::NeedsChecking
    );
    assert_eq!(
        arrival.review_card.uncertainties,
        vec!["Luna needs your direction before filing this Original."]
    );
    assert_eq!(arrival.review_card.proposed_cabinet_destination, None);
    let evidence = arrival
        .review_card
        .evidence
        .iter()
        .map(|evidence| (evidence.label.as_str(), evidence.value.as_str()))
        .collect::<Vec<_>>();
    assert!(evidence.contains(&("Original name", "electricity bill.pdf")));
    assert!(evidence.contains(&("Detected type", "PDF")));
    assert!(evidence.contains(&("Extracted text", "AGL electricity bill")));
    assert!(evidence
        .iter()
        .any(|(label, value)| *label == "SHA-256" && value.len() == 64));
}

#[test]
fn a_document_arrival_uses_local_ocr_for_an_image() {
    let directory = tempfile::tempdir().expect("temporary device directory");
    let document = directory.path().join("council-rates.png");
    fs::write(&document, image_fixture(image::ImageFormat::Png)).expect("write PNG");
    let (store, _) = open_conversation_store_with_ocr(directory.path().join("luna.db"));
    let conversation = store
        .create_conversation("rivera-household", "Council rates")
        .expect("create Conversation");

    let arrival = store
        .attach_document(
            "rivera-household",
            conversation.id,
            document,
            directory.path(),
        )
        .expect("attach PNG");

    assert_eq!(
        arrival.extracted_text.as_deref(),
        Some("Council rates notice")
    );
}

#[test]
fn a_document_arrival_uses_local_ocr_for_an_image_only_pdf() {
    let directory = tempfile::tempdir().expect("temporary device directory");
    let document = directory.path().join("council-rates-scan.pdf");
    fs::write(&document, digital_pdf_with_text("")).expect("write image-only PDF");
    let (store, _) = open_conversation_store_with_ocr(directory.path().join("luna.db"));
    let conversation = store
        .create_conversation("rivera-household", "Council rates")
        .expect("create Conversation");

    let arrival = store
        .attach_document(
            "rivera-household",
            conversation.id,
            document,
            directory.path(),
        )
        .expect("attach image-only PDF");

    assert_eq!(
        arrival.extracted_text.as_deref(),
        Some("Council rates notice")
    );
}

#[test]
fn conversation_content_is_protected_and_requires_an_unlocked_trusted_device() {
    let directory = tempfile::tempdir().expect("temporary device directory");
    let database = directory.path().join("luna.db");
    let document = directory.path().join("private rates notice.pdf");
    fs::write(&document, digital_pdf_with_text("Private rates notice"))
        .expect("write private document fixture");
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
        .attach_document(
            "rivera-household",
            conversation.id,
            &document,
            directory.path(),
        )
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
