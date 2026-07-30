mod account_session;
mod cabinet;
mod conversation;
mod document_intelligence;
mod intelligence;
mod litellm;
mod portable_memory;
mod settings;
mod trusted_device;

pub use account_session::{AccountSessionError, AccountSessionStore};
pub use cabinet::{
    CabinetAvailability, CabinetConfiguration, CabinetError, CabinetManager, CabinetPreview,
    CabinetValidation,
};
pub use conversation::{
    AuditAuthority, AuditEvent, AuditEventKind, ClarificationQuestion, ConfidenceState,
    ContextField, ContextRelevanceDirection, Conversation, ConversationAction, ConversationError,
    ConversationExpectedResponse, ConversationMessage, ConversationPrompt,
    ConversationPromptPurpose, ConversationStore, ConversationTurnOutcome, ConversationTurnStatus,
    DeterministicMemberDirectionInterpreter, DirectionInterpretation, DocumentArrival,
    DocumentContextDirection, DocumentContextReview, DocumentConversationView,
    DocumentProcessingState, DuplicateAuditEvent, DuplicateAuditKind, DuplicateCandidate,
    DuplicateDecision, DuplicateKind, DuplicateResolution, DuplicateReview, FiledOriginal,
    FilingDecisionDirection, FilingDecisionReview, FilingRuleAuditEvent, FilingRuleAuditKind,
    FilingRuleReorganizationDocument, FilingRuleReorganizationPreview, FilingRuleSummary,
    FilingRuleUpdate, InterpretationConfidence, LocalOcr, ManualMoveCandidate,
    MemberDirectionCommand, MemberDirectionInterpreter, MemberUtterance,
    RebuiltDocumentRelationship, ReviewCard, ReviewEvidence, ReviewField, TesseractOcr, TodoItem,
};
pub use document_intelligence::{
    CloudAssistanceResolution, DocumentIntelligenceError, DocumentIntelligenceService,
};
pub use intelligence::{
    AdditionalIntelligenceEvidence, CandidateDirectionInterpretation, CandidateDisposition,
    CloudAssistanceAuditEvent, CloudAssistanceOutcome, CloudConsentDecision, CloudConsentScope,
    CloudIntelligenceStore, ConsentGrantKind, ConversationIntelligenceResult,
    DeterministicIntelligenceGateway, DocumentContentExcerpt, IntelligenceCapability,
    IntelligenceEvidence, IntelligenceExecutionConstraints, IntelligenceFailure,
    IntelligenceGateway, IntelligenceModelDescriptor, IntelligenceProviderDescriptor,
    IntelligenceProviderStatus, IntelligenceRequest, IntelligenceResponseSchema,
    IntelligenceResult, IntelligenceSelection, IntelligenceUsage, UntrustedIntelligenceResult,
    CONVERSATION_REPLY_PURPOSE, CURRENT_MESSAGE_FIELD, DOCUMENT_DEFAULT_PERMISSION_FIELDS,
    DOCUMENT_EVALUATION_PURPOSE, MANAGED_INTELLIGENCE_MODEL_ID, MANAGED_INTELLIGENCE_PROVIDER_ID,
};
pub use portable_memory::{
    PortableAuditEventKind, PortableAuthority, PortableAuthorizationCutoff,
    PortableCandidateDisposition, PortableConflict, PortableConflictResolutionDraft,
    PortableConsentDetails, PortableConsentField, PortableConsentGrantKind,
    PortableConsentProvider, PortableConsentPurpose, PortableConsentScope,
    PortableConsentScopeEvidence, PortableConsentState, PortableConversationReference,
    PortableDocumentRelationshipKind, PortableEvent, PortableEventDraft,
    PortableExecutionOutcomeKind, PortableFact, PortableFilingRuleDefinition,
    PortableFilingRuleState, PortableHistoryEvent, PortableHouseholdProjection,
    PortableImportReport, PortableIntelligenceCapability, PortableMemberDirectionKind,
    PortableMemoryError, PortableMemoryStore, PortableReference, TrustedDeviceAuthorization,
};
pub use settings::SettingsStore;
pub use trusted_device::{
    CredentialVault, FirstDeviceEnrollment, HouseholdKeyRotation, OsCredentialVault,
    ProtectedHouseholdState, RecoveredDeviceEnrollment, RotatedDeviceEnvelope, TrustedDeviceError,
    TrustedDeviceManager, VaultError,
};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde::{Deserialize, Serialize};
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

#[cfg(not(feature = "e2e"))]
type IntelligenceState = CloudIntelligenceStore<OsCredentialVault>;
#[cfg(feature = "e2e")]
type IntelligenceState = CloudIntelligenceStore<E2eCredentialVault>;
#[cfg(not(feature = "e2e"))]
type PortableState = PortableMemoryStore<OsCredentialVault>;
#[cfg(feature = "e2e")]
type PortableState = PortableMemoryStore<E2eCredentialVault>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DefaultIntelligenceProvider {
    provider_id: String,
    model_id: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    invalid: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OrdinaryConversationSubmission {
    member_message: ConversationMessage,
    reply: Option<ConversationMessage>,
    failure: Option<ConversationIntelligenceFailure>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConversationIntelligenceFailure {
    code: ConversationIntelligenceFailureCode,
    provider_id: Option<String>,
    provider_name: Option<String>,
    detail: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum ConversationIntelligenceFailureCode {
    NotConfigured,
    Configuration,
    ConsentRequired,
    Unavailable,
    InvalidResult,
    InvalidCredential,
    RequestRejected,
    Unexpected,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
enum DefaultIntelligencePermission {
    Conversation,
    Document,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct DefaultIntelligencePermissionUpdate {
    portable_sync_pending: bool,
}

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
fn sign_managed_intelligence_device_provisioning(
    manager: State<'_, DeviceManager>,
    household_id: String,
    nonce: String,
) -> Result<String, String> {
    manager
        .sign_managed_intelligence_device_provisioning(&household_id, &nonce)
        .map(|signature| BASE64.encode(signature))
        .map_err(|error| error.to_string())
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

    #[cfg(all(feature = "live-canary", not(feature = "e2e")))]
    {
        let folder = std::env::var_os("LUNA_LIVE_CANARY_CABINET_DIR")
            .map(std::path::PathBuf::from)
            .ok_or_else(|| "LUNA_LIVE_CANARY_CABINET_DIR is required.".to_owned())?;
        std::fs::create_dir_all(&folder).map_err(|error| error.to_string())?;
        Ok(Some(folder.to_string_lossy().into_owned()))
    }

    #[cfg(not(any(feature = "e2e", feature = "live-canary")))]
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
    intelligence: State<'_, IntelligenceState>,
    portable: State<'_, PortableState>,
    cabinet: State<'_, CabinetState>,
    household_id: String,
    conversation_id: i64,
    path: String,
) -> Result<DocumentArrival, String> {
    let configuration = cabinet
        .load(&household_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "A Cabinet must be configured before attaching a document.".to_owned())?;
    let arrival = store
        .attach_document(&household_id, conversation_id, path, &configuration.root)
        .map_err(|error| error.to_string())?;
    capture_portable_state(
        portable.inner(),
        store.inner(),
        intelligence.inner(),
        cabinet.inner(),
        &household_id,
    )?;
    Ok(arrival)
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
    intelligence: State<'_, IntelligenceState>,
    portable: State<'_, PortableState>,
    cabinet: State<'_, CabinetState>,
    household_id: String,
) -> Result<(), String> {
    let configuration = cabinet
        .load(&household_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| {
            "A Cabinet must be configured before resuming document filing.".to_owned()
        })?;
    let resume_result = store
        .resume_document_filings(&household_id, configuration.root)
        .map_err(|error| error.to_string());
    let capture_result = capture_portable_state(
        portable.inner(),
        store.inner(),
        intelligence.inner(),
        cabinet.inner(),
        &household_id,
    );
    resume_result.and(capture_result)
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
fn list_intelligence_providers(
    store: State<'_, IntelligenceState>,
) -> Vec<IntelligenceProviderDescriptor> {
    store.providers()
}

#[tauri::command]
fn list_intelligence_provider_statuses(
    store: State<'_, IntelligenceState>,
    household_id: String,
) -> Result<Vec<IntelligenceProviderStatus>, String> {
    store
        .provider_statuses(&household_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn get_default_intelligence_provider(
    settings: State<'_, SettingsStore>,
    sessions: State<'_, AccountSessionManager>,
    household_id: String,
) -> Result<Option<DefaultIntelligenceProvider>, String> {
    current_household_actor(sessions.inner(), &household_id)?;
    read_saved_default_intelligence_provider(settings.inner(), &household_id)
}

#[tauri::command]
fn set_default_intelligence_provider(
    settings: State<'_, SettingsStore>,
    intelligence: State<'_, IntelligenceState>,
    sessions: State<'_, AccountSessionManager>,
    household_id: String,
    provider_id: String,
    model_id: String,
) -> Result<DefaultIntelligenceProvider, String> {
    current_household_actor(sessions.inner(), &household_id)?;
    let selected = DefaultIntelligenceProvider {
        provider_id,
        model_id,
        invalid: false,
    };
    if !selected_provider_is_available(intelligence.inner(), &household_id, &selected)? {
        return Err(
            "Connect the exact Intelligence Provider and model before making it the default."
                .to_owned(),
        );
    }
    let previous = read_saved_default_intelligence_provider(settings.inner(), &household_id)?;
    if previous.as_ref() != Some(&selected) {
        if let Some(previous) = previous.as_ref() {
            revoke_default_intelligence_permissions(intelligence.inner(), &household_id, previous)?;
        }
        revoke_default_intelligence_permissions(intelligence.inner(), &household_id, &selected)?;
    }
    let value = serde_json::to_string(&selected)
        .map_err(|_| "Default Intelligence settings are invalid.".to_owned())?;
    settings
        .set(&default_intelligence_key(&household_id), &value)
        .map_err(|_| "Default Intelligence settings could not be saved.".to_owned())?;
    Ok(selected)
}

#[tauri::command]
fn clear_default_intelligence_provider(
    settings: State<'_, SettingsStore>,
    intelligence: State<'_, IntelligenceState>,
    sessions: State<'_, AccountSessionManager>,
    household_id: String,
) -> Result<(), String> {
    current_household_actor(sessions.inner(), &household_id)?;
    if let Some(selected) =
        read_saved_default_intelligence_provider(settings.inner(), &household_id)?
    {
        revoke_default_intelligence_permissions(intelligence.inner(), &household_id, &selected)?;
    }
    settings
        .delete(&default_intelligence_key(&household_id))
        .map_err(|_| "Default Intelligence settings could not be cleared.".to_owned())
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
fn set_default_intelligence_permission(
    settings: State<'_, SettingsStore>,
    intelligence: State<'_, IntelligenceState>,
    conversations: State<'_, ConversationState>,
    portable: State<'_, PortableState>,
    cabinet: State<'_, CabinetState>,
    sessions: State<'_, AccountSessionManager>,
    household_id: String,
    permission: DefaultIntelligencePermission,
    enabled: bool,
) -> Result<DefaultIntelligencePermissionUpdate, String> {
    let granted_by = current_household_actor(sessions.inner(), &household_id)?;
    let selected =
        load_default_intelligence_provider(settings.inner(), intelligence.inner(), &household_id)?
            .ok_or_else(|| "Choose a default Intelligence Provider in Options.".to_owned())?;
    let matching = intelligence
        .verified_consent_scopes(&household_id)
        .map_err(|error| error.to_string())?
        .into_iter()
        .filter(|scope| default_permission_matches(scope, &selected, permission))
        .collect::<Vec<_>>();
    if enabled && matching.is_empty() {
        let (capability, purpose, fields) = match permission {
            DefaultIntelligencePermission::Conversation => (
                IntelligenceCapability::ConversationReply,
                CONVERSATION_REPLY_PURPOSE,
                vec![CURRENT_MESSAGE_FIELD.to_owned()],
            ),
            DefaultIntelligencePermission::Document => (
                IntelligenceCapability::DirectionInterpretation,
                DOCUMENT_EVALUATION_PURPOSE,
                DOCUMENT_DEFAULT_PERMISSION_FIELDS
                    .iter()
                    .map(|field| (*field).to_owned())
                    .collect(),
            ),
        };
        intelligence
            .grant_scope(
                &household_id,
                &IntelligenceSelection {
                    provider_id: selected.provider_id,
                    model_id: selected.model_id,
                },
                capability,
                purpose,
                fields,
                Vec::new(),
                &granted_by,
            )
            .map_err(|error| error.to_string())?;
    } else if !enabled {
        for scope in matching {
            intelligence
                .revoke_scope(&household_id, scope.id)
                .map_err(|error| error.to_string())?;
        }
    }
    let portable_sync_pending = capture_portable_state(
        portable.inner(),
        conversations.inner(),
        intelligence.inner(),
        cabinet.inner(),
        &household_id,
    )
    .is_err();
    Ok(DefaultIntelligencePermissionUpdate {
        portable_sync_pending,
    })
}

#[tauri::command]
fn submit_ordinary_conversation_message(
    intelligence: State<'_, IntelligenceState>,
    settings: State<'_, SettingsStore>,
    conversations: State<'_, ConversationState>,
    sessions: State<'_, AccountSessionManager>,
    household_id: String,
    conversation_id: i64,
    body: String,
) -> Result<OrdinaryConversationSubmission, String> {
    let granted_by = current_household_actor(sessions.inner(), &household_id)?;
    let member_message = conversations
        .add_member_message(&household_id, conversation_id, &body)
        .map_err(|error| error.to_string())?;
    let selected = match load_default_intelligence_provider(
        settings.inner(),
        intelligence.inner(),
        &household_id,
    ) {
        Ok(Some(selected)) => selected,
        Ok(None) => {
            return Ok(OrdinaryConversationSubmission {
                member_message,
                reply: None,
                failure: Some(ConversationIntelligenceFailure {
                    code: ConversationIntelligenceFailureCode::NotConfigured,
                    provider_id: None,
                    provider_name: None,
                    detail: "Choose a default Intelligence Provider in Options.".to_owned(),
                }),
            });
        }
        Err(detail) => {
            return Ok(OrdinaryConversationSubmission {
                member_message,
                reply: None,
                failure: Some(ConversationIntelligenceFailure {
                    code: ConversationIntelligenceFailureCode::Configuration,
                    provider_id: None,
                    provider_name: None,
                    detail,
                }),
            });
        }
    };
    let provider_name = intelligence
        .providers()
        .into_iter()
        .find(|provider| provider.id == selected.provider_id)
        .map(|provider| provider.name);
    let permission = intelligence
        .verified_consent_scopes(&household_id)
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|scope| {
            default_permission_matches(
                scope,
                &selected,
                DefaultIntelligencePermission::Conversation,
            )
        });
    let Some(permission) = permission else {
        return Ok(OrdinaryConversationSubmission {
            member_message,
            reply: None,
            failure: Some(ConversationIntelligenceFailure {
                code: ConversationIntelligenceFailureCode::ConsentRequired,
                provider_id: Some(selected.provider_id),
                provider_name,
                detail: "Enable Conversation permission in Options.".to_owned(),
            }),
        });
    };
    let provider_id = selected.provider_id.clone();
    let result: Result<ConversationIntelligenceResult, IntelligenceFailure> = intelligence
        .reply_to_conversation(
            &household_id,
            IntelligenceSelection {
                provider_id: selected.provider_id,
                model_id: selected.model_id,
            },
            conversation_id,
            member_message.id,
            &member_message.body,
            &granted_by,
            permission.id,
        );
    match result {
        Ok(result) => {
            match conversations.add_luna_message(&household_id, conversation_id, &result.reply) {
                Ok(reply) => Ok(OrdinaryConversationSubmission {
                    member_message,
                    reply: Some(reply),
                    failure: None,
                }),
                Err(_) => Ok(OrdinaryConversationSubmission {
                    member_message,
                    reply: None,
                    failure: Some(ConversationIntelligenceFailure {
                        code: ConversationIntelligenceFailureCode::Unexpected,
                        provider_id: Some(result.provider_id),
                        provider_name,
                        detail: "Luna received a reply but could not save it safely.".to_owned(),
                    }),
                }),
            }
        }
        Err(error) => Ok(OrdinaryConversationSubmission {
            member_message,
            reply: None,
            failure: Some(conversation_intelligence_failure(
                error,
                Some(provider_id),
                provider_name,
            )),
        }),
    }
}

#[tauri::command]
fn test_and_set_intelligence_provider_credential(
    store: State<'_, IntelligenceState>,
    sessions: State<'_, AccountSessionManager>,
    household_id: String,
    provider_id: String,
    credential: String,
) -> Result<(), String> {
    current_household_actor(&sessions, &household_id)?;
    store
        .test_and_set_provider_credential(&household_id, &provider_id, credential.trim().as_bytes())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn clear_intelligence_provider_credential(
    store: State<'_, IntelligenceState>,
    sessions: State<'_, AccountSessionManager>,
    household_id: String,
    provider_id: String,
) -> Result<(), String> {
    current_household_actor(&sessions, &household_id)?;
    store
        .clear_provider_credential(&household_id, &provider_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn set_managed_intelligence_gateway_credential(
    store: State<'_, IntelligenceState>,
    sessions: State<'_, AccountSessionManager>,
    household_id: String,
    credential: String,
) -> Result<(), String> {
    current_household_actor(&sessions, &household_id)?;
    store
        .set_gateway_access_credential(&household_id, credential.trim().as_bytes())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn clear_managed_intelligence_gateway_credential(
    store: State<'_, IntelligenceState>,
    sessions: State<'_, AccountSessionManager>,
    household_id: String,
) -> Result<(), String> {
    current_household_actor(&sessions, &household_id)?;
    store
        .clear_gateway_access_credential(&household_id)
        .map_err(|error| error.to_string())
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct CloudAssistanceCommand {
    household_id: String,
    arrival_id: i64,
    selection: IntelligenceSelection,
    consent: CloudConsentDecision,
    existing_consent_grant_id: Option<i64>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredHouseholdSession {
    account_id: String,
    household_id: String,
}

fn current_household_actor(
    sessions: &AccountSessionManager,
    household_id: &str,
) -> Result<String, String> {
    let stored = sessions
        .get("luna-household-session")
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "A verified Luna Account session is required.".to_owned())?;
    let session: StoredHouseholdSession = serde_json::from_str(&stored)
        .map_err(|_| "The stored Luna Household session is invalid.".to_owned())?;
    if session.household_id != household_id {
        return Err("The Luna Account session does not belong to this Household.".to_owned());
    }
    Ok(session.account_id)
}

fn default_intelligence_key(household_id: &str) -> String {
    format!("luna.intelligence.default.{household_id}")
}

fn read_saved_default_intelligence_provider(
    settings: &SettingsStore,
    household_id: &str,
) -> Result<Option<DefaultIntelligenceProvider>, String> {
    let Some(value) = settings
        .get(&default_intelligence_key(household_id))
        .map_err(|_| "Default Intelligence settings are unavailable.".to_owned())?
    else {
        return Ok(None);
    };
    let selected =
        serde_json::from_str::<DefaultIntelligenceProvider>(&value).unwrap_or_else(|_| {
            let recoverable = serde_json::from_str::<serde_json::Value>(&value).ok();
            DefaultIntelligenceProvider {
                provider_id: recoverable
                    .as_ref()
                    .and_then(|value| value.get("providerId"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                model_id: recoverable
                    .as_ref()
                    .and_then(|value| value.get("modelId"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                invalid: true,
            }
        });
    Ok(Some(selected))
}

fn selected_provider_is_available(
    intelligence: &IntelligenceState,
    household_id: &str,
    selected: &DefaultIntelligenceProvider,
) -> Result<bool, String> {
    intelligence
        .provider_statuses(household_id)
        .map_err(|error| error.to_string())
        .map(|statuses| {
            statuses.into_iter().any(|status| {
                status.configured
                    && status.descriptor.id == selected.provider_id
                    && status
                        .descriptor
                        .models
                        .iter()
                        .any(|model| model.id == selected.model_id)
            })
        })
}

fn load_default_intelligence_provider(
    settings: &SettingsStore,
    intelligence: &IntelligenceState,
    household_id: &str,
) -> Result<Option<DefaultIntelligenceProvider>, String> {
    let Some(selected) = read_saved_default_intelligence_provider(settings, household_id)? else {
        return Ok(None);
    };
    if selected.invalid {
        return Err("The saved default Intelligence settings are invalid.".to_owned());
    }
    if !selected_provider_is_available(intelligence, household_id, &selected)? {
        return Err(
            "The saved default Intelligence Provider or model is unavailable on this Trusted Device."
                .to_owned(),
        );
    }
    Ok(Some(selected))
}

fn default_permission_matches(
    scope: &CloudConsentScope,
    selected: &DefaultIntelligenceProvider,
    permission: DefaultIntelligencePermission,
) -> bool {
    if scope.revoked
        || !scope.default_permission
        || scope.kind != ConsentGrantKind::Reusable
        || scope.provider_id != selected.provider_id
        || scope.model_id != selected.model_id
    {
        return false;
    }
    match permission {
        DefaultIntelligencePermission::Conversation => {
            scope.capability == IntelligenceCapability::ConversationReply
                && scope.purpose == CONVERSATION_REPLY_PURPOSE
                && scope.fields == [CURRENT_MESSAGE_FIELD.to_owned()]
        }
        DefaultIntelligencePermission::Document => {
            scope.capability == IntelligenceCapability::DirectionInterpretation
                && scope.purpose == DOCUMENT_EVALUATION_PURPOSE
                && scope.fields.len() == DOCUMENT_DEFAULT_PERMISSION_FIELDS.len()
                && DOCUMENT_DEFAULT_PERMISSION_FIELDS
                    .iter()
                    .all(|field| scope.fields.iter().any(|allowed| allowed == field))
        }
    }
}

fn revoke_default_intelligence_permissions(
    intelligence: &IntelligenceState,
    household_id: &str,
    selected: &DefaultIntelligenceProvider,
) -> Result<(), String> {
    for scope in intelligence
        .verified_consent_scopes(household_id)
        .map_err(|error| error.to_string())?
        .into_iter()
        .filter(|scope| {
            !scope.revoked
                && !selected.provider_id.is_empty()
                && scope.provider_id == selected.provider_id
                && ((scope.capability == IntelligenceCapability::ConversationReply
                    && scope.purpose == CONVERSATION_REPLY_PURPOSE)
                    || (scope.capability == IntelligenceCapability::DirectionInterpretation
                        && scope.purpose == DOCUMENT_EVALUATION_PURPOSE))
        })
    {
        intelligence
            .revoke_scope(household_id, scope.id)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn conversation_intelligence_failure(
    error: IntelligenceFailure,
    provider_id: Option<String>,
    provider_name: Option<String>,
) -> ConversationIntelligenceFailure {
    let (code, detail) = match error {
        IntelligenceFailure::ConsentRequired
        | IntelligenceFailure::ConsentRevoked
        | IntelligenceFailure::ConsentConsumed
        | IntelligenceFailure::LocalOnlyPolicy => (
            ConversationIntelligenceFailureCode::ConsentRequired,
            "Conversation permission is not active.".to_owned(),
        ),
        IntelligenceFailure::AuthenticationUnavailable => (
            ConversationIntelligenceFailureCode::NotConfigured,
            "Managed Intelligence is not ready on this Trusted Device.".to_owned(),
        ),
        IntelligenceFailure::ProviderAuthenticationUnavailable => (
            ConversationIntelligenceFailureCode::InvalidCredential,
            "The selected Intelligence Provider credential is missing, invalid, or revoked."
                .to_owned(),
        ),
        IntelligenceFailure::ProviderUnavailable
        | IntelligenceFailure::GatewayUnavailable
        | IntelligenceFailure::RateLimited
        | IntelligenceFailure::TimedOut => (
            ConversationIntelligenceFailureCode::Unavailable,
            "The selected Intelligence Provider could not complete the request.".to_owned(),
        ),
        IntelligenceFailure::InvalidStructuredResult => (
            ConversationIntelligenceFailureCode::InvalidResult,
            "Luna rejected an invalid Intelligence Provider reply.".to_owned(),
        ),
        IntelligenceFailure::ProviderRejectedRequest => (
            ConversationIntelligenceFailureCode::RequestRejected,
            "The selected Intelligence Provider rejected the request.".to_owned(),
        ),
        IntelligenceFailure::UnsupportedSelection
        | IntelligenceFailure::UnsupportedCapability
        | IntelligenceFailure::ProtectedStateUnavailable
        | IntelligenceFailure::StorageUnavailable => (
            ConversationIntelligenceFailureCode::Configuration,
            "The saved Intelligence configuration is unavailable.".to_owned(),
        ),
        IntelligenceFailure::RequestCancelled => (
            ConversationIntelligenceFailureCode::Unexpected,
            "The Intelligence request was cancelled.".to_owned(),
        ),
    };
    ConversationIntelligenceFailure {
        code,
        provider_id,
        provider_name,
        detail,
    }
}

fn capture_portable_state(
    portable: &PortableState,
    conversations: &ConversationState,
    intelligence: &IntelligenceState,
    cabinet: &CabinetState,
    household_id: &str,
) -> Result<(), String> {
    let configuration = cabinet
        .load(household_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| {
            "A Cabinet must be configured before synchronising Household memory.".to_owned()
        })?;
    match portable.capture_owned_state(
        household_id,
        configuration.root,
        conversations,
        intelligence,
    ) {
        Ok(()) | Err(PortableMemoryError::CabinetUnavailable) => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct PortableAuthorizationInput {
    device_id: String,
    authorization_public_key: String,
    activated_key_epoch: u32,
    revoked_after: Option<PortableAuthorizationCutoff>,
}

impl TryFrom<PortableAuthorizationInput> for TrustedDeviceAuthorization {
    type Error = String;

    fn try_from(value: PortableAuthorizationInput) -> Result<Self, Self::Error> {
        let authorization_public_key: [u8; 32] = BASE64
            .decode(value.authorization_public_key)
            .map_err(|_| "A Trusted Device authorization key is invalid.".to_owned())?
            .try_into()
            .map_err(|_| "A Trusted Device authorization key is invalid.".to_owned())?;
        Ok(Self {
            device_id: value.device_id,
            authorization_public_key,
            activated_key_epoch: value.activated_key_epoch,
            revoked_after: value.revoked_after,
        })
    }
}

#[tauri::command]
fn synchronize_portable_memory(
    portable: State<'_, PortableState>,
    conversations: State<'_, ConversationState>,
    intelligence: State<'_, IntelligenceState>,
    cabinet: State<'_, CabinetState>,
    household_id: String,
    trusted_devices: Vec<PortableAuthorizationInput>,
) -> Result<PortableImportReport, String> {
    let configuration = cabinet
        .load(&household_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| {
            "A Cabinet must be configured before synchronising Household memory.".to_owned()
        })?;
    let trusted_devices = trusted_devices
        .into_iter()
        .map(TrustedDeviceAuthorization::try_from)
        .collect::<Result<Vec<_>, _>>()?;
    portable
        .synchronize_owned_state(
            &household_id,
            configuration.root,
            &trusted_devices,
            conversations.inner(),
            intelligence.inner(),
        )
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_portable_history_events(
    conversations: State<'_, ConversationState>,
    household_id: String,
) -> Result<Vec<PortableHistoryEvent>, String> {
    conversations
        .list_rebuilt_portable_history(&household_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn portable_authorization_cutoff(
    portable: State<'_, PortableState>,
    household_id: String,
    device_id: String,
) -> Result<Option<PortableAuthorizationCutoff>, String> {
    portable
        .authorization_cutoff(&household_id, &device_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn evaluate_document_with_cloud_assistance(
    conversations: State<'_, ConversationState>,
    intelligence: State<'_, IntelligenceState>,
    portable: State<'_, PortableState>,
    cabinet: State<'_, CabinetState>,
    sessions: State<'_, AccountSessionManager>,
    input: CloudAssistanceCommand,
) -> Result<CloudAssistanceResolution, String> {
    let granted_by = current_household_actor(sessions.inner(), &input.household_id)?;
    let household_id = input.household_id.clone();
    let result = DocumentIntelligenceService::new(
        conversations.inner().clone(),
        intelligence.inner().clone(),
    )
    .evaluate_document(
        &input.household_id,
        input.arrival_id,
        input.selection,
        input.consent,
        &granted_by,
        input.existing_consent_grant_id,
    );
    capture_portable_state(
        portable.inner(),
        conversations.inner(),
        intelligence.inner(),
        cabinet.inner(),
        &household_id,
    )?;
    result.map_err(|error| error.to_string())
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
fn evaluate_document_with_default_intelligence_provider(
    conversations: State<'_, ConversationState>,
    intelligence: State<'_, IntelligenceState>,
    settings: State<'_, SettingsStore>,
    portable: State<'_, PortableState>,
    cabinet: State<'_, CabinetState>,
    sessions: State<'_, AccountSessionManager>,
    household_id: String,
    arrival_id: i64,
) -> Result<CloudAssistanceResolution, String> {
    let granted_by = current_household_actor(sessions.inner(), &household_id)?;
    let selected =
        load_default_intelligence_provider(settings.inner(), intelligence.inner(), &household_id)?
            .ok_or_else(|| "Choose a default Intelligence Provider in Options.".to_owned())?;
    let permission = intelligence
        .verified_consent_scopes(&household_id)
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|scope| {
            default_permission_matches(scope, &selected, DefaultIntelligencePermission::Document)
        })
        .ok_or_else(|| "Enable Document permission in Options.".to_owned())?;
    let result = DocumentIntelligenceService::new(
        conversations.inner().clone(),
        intelligence.inner().clone(),
    )
    .evaluate_document(
        &household_id,
        arrival_id,
        IntelligenceSelection {
            provider_id: selected.provider_id,
            model_id: selected.model_id,
        },
        CloudConsentDecision::UseExistingScope,
        &granted_by,
        Some(permission.id),
    );
    capture_portable_state(
        portable.inner(),
        conversations.inner(),
        intelligence.inner(),
        cabinet.inner(),
        &household_id,
    )?;
    result.map_err(|error| error.to_string())
}

#[tauri::command]
fn list_cloud_consent_scopes(
    store: State<'_, IntelligenceState>,
    household_id: String,
) -> Result<intelligence::CloudConsentScopeListing, String> {
    store
        .consent_scope_listing(&household_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn revoke_cloud_consent_scope(
    store: State<'_, IntelligenceState>,
    conversations: State<'_, ConversationState>,
    portable: State<'_, PortableState>,
    cabinet: State<'_, CabinetState>,
    household_id: String,
    scope_id: i64,
) -> Result<(), String> {
    store
        .revoke_scope(&household_id, scope_id)
        .map_err(|error| error.to_string())?;
    capture_portable_state(
        portable.inner(),
        conversations.inner(),
        store.inner(),
        cabinet.inner(),
        &household_id,
    )
}

#[tauri::command]
fn list_cloud_assistance_audit_events(
    store: State<'_, IntelligenceState>,
    household_id: String,
) -> Result<Vec<CloudAssistanceAuditEvent>, String> {
    store
        .list_audit_events(&household_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
fn record_cloud_candidate_disposition(
    store: State<'_, IntelligenceState>,
    conversations: State<'_, ConversationState>,
    portable: State<'_, PortableState>,
    cabinet: State<'_, CabinetState>,
    sessions: State<'_, AccountSessionManager>,
    household_id: String,
    arrival_id: i64,
    request_id: String,
    disposition: CandidateDisposition,
) -> Result<(), String> {
    current_household_actor(sessions.inner(), &household_id)?;
    if !matches!(
        disposition,
        CandidateDisposition::Accepted | CandidateDisposition::Corrected
    ) {
        return Err(
            "Only accepted or corrected candidate Evidence can be recorded here.".to_owned(),
        );
    }
    let event = store
        .list_audit_events(&household_id)
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|event| event.request_id == request_id)
        .ok_or_else(|| "The Cloud Assistance History event was not found.".to_owned())?;
    if event.document_arrival_id != format!("arrival-{arrival_id}")
        || event.outcome != CloudAssistanceOutcome::Completed
        || event.candidate_disposition != CandidateDisposition::Pending
    {
        return Err("The candidate Evidence is not pending for this Document Arrival.".to_owned());
    }
    let arrival = conversations
        .list_document_arrivals(&household_id)
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|arrival| arrival.id == arrival_id)
        .ok_or_else(|| "The Document Arrival was not found.".to_owned())?;
    if arrival.processing_state != DocumentProcessingState::ReadyToFile {
        return Err(
            "Member Direction must be recorded before candidate History changes.".to_owned(),
        );
    }
    store
        .record_candidate_disposition(&household_id, &request_id, disposition)
        .map_err(|error| error.to_string())?;
    capture_portable_state(
        portable.inner(),
        conversations.inner(),
        store.inner(),
        cabinet.inner(),
        &household_id,
    )
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
fn resolve_duplicate(
    store: State<'_, ConversationState>,
    intelligence: State<'_, IntelligenceState>,
    portable: State<'_, PortableState>,
    cabinet: State<'_, CabinetState>,
    household_id: String,
    arrival_id: i64,
    related_arrival_id: i64,
    decision: DuplicateDecision,
    remember_preference: bool,
) -> Result<DocumentArrival, String> {
    let arrival = store
        .resolve_duplicate(
            &household_id,
            arrival_id,
            related_arrival_id,
            decision,
            remember_preference,
        )
        .map_err(|error| error.to_string())?;
    capture_portable_state(
        portable.inner(),
        store.inner(),
        intelligence.inner(),
        cabinet.inner(),
        &household_id,
    )?;
    Ok(arrival)
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
fn get_document_conversation(
    store: State<'_, ConversationState>,
    cabinet: State<'_, CabinetState>,
    household_id: String,
    arrival_id: i64,
) -> Result<DocumentConversationView, String> {
    let configuration = cabinet
        .load(&household_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "A Cabinet must be configured before handling a document.".to_owned())?;
    let cabinet_section = configuration
        .sections
        .first()
        .ok_or_else(|| "The Cabinet has no filing sections.".to_owned())?;
    store
        .document_conversation_in_section(&household_id, arrival_id, cabinet_section)
        .map_err(|error| error.to_string())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SubmitMemberUtteranceRequest {
    household_id: String,
    arrival_id: i64,
    utterance: MemberUtterance,
    cloud_selection: Option<IntelligenceSelection>,
    existing_consent_grant_id: Option<i64>,
}

#[tauri::command]
fn submit_member_utterance(
    store: State<'_, ConversationState>,
    intelligence: State<'_, IntelligenceState>,
    portable: State<'_, PortableState>,
    cabinet: State<'_, CabinetState>,
    sessions: State<'_, AccountSessionManager>,
    request: SubmitMemberUtteranceRequest,
) -> Result<ConversationTurnOutcome, String> {
    let SubmitMemberUtteranceRequest {
        household_id,
        arrival_id,
        utterance,
        cloud_selection,
        existing_consent_grant_id,
    } = request;
    let configuration = cabinet
        .load(&household_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "A Cabinet must be configured before handling a document.".to_owned())?;
    let cabinet_section = configuration
        .sections
        .first()
        .ok_or_else(|| "The Cabinet has no filing sections.".to_owned())?;
    let mut outcome = store
        .submit_member_utterance(
            &household_id,
            arrival_id,
            utterance,
            &DeterministicMemberDirectionInterpreter,
            &configuration.root,
            cabinet_section,
        )
        .map_err(|error| error.to_string())?;
    let prepared_consent = match outcome.accepted_direction.as_ref() {
        Some(MemberDirectionCommand::UseCloudAssistance { consent })
            if outcome.status == ConversationTurnStatus::ActionPrepared =>
        {
            Some(*consent)
        }
        _ => None,
    };
    if let Some(consent) = prepared_consent {
        if consent == CloudConsentDecision::KeepLocal && cloud_selection.is_none() {
            current_household_actor(sessions.inner(), &household_id)?;
            let resolution = DocumentIntelligenceService::new(
                store.inner().clone(),
                intelligence.inner().clone(),
            )
            .keep_document_local(&household_id, arrival_id)
            .map_err(|error| error.to_string())?;
            outcome.status = ConversationTurnStatus::AcceptedDirection;
            outcome.message =
                "Kept local. No document information was sent to an Intelligence Provider."
                    .to_owned();
            outcome.cloud_result = resolution.result;
            outcome.arrival = store
                .list_document_arrivals(&household_id)
                .map_err(|error| error.to_string())?
                .into_iter()
                .find(|arrival| arrival.id == arrival_id)
                .ok_or_else(|| "The Document Handling is no longer available.".to_owned())?;
            outcome.next_prompt = store
                .document_conversation_in_section(&household_id, arrival_id, cabinet_section)
                .map_err(|error| error.to_string())?
                .prompt;
            capture_portable_state(
                portable.inner(),
                store.inner(),
                intelligence.inner(),
                cabinet.inner(),
                &household_id,
            )?;
            return Ok(outcome);
        }
        let Some(selection) = cloud_selection else {
            outcome.status = ConversationTurnStatus::ClarificationRequired;
            outcome.accepted_direction = None;
            outcome.message =
                "Choose the disclosed Intelligence Provider and model before allowing Cloud Assistance."
                    .to_owned();
            return Ok(outcome);
        };
        if consent == CloudConsentDecision::UseExistingScope && existing_consent_grant_id.is_none()
        {
            outcome.status = ConversationTurnStatus::ClarificationRequired;
            outcome.accepted_direction = None;
            outcome.message = "There is no matching reusable Consent Grant for the selected provider and model. Choose another consent option.".to_owned();
            return Ok(outcome);
        }
        let provider_name = selection.provider_id.clone();
        let model_name = selection.model_id.clone();
        let granted_by = current_household_actor(sessions.inner(), &household_id)?;
        let resolution =
            DocumentIntelligenceService::new(store.inner().clone(), intelligence.inner().clone())
                .evaluate_document(
                    &household_id,
                    arrival_id,
                    selection,
                    consent,
                    &granted_by,
                    existing_consent_grant_id,
                );
        let updated_arrival = store
            .list_document_arrivals(&household_id)
            .map_err(|error| error.to_string())?
            .into_iter()
            .find(|arrival| arrival.id == arrival_id)
            .ok_or_else(|| "The Document Handling is no longer available.".to_owned())?;
        let next = store
            .document_conversation_in_section(&household_id, arrival_id, cabinet_section)
            .map_err(|error| error.to_string())?;
        outcome.arrival = updated_arrival;
        outcome.next_prompt = next.prompt;
        match resolution {
            Ok(resolution) => {
                outcome.status = ConversationTurnStatus::AcceptedDirection;
                outcome.message = match resolution.result.as_ref() {
                    None => "Kept local. No document information was sent to an Intelligence Provider."
                        .to_owned(),
                    Some(result) if result.fields.is_empty() => format!(
                        "{provider_name} returned no usable suggestions. Luna kept this review ready for your direction."
                    ),
                    Some(result) => format!(
                        "{provider_name} {model_name} suggested {}. This is untrusted Evidence; review it before saving Household Context.",
                        result.fields.keys().cloned().collect::<Vec<_>>().join(", ")
                    ),
                };
                outcome.cloud_result = resolution.result;
            }
            Err(error) => {
                outcome.status = ConversationTurnStatus::ClarificationRequired;
                outcome.message = error.to_string();
            }
        }
    }
    capture_portable_state(
        portable.inner(),
        store.inner(),
        intelligence.inner(),
        cabinet.inner(),
        &household_id,
    )?;
    Ok(outcome)
}

#[tauri::command]
fn record_member_direction(
    store: State<'_, ConversationState>,
    intelligence: State<'_, IntelligenceState>,
    portable: State<'_, PortableState>,
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
    let arrival = store
        .record_member_direction(&household_id, arrival_id, direction, cabinet_section)
        .map_err(|error| error.to_string())?;
    capture_portable_state(
        portable.inner(),
        store.inner(),
        intelligence.inner(),
        cabinet.inner(),
        &household_id,
    )?;
    Ok(arrival)
}

#[tauri::command]
fn confirm_filing_decision(
    store: State<'_, ConversationState>,
    intelligence: State<'_, IntelligenceState>,
    portable: State<'_, PortableState>,
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
    let arrival = store
        .file_document(&household_id, arrival_id, configuration.root)
        .map_err(|error| error.to_string())?;
    capture_portable_state(
        portable.inner(),
        store.inner(),
        intelligence.inner(),
        cabinet.inner(),
        &household_id,
    )?;
    Ok(arrival)
}

#[cfg(feature = "e2e")]
fn e2e_digital_pdf() -> Vec<u8> {
    e2e_pdf_with_text(
        "Document Type: Electricity bill; Service Provider: AGL; Addressee: Sam Rivera; Property: 12 Seabreeze Avenue; Account: 12345678; Amount: $184.72; Relevant Date: 2026-07-15",
    )
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
    intelligence: State<'_, IntelligenceState>,
    portable: State<'_, PortableState>,
    cabinet: State<'_, CabinetState>,
    household_id: String,
    rule_id: i64,
    update: FilingRuleUpdate,
) -> Result<FilingRuleSummary, String> {
    let rule = store
        .update_filing_rule(&household_id, rule_id, update)
        .map_err(|error| error.to_string())?;
    capture_portable_state(
        portable.inner(),
        store.inner(),
        intelligence.inner(),
        cabinet.inner(),
        &household_id,
    )?;
    Ok(rule)
}

#[tauri::command]
fn pause_filing_rule(
    store: State<'_, ConversationState>,
    intelligence: State<'_, IntelligenceState>,
    portable: State<'_, PortableState>,
    cabinet: State<'_, CabinetState>,
    household_id: String,
    rule_id: i64,
    paused: bool,
) -> Result<FilingRuleSummary, String> {
    let rule = store
        .pause_filing_rule(&household_id, rule_id, paused)
        .map_err(|error| error.to_string())?;
    capture_portable_state(
        portable.inner(),
        store.inner(),
        intelligence.inner(),
        cabinet.inner(),
        &household_id,
    )?;
    Ok(rule)
}

#[tauri::command]
fn delete_filing_rule(
    store: State<'_, ConversationState>,
    intelligence: State<'_, IntelligenceState>,
    portable: State<'_, PortableState>,
    cabinet: State<'_, CabinetState>,
    household_id: String,
    rule_id: i64,
) -> Result<FilingRuleSummary, String> {
    let rule = store
        .delete_filing_rule(&household_id, rule_id)
        .map_err(|error| error.to_string())?;
    capture_portable_state(
        portable.inner(),
        store.inner(),
        intelligence.inner(),
        cabinet.inner(),
        &household_id,
    )?;
    Ok(rule)
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
        "cloud-scope" => "Unfamiliar cloud scope notice 2026-10-01",
        "cloud-reuse" => "Unfamiliar cloud reuse notice 2026-10-02",
        "cloud-once" => "Unfamiliar cloud once notice 2026-10-03",
        "cloud-local" => "Unfamiliar cloud local notice 2026-10-04",
        _ => return Err("Unknown E2E context document kind.".to_owned()),
    };
    let document = std::env::temp_dir().join(format!(
        "luna-e2e-context-{}-{kind}.pdf",
        std::process::id()
    ));
    std::fs::write(&document, e2e_pdf_with_text(text)).map_err(|error| error.to_string())?;
    Ok(document.to_string_lossy().into_owned())
}

#[cfg(feature = "e2e")]
#[tauri::command]
fn set_e2e_cabinet_availability(
    cabinet: State<'_, CabinetState>,
    household_id: String,
    available: bool,
) -> Result<(), String> {
    let configuration = cabinet
        .load(&household_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "The E2E Household has no configured Cabinet.".to_owned())?;
    let unavailable = configuration.root.with_extension("luna-e2e-unavailable");
    if available {
        if configuration.root.exists() {
            return Ok(());
        }
        std::fs::rename(&unavailable, &configuration.root).map_err(|error| error.to_string())
    } else {
        if unavailable.exists() {
            return Err("The E2E Cabinet is already unavailable.".to_owned());
        }
        std::fs::rename(&configuration.root, unavailable).map_err(|error| error.to_string())
    }
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
    #[cfg(all(debug_assertions, any(feature = "e2e", feature = "live-canary")))]
    let builder = builder
        .plugin(tauri_plugin_wdio::init())
        .plugin(tauri_plugin_wdio_webdriver::init());

    let builder = builder.setup(|app| {
        #[cfg(not(any(feature = "e2e", feature = "live-canary")))]
        let application_data = app.path().app_data_dir()?;
        #[cfg(all(feature = "live-canary", not(feature = "e2e")))]
        let application_data = std::env::var_os("LUNA_LIVE_CANARY_DATA_DIR")
            .map(std::path::PathBuf::from)
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "LUNA_LIVE_CANARY_DATA_DIR is required.",
                )
            })?;
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
            let household_vault_service = if cfg!(feature = "live-canary") {
                "app.luna.live-canary.household"
            } else {
                "app.luna.household"
            };
            let trusted_device = TrustedDeviceManager::new(OsCredentialVault::new(
                household_vault_service,
            ));
            app.manage(ConversationStore::open(&database, trusted_device.clone())?);
            app.manage(CloudIntelligenceStore::open(
                &database,
                trusted_device.clone(),
            )?);
            app.manage(PortableMemoryStore::open(
                &database,
                trusted_device.clone(),
            )?);
            app.manage(trusted_device);
        }
        #[cfg(not(feature = "e2e"))]
        {
            let account_vault_service = if cfg!(feature = "live-canary") {
                "app.luna.live-canary.account"
            } else {
                "app.luna.account"
            };
            app.manage(AccountSessionStore::new(OsCredentialVault::new(
                account_vault_service,
            )));
        }
        #[cfg(feature = "e2e")]
        {
            let trusted_device = TrustedDeviceManager::new(E2eCredentialVault::default());
            app.manage(ConversationStore::open(&database, trusted_device.clone())?);
            app.manage(CloudIntelligenceStore::open_with_gateway(
                &database,
                trusted_device.clone(),
                DeterministicIntelligenceGateway::new(
                    MANAGED_INTELLIGENCE_PROVIDER_ID,
                    MANAGED_INTELLIGENCE_MODEL_ID,
                    std::collections::BTreeMap::from([(
                        "amount".to_owned(),
                        "$184.72".to_owned(),
                    )]),
                )
                .with_conversation_reply(
                    "Start with the household task that is both urgent and blocks the most other work.",
                ),
                intelligence::provider_catalog(),
            )?);
            app.manage(PortableMemoryStore::open(
                &database,
                trusted_device.clone(),
            )?);
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
        list_intelligence_providers,
        list_intelligence_provider_statuses,
        get_default_intelligence_provider,
        set_default_intelligence_provider,
        clear_default_intelligence_provider,
        set_default_intelligence_permission,
        submit_ordinary_conversation_message,
        test_and_set_intelligence_provider_credential,
        clear_intelligence_provider_credential,
        set_managed_intelligence_gateway_credential,
        clear_managed_intelligence_gateway_credential,
        evaluate_document_with_cloud_assistance,
        evaluate_document_with_default_intelligence_provider,
        list_cloud_consent_scopes,
        revoke_cloud_consent_scope,
        list_cloud_assistance_audit_events,
        record_cloud_candidate_disposition,
        resolve_duplicate,
        list_filing_rules,
        update_filing_rule,
        pause_filing_rule,
        delete_filing_rule,
        list_filing_rule_audit_events,
        synchronize_portable_memory,
        list_portable_history_events,
        portable_authorization_cutoff,
        preview_filing_rule_reorganization,
        list_manual_move_candidates,
        record_manual_move_decision,
        dismiss_document_arrival,
        get_document_conversation,
        submit_member_utterance,
        record_member_direction,
        confirm_filing_decision,
        select_document_files,
        #[cfg(feature = "e2e")]
        select_e2e_context_document_file,
        #[cfg(feature = "e2e")]
        set_e2e_cabinet_availability,
        get_account_session_item,
        set_account_session_item,
        remove_account_session_item,
        sign_managed_intelligence_device_provisioning,
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
