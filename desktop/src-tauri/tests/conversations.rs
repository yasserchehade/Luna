use std::{
    collections::HashMap,
    fs,
    io::Cursor,
    path::Path,
    sync::{Arc, Mutex},
};

use luna_core::{
    AuditAuthority, AuditEventKind, ConfidenceState, ContextField, ContextRelevanceDirection,
    ConversationStore, CredentialVault, DocumentContextDirection, DocumentProcessingState,
    FilingDecisionDirection, FilingRuleSummary, LocalOcr, TrustedDeviceManager, VaultError,
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

struct FixedLocalOcr(&'static str);

impl LocalOcr for FixedLocalOcr {
    fn extract_text(&self, _original: &Path, _media_type: &str) -> Option<String> {
        Some(self.0.to_owned())
    }
}

fn digital_pdf_with_text(text: &str) -> Vec<u8> {
    pdf_with_text_and_font_resource(text, "/F1 5 0 R")
}

fn pdf_with_text_and_font_resource(text: &str, font_resource: &str) -> Vec<u8> {
    let content = format!("BT\n/F1 12 Tf\n72 720 Td\n({text}) Tj\nET\n");
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        format!("<< /Type /Page /Parent 2 0 R /Resources << /Font << {font_resource} >> >> /MediaBox [0 0 612 792] /Contents 4 0 R >>"),
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
    extracted_text: &'static str,
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
    let store = ConversationStore::open_with_ocr(
        database,
        trusted_device.clone(),
        FixedLocalOcr(extracted_text),
    )
    .expect("open Conversation store");
    (store, trusted_device)
}

fn prepare_document_for_filing(
    store: &TestConversationStore,
    household_id: &str,
    cabinet: &Path,
    source: &Path,
    final_name: &str,
) -> luna_core::DocumentArrival {
    prepare_document_for_destination(
        store,
        household_id,
        cabinet,
        source,
        final_name,
        &format!("Household records/{final_name}"),
    )
}

fn prepare_document_for_destination(
    store: &TestConversationStore,
    household_id: &str,
    cabinet: &Path,
    source: &Path,
    final_name: &str,
    cabinet_destination: &str,
) -> luna_core::DocumentArrival {
    let conversation = store
        .create_conversation(household_id, "Document filing")
        .expect("create Conversation");
    let arrival = store
        .attach_document(household_id, conversation.id, source, cabinet)
        .expect("stage Original");
    store
        .record_member_direction(
            household_id,
            arrival.id,
            DocumentContextDirection {
                document_type: Some("Household document".to_owned()),
                document_type_resolved: true,
                service_provider: Some("Household Service".to_owned()),
                service_provider_resolved: true,
                addressee: Some("Sam Rivera".to_owned()),
                addressee_resolved: true,
                property: None,
                property_resolved: true,
                account: None,
                account_resolved: true,
                amount: None,
                amount_resolved: true,
                relevant_dates: Vec::new(),
                relevant_dates_resolved: true,
                service_provider_relevance: Some(ContextRelevanceDirection {
                    subject: "Household Service".to_owned(),
                    explanation: "Issues this Household document".to_owned(),
                }),
                property_relevance: None,
            },
            "Household records",
        )
        .expect("record Member Direction");
    store
        .confirm_filing_decision(
            household_id,
            arrival.id,
            FilingDecisionDirection {
                file_name: final_name.to_owned(),
                cabinet_destination: cabinet_destination.to_owned(),
            },
        )
        .expect("confirm Filing Decision")
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
fn a_document_arrival_survives_a_pdf_parser_font_panic() {
    let directory = tempfile::tempdir().expect("temporary utility bill directory");
    let source = directory.path().join("utility bill.pdf");
    fs::write(&source, pdf_with_text_and_font_resource("Utility bill", ""))
        .expect("write utility bill parser fixture");
    let (store, _) = open_conversation_store(directory.path().join("luna.db"));
    let conversation = store
        .create_conversation("rivera-household", "Utility bill")
        .expect("create Conversation");

    let arrival = store
        .attach_document(
            "rivera-household",
            conversation.id,
            &source,
            directory.path(),
        )
        .expect("attach utility bill PDF");

    assert_eq!(arrival.original_name, "utility bill.pdf");
    assert_eq!(arrival.media_type, "application/pdf");
}

#[test]
fn an_unfamiliar_document_review_represents_context_and_asks_only_filing_questions() {
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

    assert_eq!(arrival.review_card.context.document_type.value, None);
    assert_eq!(arrival.review_card.context.service_provider.value, None);
    assert_eq!(arrival.review_card.context.addressee.value, None);
    assert_eq!(arrival.review_card.context.property.value, None);
    assert_eq!(arrival.review_card.context.account.value, None);
    assert_eq!(arrival.review_card.context.amount.value, None);
    assert!(arrival.review_card.context.relevant_dates.is_empty());
    assert_eq!(arrival.review_card.filing_decision, None);
    assert_eq!(
        arrival
            .review_card
            .questions
            .iter()
            .map(|question| question.field)
            .collect::<Vec<_>>(),
        vec![
            ContextField::DocumentType,
            ContextField::ServiceProvider,
            ContextField::Addressee,
            ContextField::Property,
            ContextField::Account,
            ContextField::Amount,
            ContextField::RelevantDates,
        ]
    );
}

#[test]
fn labelled_local_extraction_populates_editable_fields_for_member_correction() {
    let directory = tempfile::tempdir().expect("temporary device directory");
    let document = directory.path().join("electricity bill.pdf");
    fs::write(
        &document,
        digital_pdf_with_text(
            "Document Type: Electricity statement; Service Provider: AGL; Addressee: S. Rivera; Property: 12 Seabreeze Ave; Account: 12345678; Amount: $184.72; Relevant Date: 2026-07-15",
        ),
    )
    .expect("write digital PDF");
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
        arrival.review_card.context.document_type.value.as_deref(),
        Some("Electricity statement")
    );
    assert_eq!(
        arrival
            .review_card
            .context
            .service_provider
            .value
            .as_deref(),
        Some("AGL")
    );
    assert_eq!(
        arrival.review_card.context.addressee.value.as_deref(),
        Some("S. Rivera")
    );
    assert_eq!(
        arrival.review_card.context.document_type.confidence_state,
        ConfidenceState::LooksRight
    );
    assert!(arrival
        .review_card
        .questions
        .iter()
        .any(|question| question.field == ContextField::Addressee));
}

#[test]
fn a_new_service_provider_and_property_stay_unresolved_until_their_relevance_is_explained() {
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

    let reviewed = store
        .record_member_direction(
            "rivera-household",
            arrival.id,
            DocumentContextDirection {
                document_type: Some("Electricity bill".to_owned()),
                document_type_resolved: true,
                service_provider: Some("AGL".to_owned()),
                service_provider_resolved: true,
                addressee: Some("Sam Rivera".to_owned()),
                addressee_resolved: true,
                property: Some("12 Seabreeze Avenue".to_owned()),
                property_resolved: true,
                account: Some("12345678".to_owned()),
                account_resolved: true,
                amount: Some("$184.72".to_owned()),
                amount_resolved: true,
                relevant_dates: vec!["2026-07-15".to_owned()],
                relevant_dates_resolved: true,
                service_provider_relevance: None,
                property_relevance: None,
            },
            "Household",
        )
        .expect("record Member Direction");

    assert_eq!(
        reviewed
            .review_card
            .questions
            .iter()
            .map(|question| question.field)
            .collect::<Vec<_>>(),
        vec![
            ContextField::ServiceProviderRelevance,
            ContextField::PropertyRelevance,
        ]
    );
    assert_eq!(reviewed.review_card.filing_decision, None);
    assert_eq!(
        reviewed.processing_state,
        DocumentProcessingState::NeedsMemberDirection
    );
}

#[test]
fn changed_context_cannot_reuse_relevance_for_a_previous_subject() {
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

    let reviewed = store
        .record_member_direction(
            "rivera-household",
            arrival.id,
            DocumentContextDirection {
                document_type: Some("Electricity bill".to_owned()),
                document_type_resolved: true,
                service_provider: Some("Origin Energy".to_owned()),
                service_provider_resolved: true,
                addressee: Some("Sam Rivera".to_owned()),
                addressee_resolved: true,
                property: None,
                property_resolved: true,
                account: None,
                account_resolved: true,
                amount: None,
                amount_resolved: true,
                relevant_dates: Vec::new(),
                relevant_dates_resolved: true,
                service_provider_relevance: Some(ContextRelevanceDirection {
                    subject: "AGL".to_owned(),
                    explanation: "Supplies electricity to our home".to_owned(),
                }),
                property_relevance: None,
            },
            "Household records",
        )
        .expect("record changed Service Provider");

    assert_eq!(
        reviewed
            .review_card
            .questions
            .iter()
            .map(|question| question.field)
            .collect::<Vec<_>>(),
        vec![ContextField::ServiceProviderRelevance]
    );
    assert_eq!(reviewed.review_card.filing_decision, None);
}

#[test]
fn confirmed_context_corrections_produce_a_readable_destination_proposal() {
    let directory = tempfile::tempdir().expect("temporary device directory");
    let document = directory.path().join("scan.pdf");
    fs::write(&document, digital_pdf_with_text("Origin account notice"))
        .expect("write digital PDF");
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

    let reviewed = store
        .record_member_direction(
            "rivera-household",
            arrival.id,
            DocumentContextDirection {
                document_type: Some("Electricity bill".to_owned()),
                document_type_resolved: true,
                service_provider: Some("Origin Energy".to_owned()),
                service_provider_resolved: true,
                addressee: Some("Sam Rivera".to_owned()),
                addressee_resolved: true,
                property: Some("12 Seabreeze Avenue".to_owned()),
                property_resolved: true,
                account: Some("12345678".to_owned()),
                account_resolved: true,
                amount: Some("$184.72".to_owned()),
                amount_resolved: true,
                relevant_dates: vec!["2026-07-15".to_owned(), "2026-08-02".to_owned()],
                relevant_dates_resolved: true,
                service_provider_relevance: Some(ContextRelevanceDirection {
                    subject: "Origin Energy".to_owned(),
                    explanation: "Supplies electricity to our home".to_owned(),
                }),
                property_relevance: Some(ContextRelevanceDirection {
                    subject: "12 Seabreeze Avenue".to_owned(),
                    explanation: "Our primary residence".to_owned(),
                }),
            },
            "Household records",
        )
        .expect("record corrected Member Direction");

    assert_eq!(
        reviewed
            .review_card
            .context
            .service_provider
            .value
            .as_deref(),
        Some("Origin Energy")
    );
    assert_eq!(
        reviewed
            .review_card
            .context
            .service_provider
            .confidence_state,
        ConfidenceState::Confirmed
    );
    assert!(reviewed.review_card.questions.is_empty());
    assert_eq!(
        reviewed.review_card.filing_decision,
        Some(luna_core::FilingDecisionReview {
            file_name:
                "2026-07-15 - Origin Energy - Electricity bill - Sam Rivera.pdf".to_owned(),
            cabinet_destination: "Household records/12 Seabreeze Avenue/Origin Energy/2026/2026-07-15 - Origin Energy - Electricity bill - Sam Rivera.pdf".to_owned(),
            confirmed: false,
        })
    );
    assert_eq!(
        reviewed.processing_state,
        DocumentProcessingState::NeedsMemberDirection
    );
}

#[test]
fn only_a_resolved_editable_filing_decision_can_be_confirmed() {
    let directory = tempfile::tempdir().expect("temporary device directory");
    let document = directory.path().join("scan.pdf");
    fs::write(&document, digital_pdf_with_text("Origin account notice"))
        .expect("write digital PDF");
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

    assert!(store
        .confirm_filing_decision(
            "rivera-household",
            arrival.id,
            FilingDecisionDirection {
                file_name: "Origin bill July 2026.pdf".to_owned(),
                cabinet_destination: "Household records/Electricity/Origin bill July 2026.pdf"
                    .to_owned(),
            },
        )
        .is_err());

    store
        .record_member_direction(
            "rivera-household",
            arrival.id,
            DocumentContextDirection {
                document_type: Some("Electricity bill".to_owned()),
                document_type_resolved: true,
                service_provider: Some("Origin Energy".to_owned()),
                service_provider_resolved: true,
                addressee: Some("Sam Rivera".to_owned()),
                addressee_resolved: true,
                property: None,
                property_resolved: true,
                account: Some("12345678".to_owned()),
                account_resolved: true,
                amount: Some("$184.72".to_owned()),
                amount_resolved: true,
                relevant_dates: vec!["2026-07-15".to_owned()],
                relevant_dates_resolved: true,
                service_provider_relevance: Some(ContextRelevanceDirection {
                    subject: "Origin Energy".to_owned(),
                    explanation: "Our electricity retailer".to_owned(),
                }),
                property_relevance: None,
            },
            "Household records",
        )
        .expect("record Member Direction");
    assert!(store
        .confirm_filing_decision(
            "rivera-household",
            arrival.id,
            FilingDecisionDirection {
                file_name: "Origin bill July 2026.pdf".to_owned(),
                cabinet_destination: "Incoming/Origin bill July 2026.pdf".to_owned(),
            },
        )
        .is_err());
    assert!(store
        .confirm_filing_decision(
            "rivera-household",
            arrival.id,
            FilingDecisionDirection {
                file_name: "CON.pdf".to_owned(),
                cabinet_destination: "Household records/CON.pdf".to_owned(),
            },
        )
        .is_err());
    assert!(store
        .confirm_filing_decision(
            "rivera-household",
            arrival.id,
            FilingDecisionDirection {
                file_name: "CONIN$.pdf".to_owned(),
                cabinet_destination: "Household records/CONIN$.pdf".to_owned(),
            },
        )
        .is_err());
    let confirmed = store
        .confirm_filing_decision(
            "rivera-household",
            arrival.id,
            FilingDecisionDirection {
                file_name: "Origin bill July 2026.pdf".to_owned(),
                cabinet_destination: "Household records/Electricity/Origin bill July 2026.pdf"
                    .to_owned(),
            },
        )
        .expect("confirm edited Filing Decision");

    assert_eq!(
        confirmed.processing_state,
        DocumentProcessingState::ReadyToFile
    );
    assert_eq!(
        confirmed.review_card.filing_decision,
        Some(luna_core::FilingDecisionReview {
            file_name: "Origin bill July 2026.pdf".to_owned(),
            cabinet_destination: "Household records/Electricity/Origin bill July 2026.pdf"
                .to_owned(),
            confirmed: true,
        })
    );
    assert!(store
        .list_todo_items("rivera-household")
        .expect("list To-do Items")
        .is_empty());
}

#[test]
fn filing_verifies_the_untouched_original_before_recording_one_consistent_outcome() {
    let directory = tempfile::tempdir().expect("temporary device directory");
    let cabinet = directory.path().join("Cabinet");
    fs::create_dir_all(cabinet.join("Household records")).expect("create Cabinet");
    let original = digital_pdf_with_text("AGL electricity bill");
    let source = directory.path().join("source electricity bill.pdf");
    fs::write(&source, &original).expect("write source fixture");
    let (store, _) = open_conversation_store(directory.path().join("luna.db"));
    let ready = prepare_document_for_filing(
        &store,
        "rivera-household",
        &cabinet,
        &source,
        "Electricity bill July 2026.pdf",
    );
    let staged_path = ready.original_path.clone();

    let filed = store
        .file_document("rivera-household", ready.id, &cabinet)
        .expect("file and verify Original");

    assert_eq!(filed.processing_state, DocumentProcessingState::Filed);
    assert!(!staged_path.exists(), "staging is removed after completion");
    let filed_original = filed.filed_original.expect("completed Filed Original");
    assert_eq!(
        fs::read(&filed_original.final_path).expect("read Filed Original"),
        original
    );
    assert_eq!(filed_original.original_name, "source electricity bill.pdf");
    assert_eq!(filed_original.checksum, ready.checksum);
    assert_eq!(filed_original.source_path, source);
    assert_eq!(
        filed_original.filing_decision.cabinet_destination,
        "Household records/Electricity bill July 2026.pdf"
    );
    assert!(store
        .list_todo_items("rivera-household")
        .expect("list To-do Items")
        .is_empty());
    assert_eq!(
        store
            .list_filed_originals("rivera-household")
            .expect("list Cabinet Originals"),
        vec![filed_original.clone()]
    );
    let history = store
        .list_audit_events("rivera-household")
        .expect("list History");
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].kind, AuditEventKind::DocumentFiled);
    assert_eq!(history[0].authority, AuditAuthority::MemberDirection);
    assert_eq!(history[0].subject, "source electricity bill.pdf");
    assert_eq!(history[0].filed_original, filed_original);
}

#[test]
fn filing_never_overwrites_an_existing_destination_and_keeps_staging_for_recovery() {
    let directory = tempfile::tempdir().expect("temporary device directory");
    let cabinet = directory.path().join("Cabinet");
    let section = cabinet.join("Household records");
    fs::create_dir_all(&section).expect("create Cabinet");
    let source = directory.path().join("rates.png");
    let original = image_fixture(image::ImageFormat::Png);
    fs::write(&source, &original).expect("write source fixture");
    let destination = section.join("Council rates.png");
    fs::write(&destination, b"an existing household document").expect("write existing file");
    let (store, _) = open_conversation_store(directory.path().join("luna.db"));
    let ready = prepare_document_for_filing(
        &store,
        "rivera-household",
        &cabinet,
        &source,
        "Council rates.png",
    );

    let error = store
        .file_document("rivera-household", ready.id, &cabinet)
        .expect_err("refuse destination conflict");

    assert_eq!(
        error.to_string(),
        "A different Original already occupies the Cabinet Destination."
    );
    assert_eq!(
        fs::read(&destination).expect("read existing destination"),
        b"an existing household document"
    );
    assert!(
        ready.original_path.exists(),
        "staged Original remains recoverable"
    );
    assert_eq!(
        store
            .list_document_arrivals("rivera-household")
            .expect("list Conversation work")[0]
            .processing_state,
        DocumentProcessingState::ReadyToFile
    );
    assert!(store
        .list_audit_events("rivera-household")
        .expect("list History")
        .is_empty());

    fs::write(&destination, &original).expect("replace fixture with an exact duplicate");
    assert!(store
        .file_document("rivera-household", ready.id, &cabinet)
        .is_err());
    assert!(ready.original_path.exists());
    assert!(store
        .list_audit_events("rivera-household")
        .expect("list History after exact destination conflict")
        .is_empty());
}

#[test]
fn filing_rejects_a_destination_that_escapes_the_configured_cabinet_through_a_link() {
    let directory = tempfile::tempdir().expect("temporary device directory");
    let cabinet = directory.path().join("Cabinet");
    let section = cabinet.join("Household records");
    let outside = directory.path().join("Outside");
    fs::create_dir_all(&section).expect("create Cabinet");
    fs::create_dir(&outside).expect("create outside folder");
    create_directory_link(&outside, &section.join("Escape"));
    let source = directory.path().join("notice.pdf");
    fs::write(&source, digital_pdf_with_text("Household notice")).expect("write source fixture");
    let (store, _) = open_conversation_store(directory.path().join("luna.db"));
    let ready = prepare_document_for_destination(
        &store,
        "rivera-household",
        &cabinet,
        &source,
        "Notice.pdf",
        "Household records/Escape/Notice.pdf",
    );
    let error = store
        .file_document("rivera-household", ready.id, &cabinet)
        .expect_err("reject linked destination outside Cabinet");

    assert_eq!(
        error.to_string(),
        "The Cabinet Destination must be a safe relative path ending in the chosen filename."
    );
    assert!(!outside.join("Notice.pdf").exists());
    assert!(ready.original_path.exists());
}

#[test]
fn filing_recovers_after_the_verified_destination_was_written_before_event_recording() {
    let directory = tempfile::tempdir().expect("temporary device directory");
    let cabinet = directory.path().join("Cabinet");
    let section = cabinet.join("Household records");
    fs::create_dir_all(&section).expect("create Cabinet");
    let original = digital_pdf_with_text("Interrupted filing");
    let source = directory.path().join("interrupted.pdf");
    fs::write(&source, &original).expect("write source fixture");
    let database = directory.path().join("luna.db");
    let (store, trusted_device) = open_conversation_store(&database);
    let ready = prepare_document_for_filing(
        &store,
        "rivera-household",
        &cabinet,
        &source,
        "Recovered filing.pdf",
    );
    let destination = section.join("Recovered filing.pdf");
    let interrupted_temporary =
        section.join(format!(".luna-filing-{}-{}.tmp", ready.id, ready.checksum));
    fs::create_dir(&interrupted_temporary).expect("block temporary filing write");
    store
        .file_document("rivera-household", ready.id, &cabinet)
        .expect_err("simulate interruption after durable Filing state");
    assert_eq!(
        store
            .list_document_arrivals("rivera-household")
            .expect("list interrupted filing")[0]
            .processing_state,
        DocumentProcessingState::Filing
    );
    fs::remove_dir(interrupted_temporary).expect("clear interrupted temporary write");
    fs::write(&destination, &original).expect("simulate verified write before durable event");
    drop(store);
    let reopened = ConversationStore::open(&database, trusted_device)
        .expect("reopen Conversation store after interruption");

    reopened
        .resume_document_filings("rivera-household", &cabinet)
        .expect("resume interrupted filing");
    let filed = reopened
        .list_document_arrivals("rivera-household")
        .expect("list resumed filing")
        .remove(0);

    assert_eq!(filed.processing_state, DocumentProcessingState::Filed);
    assert!(!ready.original_path.exists());
    assert_eq!(
        reopened
            .list_audit_events("rivera-household")
            .expect("list History")
            .len(),
        1
    );

    fs::create_dir_all(ready.original_path.parent().expect("staging directory"))
        .expect("recreate staging after simulated cleanup interruption");
    fs::write(&ready.original_path, &original).expect("restore staged Original");
    reopened
        .resume_document_filings("rivera-household", &cabinet)
        .expect("resume cleanup after durable event");
    assert!(!ready.original_path.exists());
    assert_eq!(
        reopened
            .list_audit_events("rivera-household")
            .expect("list History after idempotent recovery")
            .len(),
        1
    );
}

#[test]
fn pdf_jpg_and_png_originals_complete_the_verified_filing_journey() {
    let fixtures = [
        (
            "statement.pdf",
            digital_pdf_with_text("PDF statement"),
            "Filed statement.pdf",
        ),
        (
            "photo.jpg",
            image_fixture(image::ImageFormat::Jpeg),
            "Filed photo.jpg",
        ),
        (
            "scan.png",
            image_fixture(image::ImageFormat::Png),
            "Filed scan.png",
        ),
    ];

    for (index, (source_name, original, final_name)) in fixtures.into_iter().enumerate() {
        let directory = tempfile::tempdir().expect("temporary device directory");
        let cabinet = directory.path().join("Cabinet");
        fs::create_dir_all(cabinet.join("Household records")).expect("create Cabinet");
        let source = directory.path().join(source_name);
        fs::write(&source, &original).expect("write fixture");
        let (store, _) = open_conversation_store(directory.path().join(format!("luna-{index}.db")));
        let ready =
            prepare_document_for_filing(&store, "rivera-household", &cabinet, &source, final_name);

        let filed = store
            .file_document("rivera-household", ready.id, &cabinet)
            .expect("file supported Original");

        assert_eq!(filed.processing_state, DocumentProcessingState::Filed);
        assert_eq!(
            fs::read(
                filed
                    .filed_original
                    .expect("completed Filed Original")
                    .final_path
            )
            .expect("read filed fixture"),
            original
        );
    }
}

#[test]
fn a_document_arrival_uses_local_ocr_for_an_image() {
    let directory = tempfile::tempdir().expect("temporary device directory");
    let document = directory.path().join("council-rates.png");
    fs::write(&document, image_fixture(image::ImageFormat::Png)).expect("write PNG");
    let (store, _) =
        open_conversation_store_with_ocr(directory.path().join("luna.db"), "Council rates notice");
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
    let (store, _) =
        open_conversation_store_with_ocr(directory.path().join("luna.db"), "Council rates notice");
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

#[test]
fn a_confirmed_filing_teaches_a_rule_and_exact_context_matches_file_automatically() {
    let directory = tempfile::tempdir().expect("temporary rule directory");
    let cabinet = directory.path().join("Cabinet");
    fs::create_dir_all(cabinet.join("Household records")).expect("create Cabinet");
    let first_source = directory.path().join("agl-july.pdf");
    fs::write(
        &first_source,
        digital_pdf_with_text(
            "Document Type: Electricity bill; Service Provider: AGL Energy; Addressee: Sam Rivera; Property: 12 Seabreeze Avenue; Account: 12345678; Relevant Date: 2026-07-15",
        ),
    )
    .expect("write first bill");
    let (store, _) = open_conversation_store(directory.path().join("luna.db"));
    let conversation = store
        .create_conversation("rivera-household", "Electricity bills")
        .expect("create Conversation");
    let first = store
        .attach_document("rivera-household", conversation.id, &first_source, &cabinet)
        .expect("attach first bill");
    let direction = DocumentContextDirection {
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
            explanation: "Supplies electricity to our home".to_owned(),
        }),
        property_relevance: Some(ContextRelevanceDirection {
            subject: "12 Seabreeze Avenue".to_owned(),
            explanation: "Our primary residence".to_owned(),
        }),
    };
    store
        .record_member_direction("rivera-household", first.id, direction, "Household records")
        .expect("record first Member Direction");
    store
        .confirm_filing_decision(
            "rivera-household",
            first.id,
            FilingDecisionDirection {
                file_name: "AGL bill July 2026.pdf".to_owned(),
                cabinet_destination:
                    "Household records/12 Seabreeze Avenue/AGL/2026/AGL bill July 2026.pdf"
                        .to_owned(),
            },
        )
        .expect("confirm first Filing Decision");
    let filed = store
        .file_document("rivera-household", first.id, &cabinet)
        .expect("file first bill");
    assert!(filed.review_card.learned_rule.is_some());

    let second_source = directory.path().join("agl-august.pdf");
    fs::write(
        &second_source,
        digital_pdf_with_text("AGL Energy electricity bill for Sam Rivera at 12 Seabreeze Avenue, account 12345678, issued 2026-08-15"),
    )
    .expect("write second bill");
    let automatically_filed = store
        .attach_document(
            "rivera-household",
            conversation.id,
            &second_source,
            &cabinet,
        )
        .expect("attach exact contextual match");
    assert_eq!(
        automatically_filed.processing_state,
        DocumentProcessingState::Filed
    );
    assert!(automatically_filed.filed_original.is_some());
    assert!(automatically_filed.review_card.learned_rule.is_some());
    let history = store
        .list_audit_events("rivera-household")
        .expect("list filing history");
    assert_eq!(
        history[0].kind,
        AuditEventKind::ExactMatchHandledAutomatically
    );
    assert_eq!(history[0].authority, AuditAuthority::FilingRule);

    let unstructured_changed = directory.path().join("origin-unstructured.pdf");
    fs::write(
        &unstructured_changed,
        digital_pdf_with_text(
            "Origin Energy electricity bill for Sam Rivera at 12 Seabreeze Avenue, account 12345678, issued 2026-09-15; previously AGL Energy",
        ),
    )
    .expect("write unstructured changed-provider bill");
    let unstructured_changed_arrival = store
        .attach_document(
            "rivera-household",
            conversation.id,
            &unstructured_changed,
            &cabinet,
        )
        .expect("attach unstructured changed-provider bill");
    assert_eq!(
        unstructured_changed_arrival.processing_state,
        DocumentProcessingState::NeedsMemberDirection
    );

    let unstructured_account_changed = directory.path().join("account-unstructured.pdf");
    fs::write(
        &unstructured_account_changed,
        digital_pdf_with_text(
            "AGL Energy electricity bill for Sam Rivera at 12 Seabreeze Avenue, account 98765432, issued 2026-09-15; previously account 12345678",
        ),
    )
    .expect("write unstructured changed-account bill");
    let unstructured_account_changed_arrival = store
        .attach_document(
            "rivera-household",
            conversation.id,
            &unstructured_account_changed,
            &cabinet,
        )
        .expect("attach unstructured changed-account bill");
    assert_eq!(
        unstructured_account_changed_arrival.processing_state,
        DocumentProcessingState::NeedsMemberDirection
    );

    let changed_contexts = [
        (
            "provider",
            "Electricity bill",
            "Origin Energy",
            "Sam Rivera",
            "12 Seabreeze Avenue",
            "12345678",
        ),
        (
            "addressee",
            "Electricity bill",
            "AGL Energy",
            "Jordan Rivera",
            "12 Seabreeze Avenue",
            "12345678",
        ),
        (
            "property",
            "Electricity bill",
            "AGL Energy",
            "Sam Rivera",
            "14 Seabreeze Avenue",
            "12345678",
        ),
        (
            "account",
            "Electricity bill",
            "AGL Energy",
            "Sam Rivera",
            "12 Seabreeze Avenue",
            "98765432",
        ),
        (
            "document type",
            "Gas bill",
            "AGL Energy",
            "Sam Rivera",
            "12 Seabreeze Avenue",
            "12345678",
        ),
    ];
    for (index, (label, document_type, provider, addressee, property, account)) in
        changed_contexts.into_iter().enumerate()
    {
        let source = directory.path().join(format!("changed-{index}.pdf"));
        fs::write(
            &source,
            digital_pdf_with_text(&format!(
                "Document Type: {document_type}; Service Provider: {provider}; Addressee: {addressee}; Property: {property}; Account: {account}; Relevant Date: 2026-09-{:02}",
                index + 1,
            )),
        )
        .expect("write changed-context bill");
        let changed = store
            .attach_document("rivera-household", conversation.id, &source, &cabinet)
            .unwrap_or_else(|_| panic!("attach changed-{label} bill"));
        assert_eq!(
            changed.processing_state,
            DocumentProcessingState::NeedsMemberDirection,
            "changed {label} must not inherit the learned rule",
        );
        assert!(
            changed.review_card.filing_decision.is_none(),
            "changed {label} must not receive an automatic destination",
        );
    }
}

#[test]
fn a_font_unsupported_pdf_still_provides_local_text_for_rule_matching() {
    let directory = tempfile::tempdir().expect("temporary fallback directory");
    let cabinet = directory.path().join("Cabinet");
    fs::create_dir_all(&cabinet).expect("create Cabinet");
    let source = directory.path().join("utility bill.pdf");
    fs::write(
        &source,
        pdf_with_text_and_font_resource("Account Number: 12345678", ""),
    )
    .expect("write utility bill fallback fixture");
    let (store, _) = open_conversation_store_with_ocr(
        directory.path().join("luna.db"),
        "Account Number: 12345678",
    );
    let conversation = store
        .create_conversation("rivera-household", "Fallback")
        .expect("create Conversation");
    let arrival = store
        .attach_document("rivera-household", conversation.id, &source, &cabinet)
        .expect("attach utility bill");
    let extracted = arrival.extracted_text.expect("fallback text");
    let compact = extracted
        .chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();
    assert!(compact.contains("accountnumber"));
}

#[test]
fn an_owner_can_inspect_the_learned_rule_scope_and_affected_documents() {
    let directory = tempfile::tempdir().expect("temporary rulebook directory");
    let cabinet = directory.path().join("Cabinet");
    fs::create_dir_all(cabinet.join("Household records")).expect("create Cabinet");
    let source = directory.path().join("agl-july.pdf");
    fs::write(
        &source,
        digital_pdf_with_text(
            "Document Type: Electricity bill; Service Provider: AGL Energy; Addressee: Sam Rivera; Property: 12 Seabreeze Avenue; Account: 12345678; Relevant Date: 2026-07-15",
        ),
    )
    .expect("write bill");
    let (store, _) = open_conversation_store(directory.path().join("luna.db"));
    let conversation = store
        .create_conversation("rivera-household", "Electricity bills")
        .expect("create Conversation");
    let arrival = store
        .attach_document("rivera-household", conversation.id, &source, &cabinet)
        .expect("attach bill");
    store
        .record_member_direction(
            "rivera-household",
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
                    explanation: "Supplies electricity to our home".to_owned(),
                }),
                property_relevance: Some(ContextRelevanceDirection {
                    subject: "12 Seabreeze Avenue".to_owned(),
                    explanation: "Our primary residence".to_owned(),
                }),
                ..Default::default()
            },
            "Household records",
        )
        .expect("record Member Direction");
    store
        .confirm_filing_decision(
            "rivera-household",
            arrival.id,
            FilingDecisionDirection {
                file_name: "AGL bill July 2026.pdf".to_owned(),
                cabinet_destination:
                    "Household records/12 Seabreeze Avenue/AGL/2026/AGL bill July 2026.pdf"
                        .to_owned(),
            },
        )
        .expect("confirm Filing Decision");
    let filed = store
        .file_document("rivera-household", arrival.id, &cabinet)
        .expect("file bill");

    let rules: Vec<FilingRuleSummary> = store
        .list_filing_rules("rivera-household")
        .expect("list Filing Rules");
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].teacher, "Member Direction");
    assert!(!rules[0].created_at.is_empty());
    assert_eq!(rules[0].affected_documents, vec![filed.original_name]);
    assert_eq!(
        rules[0].cabinet_destination,
        filed
            .review_card
            .filing_decision
            .expect("filed decision")
            .cabinet_destination,
    );

    let updated = store
        .update_filing_rule(
            "rivera-household",
            rules[0].id,
            luna_core::FilingRuleUpdate {
                document_type: "Electricity bill".to_owned(),
                service_provider: "AGL Energy".to_owned(),
                addressee: "Sam Rivera".to_owned(),
                property: Some("12 Seabreeze Avenue".to_owned()),
                account: Some("12345678".to_owned()),
                file_name: "AGL bill July 2026.pdf".to_owned(),
                cabinet_destination:
                    "Household records/12 Seabreeze Avenue/AGL/2026-updated/AGL bill July 2026.pdf"
                        .to_owned(),
            },
        )
        .expect("update Filing Rule");
    assert_eq!(
        updated.cabinet_destination,
        "Household records/12 Seabreeze Avenue/AGL/2026-updated/AGL bill July 2026.pdf"
    );
    assert_eq!(updated.affected_documents, vec!["agl-july.pdf"]);
    let preview = store
        .preview_filing_rule_reorganization(
            "rivera-household",
            rules[0].id,
            "Household records/12 Seabreeze Avenue/AGL/2027",
        )
        .expect("preview historical reorganisation");
    assert_eq!(preview.documents.len(), 1);
    assert_eq!(
        preview.documents[0].current_destination,
        "Household records/12 Seabreeze Avenue/AGL/2026/AGL bill July 2026.pdf"
    );
    assert_eq!(
        preview.documents[0].proposed_destination,
        "Household records/12 Seabreeze Avenue/AGL/2027/AGL bill July 2026.pdf"
    );

    store
        .pause_filing_rule("rivera-household", rules[0].id, true)
        .expect("pause Filing Rule");
    let paused_source = directory.path().join("agl-august-paused.pdf");
    fs::write(
        &paused_source,
        digital_pdf_with_text(
            "Document Type: Electricity bill; Service Provider: AGL Energy; Addressee: Sam Rivera; Property: 12 Seabreeze Avenue; Account: 12345678; Relevant Date: 2026-08-15",
        ),
    )
    .expect("write paused match");
    assert_eq!(
        store
            .attach_document(
                "rivera-household",
                conversation.id,
                &paused_source,
                &cabinet,
            )
            .expect("attach paused match")
            .processing_state,
        DocumentProcessingState::NeedsMemberDirection
    );

    store
        .pause_filing_rule("rivera-household", rules[0].id, false)
        .expect("resume Filing Rule");
    let resumed_source = directory.path().join("agl-august-resumed.pdf");
    fs::write(
        &resumed_source,
        digital_pdf_with_text(
            "Document Type: Electricity bill; Service Provider: AGL Energy; Addressee: Sam Rivera; Property: 12 Seabreeze Avenue; Account: 12345678; Relevant Date: 2026-08-15",
        ),
    )
    .expect("write resumed match");
    let resumed = store
        .attach_document(
            "rivera-household",
            conversation.id,
            &resumed_source,
            &cabinet,
        )
        .expect("attach resumed match");
    assert_eq!(resumed.processing_state, DocumentProcessingState::Filed);
    let moved_destination = cabinet.join("Household records/manual/AGL bill August 2026.pdf");
    fs::create_dir_all(moved_destination.parent().expect("manual move directory"))
        .expect("create manual move directory");
    fs::rename(
        resumed
            .filed_original
            .as_ref()
            .expect("filed August bill")
            .final_path
            .clone(),
        &moved_destination,
    )
    .expect("move filed Original manually");
    let candidates = store
        .list_manual_move_candidates("rivera-household", &cabinet)
        .expect("list manual moves");
    assert_eq!(candidates.len(), 1);
    assert_eq!(
        candidates[0].current_destination,
        "Household records/manual/AGL bill August 2026.pdf"
    );
    store
        .record_manual_move_decision("rivera-household", resumed.id, &cabinet, false)
        .expect("keep manual move as one-off");
    assert!(store
        .list_manual_move_candidates("rivera-household", &cabinet)
        .expect("list resolved manual moves")
        .is_empty());

    let deleted = store
        .delete_filing_rule("rivera-household", rules[0].id)
        .expect("delete Filing Rule");
    assert!(deleted.deleted);
    let events = store
        .list_filing_rule_audit_events("rivera-household")
        .expect("list Filing Rule history");
    assert_eq!(events.len(), 5);
    assert_eq!(events[0].kind, luna_core::FilingRuleAuditKind::Deleted);
}
