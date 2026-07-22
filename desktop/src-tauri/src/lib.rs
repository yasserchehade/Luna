mod account_session;
mod cabinet;
mod conversation;
mod settings;
mod trusted_device;

pub use account_session::{AccountSessionError, AccountSessionStore};
pub use cabinet::{
    CabinetAvailability, CabinetConfiguration, CabinetError, CabinetManager, CabinetPreview,
    CabinetValidation,
};
pub use conversation::{
    AuditAuthority, AuditEvent, AuditEventKind, ClarificationQuestion, ConfidenceState,
    ContextField, ContextRelevanceDirection, Conversation, ConversationError, ConversationMessage,
    ConversationStore, DocumentArrival, DocumentContextDirection, DocumentContextReview,
    DocumentProcessingState, DuplicateAuditEvent, DuplicateAuditKind, DuplicateCandidate,
    DuplicateDecision, DuplicateKind, DuplicateResolution, DuplicateReview, FiledOriginal,
    FilingDecisionDirection, FilingDecisionReview,
    FilingRuleAuditEvent, FilingRuleAuditKind, FilingRuleReorganizationDocument,
    FilingRuleReorganizationPreview, FilingRuleSummary, FilingRuleUpdate, LocalOcr,
    ManualMoveCandidate, ReviewCard, ReviewEvidence, ReviewField, TesseractOcr, TodoItem,
};
pub use settings::SettingsStore;
pub use trusted_device::{
    CredentialVault, FirstDeviceEnrollment, HouseholdKeyRotation, OsCredentialVault,
    ProtectedHouseholdState, RecoveredDeviceEnrollment, RotatedDeviceEnvelope, TrustedDeviceError,
    TrustedDeviceManager, VaultError,
};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde::Serialize;
use tauri::{Manager, State};
#[cfg(not(feature = "e2e"))]
use tauri_plugin_dialog::DialogExt;

#[cfg(feature = "e2e")]
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

#[cfg(not(feature = "e2e"))]
type DeviceManager = TrustedDeviceManager<OsCredentialVault>;
#[cfg(not(feature = "e2e"))]
type AccountSessionManager = AccountSessionStore<OsCredentialVault>;
#[cfg(feature = "e2e")]
type DeviceManager = TrustedDeviceManager<E2eCredentialVault>;
#[cfg(feature = "e2e")]
type AccountSessionManager = AccountSessionStore<E2eCredentialVault>;

type CabinetState = CabinetManager;
#[cfg(not(feature = "e2e"))]
type ConversationState = ConversationStore<OsCredentialVault>;
#[cfg(feature = "e2e")]
type ConversationState = ConversationStore<E2eCredentialVault>;

#[cfg(feature = "e2e")]
#[derive(Clone, Default)]
struct E2eCredentialVault {
    secrets: Arc<Mutex<HashMap<String, Vec<u8>>>>,
}

#[cfg(feature = "e2e")]
impl CredentialVault for E2eCredentialVault {
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DeviceEnrollmentResponse {
    device_public_key: String,
    device_authorization_public_key: String,
    device_key_envelope: String,
    recovery_key: String,
    recovery_envelope: String,
    recovery_verification_key: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RecoveredDeviceResponse {
    device_public_key: String,
    device_authorization_public_key: String,
    device_key_envelope: String,
    recovery_authorization_signature: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RecoveryKeyReplacementResponse {
    recovery_key: String,
    recovery_envelope: String,
    recovery_verification_key: String,
    device_authorization_signature: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RotatedDeviceEnvelopeResponse {
    device_public_key: String,
    key_envelope: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HouseholdKeyRotationResponse {
    device_envelopes: Vec<RotatedDeviceEnvelopeResponse>,
    recovery_envelope: String,
    recovery_authorization_signature: String,
}

#[tauri::command]
fn get_account_session_item(
    store: State<'_, AccountSessionManager>,
    key: String,
) -> Result<Option<String>, String> {
    store.get(&key).map_err(|error| error.to_string())
}

#[tauri::command]
fn set_account_session_item(
    store: State<'_, AccountSessionManager>,
    key: String,
    value: String,
) -> Result<(), String> {
    store.set(&key, &value).map_err(|error| error.to_string())
}

#[tauri::command]
fn remove_account_session_item(
    store: State<'_, AccountSessionManager>,
    key: String,
) -> Result<(), String> {
    store.remove(&key).map_err(|error| error.to_string())
}

#[tauri::command]
fn get_setting(store: State<'_, SettingsStore>, key: String) -> Result<Option<String>, String> {
    store.get(&key).map_err(|error| error.to_string())
}

#[tauri::command]
fn set_setting(store: State<'_, SettingsStore>, key: String, value: String) -> Result<(), String> {
    store.set(&key, &value).map_err(|error| error.to_string())
}

#[tauri::command]
async fn select_cabinet_folder(_app: tauri::AppHandle) -> Result<Option<String>, String> {
    #[cfg(feature = "e2e")]
    {
        let folder = std::env::temp_dir().join(format!("luna-e2e-cabinet-{}", std::process::id()));
        if folder.exists() {
            std::fs::remove_dir_all(&folder).map_err(|error| error.to_string())?;
        }
        std::fs::create_dir(&folder).map_err(|error| error.to_string())?;
        Ok(Some(folder.to_string_lossy().into_owned()))
    }

    #[cfg(not(feature = "e2e"))]
    {
        _app.dialog()
            .file()
            .blocking_pick_folder()
            .map(|folder| {
                folder
                    .into_path()
                    .map(|path| path.to_string_lossy().into_owned())
                    .map_err(|error| error.to_string())
            })
            .transpose()
    }
}

#[tauri::command]
fn preview_cabinet(
    manager: State<'_, CabinetState>,
    root: String,
    sections: Vec<String>,
) -> Result<CabinetPreview, String> {
    manager
        .preview(root, &sections)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn create_cabinet(
    manager: State<'_, CabinetState>,
    household_id: String,
    preview: CabinetPreview,
) -> Result<CabinetConfiguration, String> {
    manager
        .create(&household_id, preview)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn validate_cabinet(
    manager: State<'_, CabinetState>,
    household_id: String,
) -> Result<Option<CabinetValidation>, String> {
    manager
        .validate(&household_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn create_conversation(
    store: State<'_, ConversationState>,
    household_id: String,
    title: String,
) -> Result<Conversation, String> {
    store
        .create_conversation(&household_id, &title)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_conversations(
    store: State<'_, ConversationState>,
    household_id: String,
    search: Option<String>,
    include_archived: bool,
) -> Result<Vec<Conversation>, String> {
    store
        .list_conversations(&household_id, search.as_deref(), include_archived)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn rename_conversation(
    store: State<'_, ConversationState>,
    household_id: String,
    conversation_id: i64,
    title: String,
) -> Result<(), String> {
    store
        .rename_conversation(&household_id, conversation_id, &title)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn archive_conversation(
    store: State<'_, ConversationState>,
    household_id: String,
    conversation_id: i64,
    archived: bool,
) -> Result<(), String> {
    store
        .archive_conversation(&household_id, conversation_id, archived)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn delete_conversation(
    store: State<'_, ConversationState>,
    household_id: String,
    conversation_id: i64,
) -> Result<(), String> {
    store
        .delete_conversation(&household_id, conversation_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn add_member_message(
    store: State<'_, ConversationState>,
    household_id: String,
    conversation_id: i64,
    body: String,
) -> Result<ConversationMessage, String> {
    store
        .add_member_message(&household_id, conversation_id, &body)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_conversation_messages(
    store: State<'_, ConversationState>,
    household_id: String,
    conversation_id: i64,
) -> Result<Vec<ConversationMessage>, String> {
    store
        .list_messages(&household_id, conversation_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn attach_document(
    store: State<'_, ConversationState>,
    cabinet: State<'_, CabinetState>,
    household_id: String,
    conversation_id: i64,
    path: String,
) -> Result<DocumentArrival, String> {
    let configuration = cabinet
        .load(&household_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "A Cabinet must be configured before attaching a document.".to_owned())?;
    store
        .attach_document(&household_id, conversation_id, path, configuration.root)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_document_arrivals(
    store: State<'_, ConversationState>,
    household_id: String,
) -> Result<Vec<DocumentArrival>, String> {
    store
        .list_document_arrivals(&household_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn resume_document_filings(
    store: State<'_, ConversationState>,
    cabinet: State<'_, CabinetState>,
    household_id: String,
) -> Result<(), String> {
    let configuration = cabinet
        .load(&household_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| {
            "A Cabinet must be configured before resuming document filing.".to_owned()
        })?;
    store
        .resume_document_filings(&household_id, configuration.root)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_todo_items(
    store: State<'_, ConversationState>,
    household_id: String,
) -> Result<Vec<TodoItem>, String> {
    store
        .list_todo_items(&household_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_filed_originals(
    store: State<'_, ConversationState>,
    household_id: String,
) -> Result<Vec<FiledOriginal>, String> {
    store
        .list_filed_originals(&household_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_audit_events(
    store: State<'_, ConversationState>,
    household_id: String,
) -> Result<Vec<AuditEvent>, String> {
    store
        .list_audit_events(&household_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_duplicate_audit_events(
    store: State<'_, ConversationState>,
    household_id: String,
) -> Result<Vec<DuplicateAuditEvent>, String> {
    store
        .list_duplicate_audit_events(&household_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn resolve_duplicate(
    store: State<'_, ConversationState>,
    household_id: String,
    arrival_id: i64,
    related_arrival_id: i64,
    decision: DuplicateDecision,
    remember_preference: bool,
) -> Result<DocumentArrival, String> {
    store
        .resolve_duplicate(
            &household_id,
            arrival_id,
            related_arrival_id,
            decision,
            remember_preference,
        )
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn dismiss_document_arrival(
    store: State<'_, ConversationState>,
    household_id: String,
    arrival_id: i64,
) -> Result<(), String> {
    store
        .dismiss_document_arrival(&household_id, arrival_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn record_member_direction(
    store: State<'_, ConversationState>,
    cabinet: State<'_, CabinetState>,
    household_id: String,
    arrival_id: i64,
    direction: DocumentContextDirection,
) -> Result<DocumentArrival, String> {
    let configuration = cabinet
        .load(&household_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "A Cabinet must be configured before choosing a destination.".to_owned())?;
    let cabinet_section = configuration
        .sections
        .first()
        .ok_or_else(|| "The Cabinet has no filing sections.".to_owned())?;
    store
        .record_member_direction(&household_id, arrival_id, direction, cabinet_section)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn confirm_filing_decision(
    store: State<'_, ConversationState>,
    cabinet: State<'_, CabinetState>,
    household_id: String,
    arrival_id: i64,
    direction: FilingDecisionDirection,
) -> Result<DocumentArrival, String> {
    let configuration = cabinet
        .load(&household_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "A Cabinet must be configured before filing a document.".to_owned())?;
    store
        .confirm_filing_decision(&household_id, arrival_id, direction)
        .map_err(|error| error.to_string())?;
    store
        .file_document(&household_id, arrival_id, configuration.root)
        .map_err(|error| error.to_string())
}

#[cfg(feature = "e2e")]
fn e2e_digital_pdf() -> Vec<u8> {
    e2e_pdf_with_text("Luna E2E fixture")
}

#[tauri::command]
fn list_filing_rules(
    store: State<'_, ConversationState>,
    household_id: String,
) -> Result<Vec<FilingRuleSummary>, String> {
    store
        .list_filing_rules(&household_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn update_filing_rule(
    store: State<'_, ConversationState>,
    household_id: String,
    rule_id: i64,
    update: FilingRuleUpdate,
) -> Result<FilingRuleSummary, String> {
    store
        .update_filing_rule(&household_id, rule_id, update)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn pause_filing_rule(
    store: State<'_, ConversationState>,
    household_id: String,
    rule_id: i64,
    paused: bool,
) -> Result<FilingRuleSummary, String> {
    store
        .pause_filing_rule(&household_id, rule_id, paused)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn delete_filing_rule(
    store: State<'_, ConversationState>,
    household_id: String,
    rule_id: i64,
) -> Result<FilingRuleSummary, String> {
    store
        .delete_filing_rule(&household_id, rule_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_filing_rule_audit_events(
    store: State<'_, ConversationState>,
    household_id: String,
) -> Result<Vec<FilingRuleAuditEvent>, String> {
    store
        .list_filing_rule_audit_events(&household_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn preview_filing_rule_reorganization(
    store: State<'_, ConversationState>,
    household_id: String,
    rule_id: i64,
    proposed_directory: String,
) -> Result<FilingRuleReorganizationPreview, String> {
    store
        .preview_filing_rule_reorganization(&household_id, rule_id, &proposed_directory)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_manual_move_candidates(
    store: State<'_, ConversationState>,
    cabinet: State<'_, CabinetState>,
    household_id: String,
) -> Result<Vec<ManualMoveCandidate>, String> {
    let configuration = cabinet
        .load(&household_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "A Cabinet must be configured before checking manual moves.".to_owned())?;
    store
        .list_manual_move_candidates(&household_id, configuration.root)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn record_manual_move_decision(
    store: State<'_, ConversationState>,
    cabinet: State<'_, CabinetState>,
    household_id: String,
    arrival_id: i64,
    teaches_rule: bool,
) -> Result<DocumentArrival, String> {
    let configuration = cabinet
        .load(&household_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "A Cabinet must be configured before recording a manual move.".to_owned())?;
    store
        .record_manual_move_decision(&household_id, arrival_id, configuration.root, teaches_rule)
        .map_err(|error| error.to_string())
}

#[cfg(feature = "e2e")]
fn e2e_pdf_with_text(text: &str) -> Vec<u8> {
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

#[cfg(feature = "e2e")]
#[tauri::command]
fn select_e2e_context_document_file(kind: String) -> Result<String, String> {
    let text = match kind.as_str() {
        "matching" => "Document Type: Electricity bill; Service Provider: Mercury Energy; Addressee: Sam Rivera; Property: 12 Seabreeze Avenue; Account: 12345678; Relevant Date: 2026-08-15",
        "rule-match" => "Document Type: Electricity bill; Service Provider: AGL; Addressee: Sam Rivera; Property: 12 Seabreeze Avenue; Account: 12345678; Relevant Date: 2026-08-16",
        "changed-provider" => "Document Type: Electricity bill; Service Provider: Origin Energy; Addressee: Sam Rivera; Property: 12 Seabreeze Avenue; Account: 12345678; Relevant Date: 2026-09-15",
        _ => return Err("Unknown E2E context document kind.".to_owned()),
    };
    let document = std::env::temp_dir().join(format!(
        "luna-e2e-context-{}-{kind}.pdf",
        std::process::id()
    ));
    std::fs::write(&document, e2e_pdf_with_text(text)).map_err(|error| error.to_string())?;
    Ok(document.to_string_lossy().into_owned())
}

#[tauri::command]
fn select_document_files(_app: tauri::AppHandle) -> Result<Vec<String>, String> {
    #[cfg(feature = "e2e")]
    {
        let document =
            std::env::temp_dir().join(format!("luna-e2e-document-{}.pdf", std::process::id()));
        std::fs::write(&document, e2e_digital_pdf()).map_err(|error| error.to_string())?;
        Ok(vec![document.to_string_lossy().into_owned()])
    }

    #[cfg(not(feature = "e2e"))]
    {
        _app.dialog()
            .file()
            .add_filter("Documents", &["pdf", "jpg", "jpeg", "png"])
            .blocking_pick_files()
            .unwrap_or_default()
            .into_iter()
            .map(|file| {
                file.into_path()
                    .map(|path| path.to_string_lossy().into_owned())
                    .map_err(|error| error.to_string())
            })
            .collect()
    }
}

#[tauri::command]
fn is_current_device_trusted(
    manager: State<'_, DeviceManager>,
    household_id: String,
) -> Result<bool, String> {
    manager
        .is_current_device_trusted(&household_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn is_current_device_unlocked(
    manager: State<'_, DeviceManager>,
    household_id: String,
) -> Result<bool, String> {
    manager
        .is_current_device_unlocked(&household_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn current_device_public_key(
    manager: State<'_, DeviceManager>,
    household_id: String,
) -> Result<String, String> {
    manager
        .current_device_public_key(&household_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn current_key_epoch(
    manager: State<'_, DeviceManager>,
    household_id: String,
) -> Result<u32, String> {
    manager
        .current_key_epoch(&household_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn set_current_key_epoch(
    manager: State<'_, DeviceManager>,
    household_id: String,
    key_epoch: u32,
) -> Result<(), String> {
    manager
        .set_current_key_epoch(&household_id, key_epoch)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn configure_device_pin(
    manager: State<'_, DeviceManager>,
    household_id: String,
    pin: String,
) -> Result<(), String> {
    manager
        .configure_device_pin(&household_id, &pin)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn unlock_device(
    manager: State<'_, DeviceManager>,
    household_id: String,
    pin: String,
) -> Result<(), String> {
    manager
        .unlock_device(&household_id, &pin)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn lock_device(manager: State<'_, DeviceManager>, household_id: String) {
    manager.lock_device(&household_id);
}

#[tauri::command]
fn forget_current_device(
    manager: State<'_, DeviceManager>,
    household_id: String,
) -> Result<(), String> {
    manager
        .forget_current_device(&household_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn enrol_first_device(
    manager: State<'_, DeviceManager>,
    household_id: String,
) -> Result<DeviceEnrollmentResponse, String> {
    let enrollment = manager
        .enrol_first_device(&household_id)
        .map_err(|error| error.to_string())?;
    Ok(DeviceEnrollmentResponse {
        device_public_key: enrollment.device_public_key,
        device_authorization_public_key: BASE64.encode(enrollment.device_authorization_public_key),
        device_key_envelope: BASE64.encode(enrollment.device_key_envelope),
        recovery_key: enrollment.recovery_key,
        recovery_envelope: BASE64.encode(enrollment.recovery_envelope),
        recovery_verification_key: BASE64.encode(enrollment.recovery_verification_key),
    })
}

#[tauri::command]
fn confirm_recovery_key(
    manager: State<'_, DeviceManager>,
    household_id: String,
    recovery_key: String,
    recovery_envelope: String,
) -> Result<(), String> {
    let envelope = BASE64
        .decode(recovery_envelope)
        .map_err(|_| "The Recovery Key envelope is invalid.".to_owned())?;
    manager
        .confirm_recovery_key(&household_id, &recovery_key, &envelope)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn recover_device(
    manager: State<'_, DeviceManager>,
    household_id: String,
    recovery_key: String,
    recovery_envelope: String,
    key_epoch: u32,
) -> Result<RecoveredDeviceResponse, String> {
    let envelope = BASE64
        .decode(recovery_envelope)
        .map_err(|_| "The Recovery Key envelope is invalid.".to_owned())?;
    let recovered = manager
        .recover_device(&household_id, &recovery_key, &envelope, key_epoch)
        .map_err(|error| error.to_string())?;
    Ok(RecoveredDeviceResponse {
        device_public_key: recovered.device_public_key,
        device_authorization_public_key: BASE64.encode(recovered.device_authorization_public_key),
        device_key_envelope: BASE64.encode(recovered.device_key_envelope),
        recovery_authorization_signature: BASE64.encode(recovered.recovery_authorization_signature),
    })
}

#[tauri::command]
fn prepare_recovery_key_replacement(
    manager: State<'_, DeviceManager>,
    household_id: String,
    current_key_epoch: u32,
    current_recovery_verification_key: String,
) -> Result<RecoveryKeyReplacementResponse, String> {
    let current_recovery_verification_key: [u8; 32] = BASE64
        .decode(current_recovery_verification_key)
        .map_err(|_| "The current Recovery Key verifier is invalid.".to_owned())?
        .try_into()
        .map_err(|_| "The current Recovery Key verifier is invalid.".to_owned())?;
    let replacement = manager
        .prepare_recovery_key_replacement(
            &household_id,
            current_key_epoch,
            &current_recovery_verification_key,
        )
        .map_err(|error| error.to_string())?;
    Ok(RecoveryKeyReplacementResponse {
        recovery_key: replacement.recovery_key,
        recovery_envelope: BASE64.encode(replacement.recovery_envelope),
        recovery_verification_key: BASE64.encode(replacement.recovery_verification_key),
        device_authorization_signature: BASE64.encode(replacement.device_authorization_signature),
    })
}

#[tauri::command]
fn confirm_recovery_key_replacement(
    manager: State<'_, DeviceManager>,
    household_id: String,
    recovery_key: String,
    recovery_envelope: String,
) -> Result<(), String> {
    let envelope = BASE64
        .decode(recovery_envelope)
        .map_err(|_| "The replacement Recovery Key envelope is invalid.".to_owned())?;
    manager
        .confirm_recovery_key_replacement(&household_id, &recovery_key, &envelope)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn finalize_recovered_device(
    manager: State<'_, DeviceManager>,
    household_id: String,
    key_epoch: u32,
) -> Result<(), String> {
    manager
        .finalize_recovered_device(&household_id, key_epoch)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn prepare_household_key_rotation(
    manager: State<'_, DeviceManager>,
    household_id: String,
    recovery_key: String,
    recovery_envelope: String,
    retained_device_public_keys: Vec<String>,
    current_key_epoch: u32,
    revoked_device_id: String,
) -> Result<HouseholdKeyRotationResponse, String> {
    let envelope = BASE64
        .decode(recovery_envelope)
        .map_err(|_| "The Recovery Key envelope is invalid.".to_owned())?;
    let rotation = manager
        .prepare_household_key_rotation_after_revocation(
            &household_id,
            &recovery_key,
            &envelope,
            &retained_device_public_keys,
            current_key_epoch,
            &revoked_device_id,
        )
        .map_err(|error| error.to_string())?;
    Ok(HouseholdKeyRotationResponse {
        device_envelopes: rotation
            .device_envelopes
            .into_iter()
            .map(|device| RotatedDeviceEnvelopeResponse {
                device_public_key: device.device_public_key,
                key_envelope: BASE64.encode(device.key_envelope),
            })
            .collect(),
        recovery_envelope: BASE64.encode(rotation.recovery_envelope),
        recovery_authorization_signature: BASE64.encode(rotation.recovery_authorization_signature),
    })
}

#[tauri::command]
fn finalize_household_key_rotation(
    manager: State<'_, DeviceManager>,
    household_id: String,
    key_epoch: u32,
) -> Result<(), String> {
    manager
        .finalize_household_key_rotation(&household_id, key_epoch)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn discard_household_key_rotation(
    manager: State<'_, DeviceManager>,
    household_id: String,
) -> Result<(), String> {
    manager
        .discard_household_key_rotation(&household_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn apply_rotated_device_envelope(
    manager: State<'_, DeviceManager>,
    household_id: String,
    key_envelope: String,
    key_epoch: u32,
) -> Result<(), String> {
    let envelope = BASE64
        .decode(key_envelope)
        .map_err(|_| "The Trusted Device key envelope is invalid.".to_owned())?;
    manager
        .apply_rotated_device_envelope(&household_id, &envelope, key_epoch)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn protect_household_state(
    manager: State<'_, DeviceManager>,
    household_id: String,
    plaintext: String,
) -> Result<ProtectedHouseholdState, String> {
    manager
        .protect_household_state(&household_id, plaintext.as_bytes())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn open_household_state(
    manager: State<'_, DeviceManager>,
    household_id: String,
    protected: ProtectedHouseholdState,
) -> Result<String, String> {
    let plaintext = manager
        .open_household_state(&household_id, &protected)
        .map_err(|error| error.to_string())?;
    String::from_utf8(plaintext).map_err(|_| "Protected Household state is not UTF-8.".to_owned())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default().plugin(tauri_plugin_dialog::init());
    #[cfg(all(debug_assertions, feature = "e2e"))]
    let builder = builder
        .plugin(tauri_plugin_wdio::init())
        .plugin(tauri_plugin_wdio_webdriver::init());

    let builder = builder.setup(|app| {
        #[cfg(not(feature = "e2e"))]
        let application_data = app.path().app_data_dir()?;
        #[cfg(feature = "e2e")]
        let application_data =
            std::env::temp_dir().join(format!("luna-e2e-device-{}", std::process::id()));
        #[cfg(feature = "e2e")]
        if application_data.exists() {
            std::fs::remove_dir_all(&application_data)?;
        }
        std::fs::create_dir_all(&application_data)?;
        let database = application_data.join("luna.db");
        let settings = SettingsStore::open(&database)?;
        app.manage(settings.clone());
        app.manage(CabinetManager::new(settings));
        #[cfg(not(feature = "e2e"))]
        {
            let trusted_device =
                TrustedDeviceManager::new(OsCredentialVault::new("app.luna.household"));
            app.manage(ConversationStore::open(&database, trusted_device.clone())?);
            app.manage(trusted_device);
        }
        #[cfg(not(feature = "e2e"))]
        app.manage(AccountSessionStore::new(OsCredentialVault::new(
            "app.luna.account",
        )));
        #[cfg(feature = "e2e")]
        {
            let trusted_device = TrustedDeviceManager::new(E2eCredentialVault::default());
            app.manage(ConversationStore::open(&database, trusted_device.clone())?);
            app.manage(trusted_device);
        }
        #[cfg(feature = "e2e")]
        app.manage(AccountSessionStore::new(E2eCredentialVault::default()));
        Ok(())
    });

    let builder = builder.invoke_handler(tauri::generate_handler![
        get_setting,
        set_setting,
        select_cabinet_folder,
        preview_cabinet,
        create_cabinet,
        validate_cabinet,
        create_conversation,
        list_conversations,
        rename_conversation,
        archive_conversation,
        delete_conversation,
        add_member_message,
        list_conversation_messages,
        attach_document,
        list_document_arrivals,
        resume_document_filings,
        list_todo_items,
        list_filed_originals,
        list_audit_events,
        list_duplicate_audit_events,
        resolve_duplicate,
        list_filing_rules,
        update_filing_rule,
        pause_filing_rule,
        delete_filing_rule,
        list_filing_rule_audit_events,
        preview_filing_rule_reorganization,
        list_manual_move_candidates,
        record_manual_move_decision,
        dismiss_document_arrival,
        record_member_direction,
        confirm_filing_decision,
        select_document_files,
        #[cfg(feature = "e2e")]
        select_e2e_context_document_file,
        get_account_session_item,
        set_account_session_item,
        remove_account_session_item,
        is_current_device_trusted,
        is_current_device_unlocked,
        current_device_public_key,
        current_key_epoch,
        set_current_key_epoch,
        configure_device_pin,
        unlock_device,
        lock_device,
        forget_current_device,
        enrol_first_device,
        confirm_recovery_key,
        recover_device,
        finalize_recovered_device,
        prepare_recovery_key_replacement,
        confirm_recovery_key_replacement,
        prepare_household_key_rotation,
        finalize_household_key_rotation,
        discard_household_key_rotation,
        apply_rotated_device_envelope,
        protect_household_state,
        open_household_state
    ]);

    builder
        .run(tauri::generate_context!())
        .expect("Luna failed to start");
}
