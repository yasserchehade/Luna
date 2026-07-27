use std::{
    env,
    fs::{self, OpenOptions},
    io::{self, Write},
    panic::{catch_unwind, AssertUnwindSafe},
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
};

use image::ImageFormat;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::cabinet::ensure_incoming_folder;
use crate::trusted_device::{
    CredentialVault, ProtectedHouseholdState, TrustedDeviceError, TrustedDeviceManager,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Conversation {
    pub id: i64,
    pub household_id: String,
    pub title: String,
    pub archived: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DocumentProcessingState {
    NeedsMemberDirection,
    ReadyToFile,
    Filing,
    Filed,
    Dismissed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConfidenceState {
    Confirmed,
    LooksRight,
    NeedsChecking,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewEvidence {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewField {
    pub value: Option<String>,
    pub confidence_state: ConfidenceState,
}

impl Default for ReviewField {
    fn default() -> Self {
        Self {
            value: None,
            confidence_state: ConfidenceState::Unknown,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentContextReview {
    pub document_type: ReviewField,
    pub service_provider: ReviewField,
    pub service_provider_relevance: ReviewField,
    pub addressee: ReviewField,
    pub property: ReviewField,
    pub property_relevance: ReviewField,
    pub account: ReviewField,
    pub amount: ReviewField,
    pub relevant_dates: Vec<ReviewField>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClarificationQuestion {
    pub field: ContextField,
    pub prompt: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ContextField {
    DocumentType,
    ServiceProvider,
    ServiceProviderRelevance,
    Addressee,
    Property,
    PropertyRelevance,
    Account,
    Amount,
    RelevantDates,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilingDecisionReview {
    pub file_name: String,
    pub cabinet_destination: String,
    pub confirmed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilingDecisionDirection {
    pub file_name: String,
    pub cabinet_destination: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FiledOriginal {
    pub arrival_id: i64,
    pub conversation_id: i64,
    pub original_name: String,
    pub final_path: PathBuf,
    pub checksum: String,
    pub source_path: PathBuf,
    pub filing_decision: FilingDecisionReview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AuditEventKind {
    DocumentFiled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AuditAuthority {
    MemberDirection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditEvent {
    pub id: i64,
    pub household_id: String,
    pub kind: AuditEventKind,
    pub authority: AuditAuthority,
    pub subject: String,
    pub outcome: String,
    pub filed_original: FiledOriginal,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentContextDirection {
    pub document_type: Option<String>,
    #[serde(default)]
    pub document_type_resolved: bool,
    pub service_provider: Option<String>,
    #[serde(default)]
    pub service_provider_resolved: bool,
    pub addressee: Option<String>,
    #[serde(default)]
    pub addressee_resolved: bool,
    pub property: Option<String>,
    #[serde(default)]
    pub property_resolved: bool,
    pub account: Option<String>,
    #[serde(default)]
    pub account_resolved: bool,
    pub amount: Option<String>,
    #[serde(default)]
    pub amount_resolved: bool,
    pub relevant_dates: Vec<String>,
    #[serde(default)]
    pub relevant_dates_resolved: bool,
    pub service_provider_relevance: Option<ContextRelevanceDirection>,
    pub property_relevance: Option<ContextRelevanceDirection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextRelevanceDirection {
    pub subject: String,
    pub explanation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewCard {
    pub confidence_state: ConfidenceState,
    pub evidence: Vec<ReviewEvidence>,
    pub uncertainties: Vec<String>,
    pub proposed_cabinet_destination: Option<String>,
    pub context: DocumentContextReview,
    pub questions: Vec<ClarificationQuestion>,
    pub filing_decision: Option<FilingDecisionReview>,
}

pub trait LocalOcr: Send + Sync {
    fn extract_text(&self, original: &Path, media_type: &str) -> Option<String>;
}

#[derive(Default)]
pub struct TesseractOcr;

impl LocalOcr for TesseractOcr {
    fn extract_text(&self, original: &Path, media_type: &str) -> Option<String> {
        if media_type == "application/pdf" {
            let directory = tempfile::tempdir().ok()?;
            let image_base = directory.path().join("page");
            let output = Command::new(configured_local_executable(
                "LUNA_PDFTOPPM_COMMAND",
                "pdftoppm",
            ))
            .arg("-png")
            .arg(original)
            .arg(&image_base)
            .output()
            .ok()?;
            if !output.status.success() {
                return None;
            }
            let mut pages = fs::read_dir(directory.path())
                .ok()?
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.extension().is_some_and(|extension| extension == "png"))
                .collect::<Vec<_>>();
            pages.sort();
            let text = pages
                .iter()
                .filter_map(|page| self.extract_image_text(page))
                .collect::<Vec<_>>()
                .join("\n\n");
            return (!text.is_empty()).then_some(text);
        }
        self.extract_image_text(original)
    }
}

impl TesseractOcr {
    fn extract_image_text(&self, image: &Path) -> Option<String> {
        let output = Command::new(tesseract_executable())
            .arg(image)
            .arg("stdout")
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let text = String::from_utf8(output.stdout).ok()?;
        let text = text.trim().to_owned();
        (!text.is_empty()).then_some(text)
    }
}

fn configured_local_executable(environment_variable: &str, default: &str) -> PathBuf {
    if let Some(command) = env::var_os(environment_variable) {
        return command.into();
    }
    default.into()
}

fn tesseract_executable() -> PathBuf {
    if let Some(command) = env::var_os("LUNA_TESSERACT_COMMAND") {
        return command.into();
    }
    #[cfg(target_os = "windows")]
    {
        if let Some(program_files) = env::var_os("ProgramFiles") {
            let command = PathBuf::from(program_files)
                .join("Tesseract-OCR")
                .join("tesseract.exe");
            if command.is_file() {
                return command;
            }
        }
    }
    "tesseract".into()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentArrival {
    pub id: i64,
    pub household_id: String,
    pub conversation_id: i64,
    pub original_name: String,
    pub original_path: PathBuf,
    pub source_path: PathBuf,
    pub checksum: String,
    pub media_type: String,
    pub extracted_text: Option<String>,
    pub review_card: ReviewCard,
    pub processing_state: DocumentProcessingState,
    pub filed_original: Option<FiledOriginal>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoItem {
    pub arrival_id: i64,
    pub conversation_id: i64,
    pub conversation_title: String,
    pub conversation_deleted: bool,
    pub document_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationMessage {
    pub id: i64,
    pub conversation_id: i64,
    pub author: String,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConversationPayload {
    title: String,
    archived: bool,
    deleted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MessagePayload {
    author: String,
    body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DocumentArrivalPayload {
    original_name: String,
    #[serde(default)]
    original_path: PathBuf,
    source_path: PathBuf,
    #[serde(default)]
    checksum: String,
    media_type: String,
    extracted_text: Option<String>,
    processing_state: DocumentProcessingState,
    #[serde(default)]
    context_direction: DocumentContextDirection,
    #[serde(default)]
    filing_decision: Option<FilingDecisionReview>,
    #[serde(default)]
    filed_original: Option<FiledOriginal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AuditEventPayload {
    kind: AuditEventKind,
    authority: AuditAuthority,
    subject: String,
    outcome: String,
    filed_original: FiledOriginal,
}

impl DocumentArrivalPayload {
    fn restore_legacy_original_path(&mut self) {
        if self.original_path.as_os_str().is_empty() {
            self.original_path = self.source_path.clone();
        }
    }
}

#[derive(Debug, Error)]
pub enum ConversationError {
    #[error("The Conversation title cannot be empty.")]
    EmptyTitle,
    #[error("The message cannot be empty.")]
    EmptyMessage,
    #[error("The Conversation no longer exists.")]
    NotFound,
    #[error("Only PDF, JPG, and PNG documents can be attached.")]
    UnsupportedDocument,
    #[error("The selected document does not match its declared file type.")]
    InvalidDocument,
    #[error("A different Original already occupies this document's preserved location.")]
    OriginalConflict,
    #[error("A different Original already occupies the Cabinet Destination.")]
    CabinetDestinationConflict,
    #[error("The staged or filed Original could not be verified.")]
    OriginalVerificationFailed,
    #[error("Household Context must be resolved before confirming a Filing Decision.")]
    UnresolvedContext,
    #[error("The Cabinet Destination must be a safe relative path ending in the chosen filename.")]
    InvalidCabinetDestination,
    #[error("The selected document is unavailable.")]
    DocumentUnavailable(#[from] io::Error),
    #[error("Protected Household state is unavailable.")]
    TrustedDevice(#[from] TrustedDeviceError),
    #[error("Protected Household state is invalid.")]
    ProtectedState(#[from] serde_json::Error),
    #[error("Conversation storage is unavailable.")]
    Storage(#[from] rusqlite::Error),
}

#[derive(Clone)]
pub struct ConversationStore<V: CredentialVault> {
    database: PathBuf,
    trusted_device: TrustedDeviceManager<V>,
    local_ocr: Arc<dyn LocalOcr>,
}

impl<V: CredentialVault> ConversationStore<V> {
    pub fn open(
        database: impl AsRef<Path>,
        trusted_device: TrustedDeviceManager<V>,
    ) -> Result<Self, ConversationError> {
        Self::open_with_ocr(database, trusted_device, TesseractOcr)
    }

    pub fn open_with_ocr(
        database: impl AsRef<Path>,
        trusted_device: TrustedDeviceManager<V>,
        local_ocr: impl LocalOcr + 'static,
    ) -> Result<Self, ConversationError> {
        let store = Self {
            database: database.as_ref().to_owned(),
            trusted_device,
            local_ocr: Arc::new(local_ocr),
        };
        store.connect()?.execute_batch(
            "CREATE TABLE IF NOT EXISTS conversations (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                household_id TEXT NOT NULL,
                protected_payload TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS conversations_household
                ON conversations(household_id);
            CREATE TABLE IF NOT EXISTS conversation_messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                conversation_id INTEGER NOT NULL,
                protected_payload TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS conversation_messages_conversation
                ON conversation_messages(conversation_id, id);
            CREATE TABLE IF NOT EXISTS document_arrivals (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                household_id TEXT NOT NULL,
                conversation_id INTEGER NOT NULL,
                protected_payload TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS document_arrivals_household
                ON document_arrivals(household_id);
            CREATE TABLE IF NOT EXISTS audit_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                household_id TEXT NOT NULL,
                arrival_id INTEGER NOT NULL,
                protected_payload TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS audit_events_household
                ON audit_events(household_id, id);
            CREATE UNIQUE INDEX IF NOT EXISTS audit_events_arrival
                ON audit_events(arrival_id);",
        )?;
        Ok(store)
    }

    pub fn create_conversation(
        &self,
        household_id: &str,
        title: &str,
    ) -> Result<Conversation, ConversationError> {
        let title = title.trim();
        if title.is_empty() {
            return Err(ConversationError::EmptyTitle);
        }
        let payload = ConversationPayload {
            title: title.to_owned(),
            archived: false,
            deleted: false,
        };
        let protected = self.protect(household_id, &payload)?;
        let connection = self.connect()?;
        connection.execute(
            "INSERT INTO conversations (household_id, protected_payload) VALUES (?1, ?2)",
            params![household_id, protected],
        )?;
        Ok(Conversation {
            id: connection.last_insert_rowid(),
            household_id: household_id.to_owned(),
            title: title.to_owned(),
            archived: false,
        })
    }

    pub fn list_conversations(
        &self,
        household_id: &str,
        search: Option<&str>,
        include_archived: bool,
    ) -> Result<Vec<Conversation>, ConversationError> {
        let search = search.unwrap_or_default().trim().to_lowercase();
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT id, protected_payload FROM conversations
              WHERE household_id = ?1 ORDER BY id DESC",
        )?;
        let protected_rows = statement
            .query_map(params![household_id], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let mut conversations = Vec::new();
        for (id, protected) in protected_rows {
            let payload: ConversationPayload = self.open_protected(household_id, &protected)?;
            if payload.deleted || (!include_archived && payload.archived) {
                continue;
            }
            if !payload.title.to_lowercase().contains(&search) {
                continue;
            }
            conversations.push(Conversation {
                id,
                household_id: household_id.to_owned(),
                title: payload.title,
                archived: payload.archived,
            });
        }
        Ok(conversations)
    }

    pub fn rename_conversation(
        &self,
        household_id: &str,
        conversation_id: i64,
        title: &str,
    ) -> Result<(), ConversationError> {
        let title = title.trim();
        if title.is_empty() {
            return Err(ConversationError::EmptyTitle);
        }
        let mut payload = self.load_conversation_payload(household_id, conversation_id)?;
        if payload.deleted {
            return Err(ConversationError::NotFound);
        }
        payload.title = title.to_owned();
        self.save_conversation_payload(household_id, conversation_id, &payload)
    }

    pub fn archive_conversation(
        &self,
        household_id: &str,
        conversation_id: i64,
        archived: bool,
    ) -> Result<(), ConversationError> {
        let mut payload = self.load_conversation_payload(household_id, conversation_id)?;
        if payload.deleted {
            return Err(ConversationError::NotFound);
        }
        payload.archived = archived;
        self.save_conversation_payload(household_id, conversation_id, &payload)
    }

    pub fn delete_conversation(
        &self,
        household_id: &str,
        conversation_id: i64,
    ) -> Result<(), ConversationError> {
        let mut payload = self.load_conversation_payload(household_id, conversation_id)?;
        if payload.deleted {
            return Err(ConversationError::NotFound);
        }
        payload.title = "Deleted Conversation".to_owned();
        payload.archived = false;
        payload.deleted = true;
        let protected = self.protect(household_id, &payload)?;
        let mut connection = self.connect()?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "UPDATE conversations SET protected_payload = ?1
              WHERE id = ?2 AND household_id = ?3",
            params![protected, conversation_id, household_id],
        )?;
        transaction.execute(
            "DELETE FROM conversation_messages WHERE conversation_id = ?1",
            params![conversation_id],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn attach_document(
        &self,
        household_id: &str,
        conversation_id: i64,
        path: impl AsRef<Path>,
        cabinet_root: impl AsRef<Path>,
    ) -> Result<DocumentArrival, ConversationError> {
        self.require_active_conversation(household_id, conversation_id)?;
        let path = path.as_ref();
        if !path.is_file() {
            return Err(ConversationError::DocumentUnavailable(io::Error::new(
                io::ErrorKind::NotFound,
                "document is not a file",
            )));
        }
        let declared_media_type = match path
            .extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("pdf") => "application/pdf",
            Some("jpg" | "jpeg") => "image/jpeg",
            Some("png") => "image/png",
            _ => return Err(ConversationError::UnsupportedDocument),
        };
        let original = fs::read(path)?;
        let media_type = detected_media_type(&original)?;
        if media_type != declared_media_type {
            return Err(ConversationError::InvalidDocument);
        }
        let original_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or(ConversationError::UnsupportedDocument)?;
        let extracted_pdf_text = extract_digital_pdf_text(media_type, &original)?;
        let checksum = sha256(&original);
        let original_path =
            self.preserve_original(cabinet_root.as_ref(), &checksum, original_name, &original)?;
        let extracted_text = extract_local_text(
            media_type,
            &original_path,
            extracted_pdf_text,
            &*self.local_ocr,
        );
        let context_direction = local_context_direction(extracted_text.as_deref());
        let payload = DocumentArrivalPayload {
            original_name: original_name.to_owned(),
            original_path,
            source_path: path.to_owned(),
            checksum,
            media_type: media_type.to_owned(),
            extracted_text,
            processing_state: DocumentProcessingState::NeedsMemberDirection,
            context_direction,
            filing_decision: None,
            filed_original: None,
        };
        let protected = self.protect(household_id, &payload)?;
        let connection = self.connect()?;
        connection.execute(
            "INSERT INTO document_arrivals (household_id, conversation_id, protected_payload)
             VALUES (?1, ?2, ?3)",
            params![household_id, conversation_id, protected],
        )?;
        self.document_arrival(
            household_id,
            connection.last_insert_rowid(),
            conversation_id,
            payload,
        )
    }

    pub fn add_member_message(
        &self,
        household_id: &str,
        conversation_id: i64,
        body: &str,
    ) -> Result<ConversationMessage, ConversationError> {
        self.require_active_conversation(household_id, conversation_id)?;
        let body = body.trim();
        if body.is_empty() {
            return Err(ConversationError::EmptyMessage);
        }
        let payload = MessagePayload {
            author: "member".to_owned(),
            body: body.to_owned(),
        };
        let protected = self.protect(household_id, &payload)?;
        let connection = self.connect()?;
        connection.execute(
            "INSERT INTO conversation_messages (conversation_id, protected_payload)
             VALUES (?1, ?2)",
            params![conversation_id, protected],
        )?;
        Ok(ConversationMessage {
            id: connection.last_insert_rowid(),
            conversation_id,
            author: payload.author,
            body: payload.body,
        })
    }

    pub fn list_messages(
        &self,
        household_id: &str,
        conversation_id: i64,
    ) -> Result<Vec<ConversationMessage>, ConversationError> {
        let conversation = self.load_conversation_payload(household_id, conversation_id)?;
        if conversation.deleted {
            return Ok(Vec::new());
        }
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT id, protected_payload FROM conversation_messages
              WHERE conversation_id = ?1 ORDER BY id",
        )?;
        let protected_rows = statement
            .query_map(params![conversation_id], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        protected_rows
            .into_iter()
            .map(|(id, protected)| {
                let payload: MessagePayload = self.open_protected(household_id, &protected)?;
                Ok(ConversationMessage {
                    id,
                    conversation_id,
                    author: payload.author,
                    body: payload.body,
                })
            })
            .collect()
    }

    pub fn list_document_arrivals(
        &self,
        household_id: &str,
    ) -> Result<Vec<DocumentArrival>, ConversationError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT id, conversation_id, protected_payload
               FROM document_arrivals WHERE household_id = ?1 ORDER BY id DESC",
        )?;
        let protected_rows = statement
            .query_map(params![household_id], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        protected_rows
            .into_iter()
            .map(|(id, conversation_id, protected)| {
                let mut payload: DocumentArrivalPayload =
                    self.open_protected(household_id, &protected)?;
                payload.restore_legacy_original_path();
                self.document_arrival(household_id, id, conversation_id, payload)
            })
            .collect()
    }

    pub fn list_todo_items(&self, household_id: &str) -> Result<Vec<TodoItem>, ConversationError> {
        let arrivals = self.list_document_arrivals(household_id)?;
        arrivals
            .into_iter()
            .filter(|arrival| {
                arrival.processing_state == DocumentProcessingState::NeedsMemberDirection
            })
            .map(|arrival| {
                let conversation =
                    self.load_conversation_payload(household_id, arrival.conversation_id)?;
                Ok(TodoItem {
                    arrival_id: arrival.id,
                    conversation_id: arrival.conversation_id,
                    conversation_title: conversation.title,
                    conversation_deleted: conversation.deleted,
                    document_name: arrival.original_name,
                })
            })
            .collect()
    }

    pub fn list_filed_originals(
        &self,
        household_id: &str,
    ) -> Result<Vec<FiledOriginal>, ConversationError> {
        Ok(self
            .list_document_arrivals(household_id)?
            .into_iter()
            .filter_map(|arrival| arrival.filed_original)
            .collect())
    }

    pub fn list_audit_events(
        &self,
        household_id: &str,
    ) -> Result<Vec<AuditEvent>, ConversationError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT id, protected_payload FROM audit_events
              WHERE household_id = ?1 ORDER BY id DESC",
        )?;
        let protected_rows = statement
            .query_map(params![household_id], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        protected_rows
            .into_iter()
            .map(|(id, protected)| {
                let payload: AuditEventPayload = self.open_protected(household_id, &protected)?;
                Ok(AuditEvent {
                    id,
                    household_id: household_id.to_owned(),
                    kind: payload.kind,
                    authority: payload.authority,
                    subject: payload.subject,
                    outcome: payload.outcome,
                    filed_original: payload.filed_original,
                })
            })
            .collect()
    }

    pub fn dismiss_document_arrival(
        &self,
        household_id: &str,
        arrival_id: i64,
    ) -> Result<(), ConversationError> {
        let connection = self.connect()?;
        let protected: Option<String> = connection
            .query_row(
                "SELECT protected_payload FROM document_arrivals
                  WHERE id = ?1 AND household_id = ?2",
                params![arrival_id, household_id],
                |row| row.get(0),
            )
            .optional()?;
        let mut payload: DocumentArrivalPayload =
            self.open_protected(household_id, &protected.ok_or(ConversationError::NotFound)?)?;
        payload.restore_legacy_original_path();
        if payload.processing_state != DocumentProcessingState::NeedsMemberDirection {
            return Err(ConversationError::NotFound);
        }
        payload.processing_state = DocumentProcessingState::Dismissed;
        let protected = self.protect(household_id, &payload)?;
        self.ensure_updated(
            "UPDATE document_arrivals SET protected_payload = ?1
              WHERE id = ?2 AND household_id = ?3",
            params![protected, arrival_id, household_id],
        )
    }

    pub fn record_member_direction(
        &self,
        household_id: &str,
        arrival_id: i64,
        direction: DocumentContextDirection,
        cabinet_section: &str,
    ) -> Result<DocumentArrival, ConversationError> {
        let (conversation_id, mut payload) =
            self.load_document_arrival_payload(household_id, arrival_id)?;
        if payload.processing_state != DocumentProcessingState::NeedsMemberDirection {
            return Err(ConversationError::NotFound);
        }
        payload.context_direction = direction.normalized();
        payload.filing_decision = clarification_questions(&payload.context_direction)
            .is_empty()
            .then(|| propose_filing_decision(&payload, cabinet_section));
        self.save_document_arrival_payload(household_id, arrival_id, conversation_id, payload)
    }

    pub fn confirm_filing_decision(
        &self,
        household_id: &str,
        arrival_id: i64,
        direction: FilingDecisionDirection,
    ) -> Result<DocumentArrival, ConversationError> {
        let (conversation_id, mut payload) =
            self.load_document_arrival_payload(household_id, arrival_id)?;
        if payload.processing_state != DocumentProcessingState::NeedsMemberDirection {
            return Err(ConversationError::NotFound);
        }
        if !clarification_questions(&payload.context_direction).is_empty()
            || payload.filing_decision.is_none()
        {
            return Err(ConversationError::UnresolvedContext);
        }
        let file_name = direction.file_name.trim();
        let cabinet_destination = direction.cabinet_destination.trim();
        if !valid_file_name(file_name, &payload.original_name)
            || !valid_cabinet_destination(cabinet_destination, file_name)
        {
            return Err(ConversationError::InvalidCabinetDestination);
        }
        payload.filing_decision = Some(FilingDecisionReview {
            file_name: file_name.to_owned(),
            cabinet_destination: cabinet_destination.to_owned(),
            confirmed: true,
        });
        payload.processing_state = DocumentProcessingState::ReadyToFile;
        self.save_document_arrival_payload(household_id, arrival_id, conversation_id, payload)
    }

    pub fn file_document(
        &self,
        household_id: &str,
        arrival_id: i64,
        cabinet_root: impl AsRef<Path>,
    ) -> Result<DocumentArrival, ConversationError> {
        let (conversation_id, mut payload) =
            self.load_document_arrival_payload(household_id, arrival_id)?;
        if payload.processing_state == DocumentProcessingState::Filed {
            let filed_original = payload
                .filed_original
                .as_ref()
                .ok_or(ConversationError::OriginalVerificationFailed)?;
            verify_existing_destination(&filed_original.final_path, &filed_original.checksum)?;
            if payload.original_path.is_file() {
                fs::remove_file(&payload.original_path)?;
                remove_empty_staging_directory(&payload.original_path);
            }
            return self.document_arrival(household_id, arrival_id, conversation_id, payload);
        }
        let resuming = payload.processing_state == DocumentProcessingState::Filing;
        if !matches!(
            payload.processing_state,
            DocumentProcessingState::ReadyToFile | DocumentProcessingState::Filing
        ) {
            return Err(ConversationError::NotFound);
        }
        let decision = payload
            .filing_decision
            .clone()
            .filter(|decision| decision.confirmed)
            .ok_or(ConversationError::UnresolvedContext)?;
        let cabinet_root = cabinet_root.as_ref();
        let destination = safe_cabinet_destination(cabinet_root, &decision.cabinet_destination)?;
        let staged = fs::read(&payload.original_path)?;
        if sha256(&staged) != payload.checksum {
            return Err(ConversationError::OriginalVerificationFailed);
        }

        if destination.exists() && !resuming {
            return Err(ConversationError::CabinetDestinationConflict);
        }
        if destination.exists() {
            verify_existing_destination(&destination, &payload.checksum)?;
        } else {
            if !resuming {
                payload.processing_state = DocumentProcessingState::Filing;
                self.save_document_arrival_payload(
                    household_id,
                    arrival_id,
                    conversation_id,
                    payload.clone(),
                )?;
            }
            let temporary = destination.with_file_name(format!(
                ".luna-filing-{arrival_id}-{}.tmp",
                payload.checksum
            ));
            if temporary.exists() && sha256(&fs::read(&temporary)?) != payload.checksum {
                fs::remove_file(&temporary)?;
            }
            if !temporary.exists() {
                let mut file = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&temporary)?;
                file.write_all(&staged)?;
                file.sync_all()?;
            }
            if sha256(&fs::read(&temporary)?) != payload.checksum {
                return Err(ConversationError::OriginalVerificationFailed);
            }
            match fs::hard_link(&temporary, &destination) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    return Err(ConversationError::CabinetDestinationConflict);
                }
                Err(error) => return Err(ConversationError::DocumentUnavailable(error)),
            }
            fs::remove_file(&temporary)?;
        }
        verify_existing_destination(&destination, &payload.checksum)?;

        let filed_original = FiledOriginal {
            arrival_id,
            conversation_id,
            original_name: payload.original_name.clone(),
            final_path: destination,
            checksum: payload.checksum.clone(),
            source_path: payload.source_path.clone(),
            filing_decision: decision.clone(),
        };
        payload.processing_state = DocumentProcessingState::Filed;
        payload.filed_original = Some(filed_original.clone());
        let protected_arrival = self.protect(household_id, &payload)?;
        let protected_event = self.protect(
            household_id,
            &AuditEventPayload {
                kind: AuditEventKind::DocumentFiled,
                authority: AuditAuthority::MemberDirection,
                subject: payload.original_name.clone(),
                outcome: format!("Filed and verified at {}", decision.cabinet_destination),
                filed_original,
            },
        )?;
        let mut connection = self.connect()?;
        let transaction = connection.transaction()?;
        let updated = transaction.execute(
            "UPDATE document_arrivals SET protected_payload = ?1
              WHERE id = ?2 AND household_id = ?3",
            params![protected_arrival, arrival_id, household_id],
        )?;
        if updated == 0 {
            return Err(ConversationError::NotFound);
        }
        transaction.execute(
            "INSERT INTO audit_events (household_id, arrival_id, protected_payload)
             VALUES (?1, ?2, ?3)",
            params![household_id, arrival_id, protected_event],
        )?;
        transaction.commit()?;

        fs::remove_file(&payload.original_path)?;
        remove_empty_staging_directory(&payload.original_path);
        self.document_arrival(household_id, arrival_id, conversation_id, payload)
    }

    pub fn resume_document_filings(
        &self,
        household_id: &str,
        cabinet_root: impl AsRef<Path>,
    ) -> Result<(), ConversationError> {
        let resumable = self
            .list_document_arrivals(household_id)?
            .into_iter()
            .filter(|arrival| {
                matches!(
                    arrival.processing_state,
                    DocumentProcessingState::ReadyToFile
                        | DocumentProcessingState::Filing
                        | DocumentProcessingState::Filed
                )
            })
            .map(|arrival| arrival.id)
            .collect::<Vec<_>>();
        for arrival_id in resumable {
            self.file_document(household_id, arrival_id, cabinet_root.as_ref())?;
        }
        Ok(())
    }

    fn load_document_arrival_payload(
        &self,
        household_id: &str,
        arrival_id: i64,
    ) -> Result<(i64, DocumentArrivalPayload), ConversationError> {
        let stored: Option<(i64, String)> = self
            .connect()?
            .query_row(
                "SELECT conversation_id, protected_payload FROM document_arrivals
                  WHERE id = ?1 AND household_id = ?2",
                params![arrival_id, household_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let (conversation_id, protected) = stored.ok_or(ConversationError::NotFound)?;
        let mut payload: DocumentArrivalPayload = self.open_protected(household_id, &protected)?;
        payload.restore_legacy_original_path();
        Ok((conversation_id, payload))
    }

    fn save_document_arrival_payload(
        &self,
        household_id: &str,
        arrival_id: i64,
        conversation_id: i64,
        payload: DocumentArrivalPayload,
    ) -> Result<DocumentArrival, ConversationError> {
        let protected = self.protect(household_id, &payload)?;
        self.ensure_updated(
            "UPDATE document_arrivals SET protected_payload = ?1
              WHERE id = ?2 AND household_id = ?3",
            params![protected, arrival_id, household_id],
        )?;
        self.document_arrival(household_id, arrival_id, conversation_id, payload)
    }

    fn document_arrival(
        &self,
        household_id: &str,
        arrival_id: i64,
        conversation_id: i64,
        payload: DocumentArrivalPayload,
    ) -> Result<DocumentArrival, ConversationError> {
        let review_card = review_card(&payload);
        Ok(DocumentArrival {
            id: arrival_id,
            household_id: household_id.to_owned(),
            conversation_id,
            original_name: payload.original_name,
            original_path: payload.original_path,
            source_path: payload.source_path,
            checksum: payload.checksum,
            media_type: payload.media_type,
            extracted_text: payload.extracted_text,
            review_card,
            processing_state: payload.processing_state,
            filed_original: payload.filed_original,
        })
    }

    fn require_active_conversation(
        &self,
        household_id: &str,
        conversation_id: i64,
    ) -> Result<ConversationPayload, ConversationError> {
        let payload = self.load_conversation_payload(household_id, conversation_id)?;
        if payload.deleted {
            return Err(ConversationError::NotFound);
        }
        Ok(payload)
    }

    fn load_conversation_payload(
        &self,
        household_id: &str,
        conversation_id: i64,
    ) -> Result<ConversationPayload, ConversationError> {
        let protected: Option<String> = self
            .connect()?
            .query_row(
                "SELECT protected_payload FROM conversations
                  WHERE id = ?1 AND household_id = ?2",
                params![conversation_id, household_id],
                |row| row.get(0),
            )
            .optional()?;
        self.open_protected(household_id, &protected.ok_or(ConversationError::NotFound)?)
    }

    fn save_conversation_payload(
        &self,
        household_id: &str,
        conversation_id: i64,
        payload: &ConversationPayload,
    ) -> Result<(), ConversationError> {
        let protected = self.protect(household_id, payload)?;
        self.ensure_updated(
            "UPDATE conversations SET protected_payload = ?1
              WHERE id = ?2 AND household_id = ?3",
            params![protected, conversation_id, household_id],
        )
    }

    fn protect<T: Serialize>(
        &self,
        household_id: &str,
        value: &T,
    ) -> Result<String, ConversationError> {
        let plaintext = serde_json::to_vec(value)?;
        let protected = self
            .trusted_device
            .protect_household_state(household_id, &plaintext)?;
        Ok(serde_json::to_string(&protected)?)
    }

    fn open_protected<T: DeserializeOwned>(
        &self,
        household_id: &str,
        protected: &str,
    ) -> Result<T, ConversationError> {
        let protected: ProtectedHouseholdState = serde_json::from_str(protected)?;
        let plaintext = self
            .trusted_device
            .open_household_state(household_id, &protected)?;
        Ok(serde_json::from_slice(&plaintext)?)
    }

    fn ensure_updated<P>(&self, sql: &str, parameters: P) -> Result<(), ConversationError>
    where
        P: rusqlite::Params,
    {
        if self.connect()?.execute(sql, parameters)? == 0 {
            return Err(ConversationError::NotFound);
        }
        Ok(())
    }

    fn connect(&self) -> rusqlite::Result<Connection> {
        Connection::open(&self.database)
    }

    fn preserve_original(
        &self,
        cabinet_root: &Path,
        checksum: &str,
        original_name: &str,
        original: &[u8],
    ) -> Result<PathBuf, ConversationError> {
        if !cabinet_root.is_dir() {
            return Err(ConversationError::DocumentUnavailable(io::Error::new(
                io::ErrorKind::NotFound,
                "Cabinet is unavailable",
            )));
        }
        let incoming = ensure_incoming_folder(cabinet_root).map_err(io::Error::other)?;
        let directory = incoming.join(checksum);
        fs::create_dir_all(&directory)?;
        let original_path = directory.join(original_name);

        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&original_path)
        {
            Ok(mut file) => {
                file.write_all(original)?;
                file.sync_all()?;
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                if fs::read(&original_path)? != original {
                    return Err(ConversationError::OriginalConflict);
                }
            }
            Err(error) => return Err(ConversationError::DocumentUnavailable(error)),
        }

        Ok(original_path)
    }
}

fn safe_cabinet_destination(
    cabinet_root: &Path,
    cabinet_destination: &str,
) -> Result<PathBuf, ConversationError> {
    let file_name = cabinet_destination
        .rsplit('/')
        .next()
        .ok_or(ConversationError::InvalidCabinetDestination)?;
    if !valid_cabinet_destination(cabinet_destination, file_name) || !cabinet_root.is_dir() {
        return Err(ConversationError::InvalidCabinetDestination);
    }
    let canonical_root = cabinet_root
        .canonicalize()
        .map_err(ConversationError::DocumentUnavailable)?;
    let components = cabinet_destination.split('/').collect::<Vec<_>>();
    let mut parent = canonical_root.clone();
    for component in &components[..components.len() - 1] {
        let candidate = parent.join(component);
        if !candidate.exists() {
            fs::create_dir(&candidate)?;
        }
        let canonical_candidate = candidate.canonicalize()?;
        if !canonical_candidate.is_dir() || !canonical_candidate.starts_with(&canonical_root) {
            return Err(ConversationError::InvalidCabinetDestination);
        }
        parent = canonical_candidate;
    }
    let destination = parent.join(file_name);
    if destination
        .parent()
        .is_none_or(|candidate| !candidate.starts_with(&canonical_root))
    {
        return Err(ConversationError::InvalidCabinetDestination);
    }
    Ok(destination)
}

fn verify_existing_destination(
    destination: &Path,
    expected_checksum: &str,
) -> Result<(), ConversationError> {
    if !destination.is_file() || sha256(&fs::read(destination)?) != expected_checksum {
        return Err(ConversationError::CabinetDestinationConflict);
    }
    Ok(())
}

fn remove_empty_staging_directory(original_path: &Path) {
    if let Some(directory) = original_path.parent() {
        let _ = fs::remove_dir(directory);
    }
}

fn detected_media_type(original: &[u8]) -> Result<&'static str, ConversationError> {
    if original.starts_with(b"%PDF-") {
        Ok("application/pdf")
    } else if original.starts_with(&[0xFF, 0xD8, 0xFF]) {
        image::load_from_memory_with_format(original, ImageFormat::Jpeg)
            .map_err(|_| ConversationError::InvalidDocument)?;
        Ok("image/jpeg")
    } else if original.starts_with(b"\x89PNG\r\n\x1a\n") {
        image::load_from_memory_with_format(original, ImageFormat::Png)
            .map_err(|_| ConversationError::InvalidDocument)?;
        Ok("image/png")
    } else {
        Err(ConversationError::InvalidDocument)
    }
}

fn sha256(original: &[u8]) -> String {
    format!("{:x}", Sha256::digest(original))
}

fn extract_digital_pdf_text(
    media_type: &str,
    original: &[u8],
) -> Result<Option<String>, ConversationError> {
    if media_type != "application/pdf" {
        return Ok(None);
    }
    match catch_unwind(AssertUnwindSafe(|| {
        pdf_extract::extract_text_from_mem(original)
    })) {
        Ok(result) => result
            .map(|text| {
                let text = text.trim().to_owned();
                (!text.is_empty()).then_some(text)
            })
            .map_err(|_| ConversationError::InvalidDocument),
        Err(_) => Ok(None),
    }
}

fn extract_local_text(
    media_type: &str,
    original_path: &Path,
    extracted_pdf_text: Option<String>,
    local_ocr: &dyn LocalOcr,
) -> Option<String> {
    match media_type {
        "application/pdf" => {
            extracted_pdf_text.or_else(|| local_ocr.extract_text(original_path, media_type))
        }
        "image/jpeg" | "image/png" => local_ocr.extract_text(original_path, media_type),
        _ => None,
    }
}

fn local_context_direction(extracted_text: Option<&str>) -> DocumentContextDirection {
    let mut direction = DocumentContextDirection::default();
    let Some(extracted_text) = extracted_text else {
        return direction;
    };
    for segment in extracted_text.split(['\n', ';']) {
        let Some((label, value)) = segment.split_once(':') else {
            continue;
        };
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        match label.trim().to_ascii_lowercase().as_str() {
            "document type" => direction.document_type = Some(value.to_owned()),
            "service provider" | "provider" => {
                direction.service_provider = Some(value.to_owned());
            }
            "addressee" => direction.addressee = Some(value.to_owned()),
            "property" | "property address" | "address" => {
                direction.property = Some(value.to_owned());
            }
            "account" | "account number" => direction.account = Some(value.to_owned()),
            "amount" => direction.amount = Some(value.to_owned()),
            "relevant date" | "date" => direction.relevant_dates.push(value.to_owned()),
            _ => {}
        }
    }
    direction
}

fn review_card(payload: &DocumentArrivalPayload) -> ReviewCard {
    let mut evidence = vec![
        ReviewEvidence {
            label: "Original name".to_owned(),
            value: payload.original_name.clone(),
        },
        ReviewEvidence {
            label: "Detected type".to_owned(),
            value: media_type_label(&payload.media_type).to_owned(),
        },
        ReviewEvidence {
            label: "SHA-256".to_owned(),
            value: if payload.checksum.is_empty() {
                "Not recorded before Original staging".to_owned()
            } else {
                payload.checksum.clone()
            },
        },
    ];
    if let Some(text) = &payload.extracted_text {
        evidence.push(ReviewEvidence {
            label: "Extracted text".to_owned(),
            value: text.clone(),
        });
    } else {
        evidence.push(ReviewEvidence {
            label: "Local inspection".to_owned(),
            value: "No text could be read locally.".to_owned(),
        });
    }
    let context = &payload.context_direction;
    let questions = clarification_questions(context);
    ReviewCard {
        confidence_state: if questions.is_empty() {
            ConfidenceState::Confirmed
        } else if payload.extracted_text.is_some() {
            ConfidenceState::NeedsChecking
        } else {
            ConfidenceState::Unknown
        },
        evidence,
        uncertainties: if questions.is_empty() {
            Vec::new()
        } else {
            vec!["Luna needs your direction before filing this Original.".to_owned()]
        },
        proposed_cabinet_destination: payload
            .filing_decision
            .as_ref()
            .map(|decision| decision.cabinet_destination.clone()),
        context: DocumentContextReview {
            document_type: review_field(&context.document_type, context.document_type_resolved),
            service_provider: review_field(
                &context.service_provider,
                context.service_provider_resolved,
            ),
            service_provider_relevance: review_field(
                &context
                    .service_provider_relevance
                    .as_ref()
                    .map(|relevance| relevance.explanation.clone()),
                context.service_provider_relevance.is_some(),
            ),
            addressee: review_field(&context.addressee, context.addressee_resolved),
            property: review_field(&context.property, context.property_resolved),
            property_relevance: review_field(
                &context
                    .property_relevance
                    .as_ref()
                    .map(|relevance| relevance.explanation.clone()),
                context.property_relevance.is_some(),
            ),
            account: review_field(&context.account, context.account_resolved),
            amount: review_field(&context.amount, context.amount_resolved),
            relevant_dates: context
                .relevant_dates
                .iter()
                .map(|date| review_field(&Some(date.clone()), context.relevant_dates_resolved))
                .collect(),
        },
        questions,
        filing_decision: payload.filing_decision.clone(),
    }
}

fn clarification_questions(context: &DocumentContextDirection) -> Vec<ClarificationQuestion> {
    let mut questions = Vec::new();
    if !context.document_type_resolved || context.document_type.is_none() {
        questions.push(ClarificationQuestion {
            field: ContextField::DocumentType,
            prompt: "What kind of document is this?".to_owned(),
        });
    }
    if !context.service_provider_resolved || context.service_provider.is_none() {
        questions.push(ClarificationQuestion {
            field: ContextField::ServiceProvider,
            prompt: "Which Service Provider issued this document?".to_owned(),
        });
    } else if context.service_provider_relevance.is_none() {
        questions.push(ClarificationQuestion {
            field: ContextField::ServiceProviderRelevance,
            prompt: format!(
                "How is {} relevant to this Household?",
                context.service_provider.as_deref().unwrap_or_default()
            ),
        });
    }
    if !context.addressee_resolved || context.addressee.is_none() {
        questions.push(ClarificationQuestion {
            field: ContextField::Addressee,
            prompt: "Who is this document addressed to?".to_owned(),
        });
    }
    if !context.property_resolved {
        questions.push(ClarificationQuestion {
            field: ContextField::Property,
            prompt: "Does this document relate to a Household property or address?".to_owned(),
        });
    } else if context.property.is_some() && context.property_relevance.is_none() {
        questions.push(ClarificationQuestion {
            field: ContextField::PropertyRelevance,
            prompt: format!(
                "How is {} relevant to this Household?",
                context.property.as_deref().unwrap_or_default()
            ),
        });
    }
    if !context.account_resolved {
        questions.push(ClarificationQuestion {
            field: ContextField::Account,
            prompt: "Does this document relate to a Household account?".to_owned(),
        });
    }
    if !context.amount_resolved {
        questions.push(ClarificationQuestion {
            field: ContextField::Amount,
            prompt: "What amount, if any, helps identify this document?".to_owned(),
        });
    }
    if !context.relevant_dates_resolved {
        questions.push(ClarificationQuestion {
            field: ContextField::RelevantDates,
            prompt: "Which dates, if any, should identify this document?".to_owned(),
        });
    }
    questions
}

fn propose_filing_decision(
    payload: &DocumentArrivalPayload,
    cabinet_section: &str,
) -> FilingDecisionReview {
    let context = &payload.context_direction;
    let date = context
        .relevant_dates
        .first()
        .map(String::as_str)
        .unwrap_or("Undated");
    let year = date
        .get(..4)
        .filter(|year| year.chars().all(|character| character.is_ascii_digit()))
        .unwrap_or("Undated");
    let service_provider = context
        .service_provider
        .as_deref()
        .unwrap_or("Unknown provider");
    let document_type = context.document_type.as_deref().unwrap_or("Document");
    let addressee = context.addressee.as_deref().unwrap_or("Household");
    let extension = Path::new(&payload.original_name)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_else(|| "pdf".to_owned());
    let file_name = format!(
        "{} - {} - {} - {}.{}",
        safe_generated_component(date),
        safe_generated_component(service_provider),
        safe_generated_component(document_type),
        safe_generated_component(addressee),
        extension
    );
    let subject = context
        .property
        .as_deref()
        .or(context.account.as_deref())
        .unwrap_or(addressee);
    let cabinet_destination = [
        safe_component(cabinet_section),
        safe_generated_component(subject),
        safe_generated_component(service_provider),
        safe_generated_component(year),
        file_name.clone(),
    ]
    .join("/");
    FilingDecisionReview {
        file_name,
        cabinet_destination,
        confirmed: false,
    }
}

fn safe_component(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|character| {
            if character.is_control() || "<>:\"/\\|?*".contains(character) {
                '-'
            } else {
                character
            }
        })
        .collect::<String>();
    let sanitized = sanitized.trim().trim_matches('.').trim();
    let sanitized = if sanitized.is_empty() {
        "Unknown"
    } else {
        sanitized
    };
    if reserved_windows_name(sanitized) {
        match sanitized.rsplit_once('.') {
            Some((stem, extension)) => format!("{stem}_.{extension}"),
            None => format!("{sanitized}_"),
        }
    } else {
        sanitized.to_owned()
    }
}

fn safe_generated_component(value: &str) -> String {
    let safe = safe_component(value);
    if safe.len() <= 48 {
        return safe;
    }
    let mut shortened = String::new();
    for character in safe.chars() {
        if shortened.len() + character.len_utf8() > 48 {
            break;
        }
        shortened.push(character);
    }
    safe_component(&shortened)
}

fn valid_file_name(file_name: &str, original_name: &str) -> bool {
    if file_name.is_empty()
        || file_name == "."
        || file_name == ".."
        || !valid_path_component(file_name)
    {
        return false;
    }
    let extension = Path::new(file_name)
        .extension()
        .and_then(|extension| extension.to_str());
    let original_extension = Path::new(original_name)
        .extension()
        .and_then(|extension| extension.to_str());
    extension
        .zip(original_extension)
        .is_some_and(|(left, right)| left.eq_ignore_ascii_case(right))
}

fn valid_cabinet_destination(cabinet_destination: &str, file_name: &str) -> bool {
    if cabinet_destination.is_empty()
        || cabinet_destination.starts_with(['/', '\\'])
        || cabinet_destination.contains('\\')
    {
        return false;
    }
    let components = cabinet_destination.split('/').collect::<Vec<_>>();
    components.len() >= 2
        && !components[0].eq_ignore_ascii_case("Incoming")
        && components
            .last()
            .is_some_and(|component| *component == file_name)
        && components.iter().all(|component| {
            !component.is_empty()
                && *component != "."
                && *component != ".."
                && valid_path_component(component)
        })
}

fn valid_path_component(component: &str) -> bool {
    component.len() <= 240
        && safe_component(component) == component
        && !reserved_windows_name(component)
}

fn reserved_windows_name(component: &str) -> bool {
    let stem = component.split('.').next().unwrap_or_default();
    matches!(
        stem.to_ascii_uppercase().as_str(),
        "CON"
            | "CONIN$"
            | "CONOUT$"
            | "CLOCK$"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "COM¹"
            | "COM²"
            | "COM³"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
            | "LPT¹"
            | "LPT²"
            | "LPT³"
    )
}

impl DocumentContextDirection {
    fn normalized(self) -> Self {
        let document_type = non_empty(self.document_type);
        let service_provider = non_empty(self.service_provider);
        let addressee = non_empty(self.addressee);
        let property = non_empty(self.property);
        let account = non_empty(self.account);
        let amount = non_empty(self.amount);
        Self {
            document_type,
            document_type_resolved: self.document_type_resolved,
            service_provider_relevance: normalized_relevance(
                self.service_provider_relevance,
                service_provider.as_deref(),
            ),
            service_provider,
            service_provider_resolved: self.service_provider_resolved,
            addressee,
            addressee_resolved: self.addressee_resolved,
            property_relevance: normalized_relevance(self.property_relevance, property.as_deref()),
            property,
            property_resolved: self.property_resolved,
            account,
            account_resolved: self.account_resolved,
            amount,
            amount_resolved: self.amount_resolved,
            relevant_dates: self
                .relevant_dates
                .into_iter()
                .filter_map(|date| non_empty(Some(date)))
                .collect(),
            relevant_dates_resolved: self.relevant_dates_resolved,
        }
    }
}

fn normalized_relevance(
    relevance: Option<ContextRelevanceDirection>,
    current_subject: Option<&str>,
) -> Option<ContextRelevanceDirection> {
    let relevance = relevance?;
    let subject = non_empty(Some(relevance.subject))?;
    let explanation = non_empty(Some(relevance.explanation))?;
    (current_subject == Some(subject.as_str())).then_some(ContextRelevanceDirection {
        subject,
        explanation,
    })
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim().to_owned();
        (!value.is_empty()).then_some(value)
    })
}

fn review_field(value: &Option<String>, resolved: bool) -> ReviewField {
    ReviewField {
        value: value.clone(),
        confidence_state: if resolved {
            ConfidenceState::Confirmed
        } else if value.is_some() {
            ConfidenceState::LooksRight
        } else {
            ConfidenceState::Unknown
        },
    }
}

fn media_type_label(media_type: &str) -> &str {
    match media_type {
        "application/pdf" => "PDF",
        "image/jpeg" => "JPG",
        "image/png" => "PNG",
        _ => "Unknown",
    }
}
