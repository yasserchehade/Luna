use std::{
    env,
    fs::{self, OpenOptions},
    io::{self, Write},
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
pub struct ReviewCard {
    pub confidence_state: ConfidenceState,
    pub evidence: Vec<ReviewEvidence>,
    pub uncertainties: Vec<String>,
    pub proposed_cabinet_destination: Option<String>,
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
                ON document_arrivals(household_id);",
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
        let payload = DocumentArrivalPayload {
            original_name: original_name.to_owned(),
            original_path,
            source_path: path.to_owned(),
            checksum,
            media_type: media_type.to_owned(),
            extracted_text,
            processing_state: DocumentProcessingState::NeedsMemberDirection,
        };
        let protected = self.protect(household_id, &payload)?;
        let connection = self.connect()?;
        connection.execute(
            "INSERT INTO document_arrivals (household_id, conversation_id, protected_payload)
             VALUES (?1, ?2, ?3)",
            params![household_id, conversation_id, protected],
        )?;
        let review_card = review_card(&payload);
        Ok(DocumentArrival {
            id: connection.last_insert_rowid(),
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
        })
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
                let review_card = review_card(&payload);
                Ok(DocumentArrival {
                    id,
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
                })
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
    pdf_extract::extract_text_from_mem(original)
        .map(|text| {
            let text = text.trim().to_owned();
            (!text.is_empty()).then_some(text)
        })
        .map_err(|_| ConversationError::InvalidDocument)
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
    ReviewCard {
        confidence_state: if payload.extracted_text.is_some() {
            ConfidenceState::NeedsChecking
        } else {
            ConfidenceState::Unknown
        },
        evidence,
        uncertainties: vec!["Luna needs your direction before filing this Original.".to_owned()],
        proposed_cabinet_destination: None,
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
