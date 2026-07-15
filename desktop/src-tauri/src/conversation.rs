use std::{
    io,
    path::{Path, PathBuf},
};

use rusqlite::{params, Connection, OptionalExtension};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use thiserror::Error;

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentArrival {
    pub id: i64,
    pub household_id: String,
    pub conversation_id: i64,
    pub original_name: String,
    pub source_path: PathBuf,
    pub media_type: String,
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
    source_path: PathBuf,
    media_type: String,
    processing_state: DocumentProcessingState,
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
}

impl<V: CredentialVault> ConversationStore<V> {
    pub fn open(
        database: impl AsRef<Path>,
        trusted_device: TrustedDeviceManager<V>,
    ) -> Result<Self, ConversationError> {
        let store = Self {
            database: database.as_ref().to_owned(),
            trusted_device,
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
    ) -> Result<DocumentArrival, ConversationError> {
        self.require_active_conversation(household_id, conversation_id)?;
        let path = path.as_ref();
        if !path.is_file() {
            return Err(ConversationError::DocumentUnavailable(io::Error::new(
                io::ErrorKind::NotFound,
                "document is not a file",
            )));
        }
        let media_type = match path
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
        let original_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or(ConversationError::UnsupportedDocument)?;
        let payload = DocumentArrivalPayload {
            original_name: original_name.to_owned(),
            source_path: path.to_owned(),
            media_type: media_type.to_owned(),
            processing_state: DocumentProcessingState::NeedsMemberDirection,
        };
        let protected = self.protect(household_id, &payload)?;
        let connection = self.connect()?;
        connection.execute(
            "INSERT INTO document_arrivals (household_id, conversation_id, protected_payload)
             VALUES (?1, ?2, ?3)",
            params![household_id, conversation_id, protected],
        )?;
        Ok(DocumentArrival {
            id: connection.last_insert_rowid(),
            household_id: household_id.to_owned(),
            conversation_id,
            original_name: payload.original_name,
            source_path: payload.source_path,
            media_type: payload.media_type,
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
                let payload: DocumentArrivalPayload =
                    self.open_protected(household_id, &protected)?;
                Ok(DocumentArrival {
                    id,
                    household_id: household_id.to_owned(),
                    conversation_id,
                    original_name: payload.original_name,
                    source_path: payload.source_path,
                    media_type: payload.media_type,
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
}
