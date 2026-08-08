use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use luna_core::{
    ConversationPort, ConversationPortError, HandleHouseholdAdministrationTurn,
    HandleHouseholdWorkCommand, HouseholdAdministrationClock, HouseholdAdministrationEngine,
    HouseholdAdministrationFailure, HouseholdAdministrationMessage, HouseholdAdministrationOutcome,
    HouseholdAdministrationReasoning, HouseholdAdministrationSource, HouseholdWork,
    HouseholdWorkPort, HouseholdWorkPortError, SourcePort, SourcePortError, MAX_MVP_DOCUMENT_BYTES,
};
use rusqlite::{params, Connection, OptionalExtension};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use uuid::Uuid;

const MIGRATION_0001: &str = include_str!("../migrations/0001_web_household_administration.sql");

#[derive(Clone)]
pub struct WebStore {
    database_path: PathBuf,
    source_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct StoredSource {
    pub id: String,
    pub display_name: String,
    pub media_type: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct SourceMetadata {
    pub display_name: String,
    pub media_type: String,
    pub size_bytes: u64,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct PersistedConversationMessage {
    pub id: String,
    pub role: String,
    pub body: String,
    pub created_at: String,
    pub contextual_work_ids: Vec<String>,
}

pub struct ExecutedTurn {
    pub outcome: HouseholdAdministrationOutcome,
    pub member_message: PersistedConversationMessage,
    pub luna_message: PersistedConversationMessage,
    pub affected_work_ids: Vec<String>,
}

#[derive(Debug)]
pub enum ExecuteTurnError {
    Engine(HouseholdAdministrationFailure),
    Persistence,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct WebClock;

impl HouseholdAdministrationClock for WebClock {
    fn now(&self) -> String {
        now()
    }
}

impl WebStore {
    pub fn open(data_dir: &Path) -> rusqlite::Result<Self> {
        fs::create_dir_all(data_dir)
            .map_err(|_| rusqlite::Error::InvalidPath(data_dir.to_path_buf()))?;
        let database_path = data_dir.join("luna-web.sqlite3");
        let source_dir = data_dir.join("sources");
        fs::create_dir_all(&source_dir)
            .map_err(|_| rusqlite::Error::InvalidPath(source_dir.clone()))?;
        let store = Self {
            database_path,
            source_dir,
        };
        store.connection()?.execute_batch(MIGRATION_0001)?;
        Ok(store)
    }

    fn connection(&self) -> rusqlite::Result<Connection> {
        let connection = Connection::open(&self.database_path)?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        Ok(connection)
    }

    pub fn source_count(&self, household_id: &str) -> rusqlite::Result<u32> {
        self.connection()?.query_row(
            "SELECT COUNT(*) FROM sources WHERE household_id = ?1",
            [household_id],
            |row| row.get(0),
        )
    }

    pub fn store_source(
        &self,
        household_id: &str,
        display_name: &str,
        media_type: &str,
        bytes: &[u8],
        created_at: &str,
    ) -> rusqlite::Result<StoredSource> {
        let id = format!("source-{}", Uuid::new_v4());
        let storage_key = format!("{id}.bin");
        let path = self.source_dir.join(&storage_key);
        fs::write(&path, bytes).map_err(|_| rusqlite::Error::InvalidPath(path.clone()))?;
        let result = self.connection()?.execute(
            "INSERT INTO sources (id, household_id, display_name, media_type, size_bytes, storage_key, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![id, household_id, display_name, media_type, bytes.len() as u64, storage_key, created_at],
        );
        if let Err(error) = result {
            let _ = fs::remove_file(path);
            return Err(error);
        }
        Ok(StoredSource {
            id,
            display_name: display_name.to_owned(),
            media_type: media_type.to_owned(),
            size_bytes: bytes.len() as u64,
        })
    }

    pub fn list_conversation(
        &self,
        household_id: &str,
        conversation_id: i64,
    ) -> rusqlite::Result<Vec<PersistedConversationMessage>> {
        let connection = self.connection()?;
        query_conversation(&connection, household_id, conversation_id, None)
    }

    pub fn list_work(&self, household_id: &str) -> rusqlite::Result<Vec<HouseholdWork>> {
        let connection = self.connection()?;
        query_work(&connection, household_id)
    }

    pub fn find_work(
        &self,
        household_id: &str,
        work_id: &str,
    ) -> rusqlite::Result<Option<HouseholdWork>> {
        self.connection()?
            .query_row(
                "SELECT payload FROM household_work WHERE household_id = ?1 AND id = ?2",
                params![household_id, work_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .map(|payload| serde_json::from_str(&payload).map_err(json_error))
            .transpose()
    }

    pub fn source_metadata(
        &self,
        household_id: &str,
        source_id: &str,
    ) -> rusqlite::Result<Option<SourceMetadata>> {
        self.connection()?
            .query_row(
                "SELECT display_name, media_type, size_bytes, created_at
                 FROM sources WHERE household_id = ?1 AND id = ?2",
                params![household_id, source_id],
                |row| {
                    Ok(SourceMetadata {
                        display_name: row.get(0)?,
                        media_type: row.get(1)?,
                        size_bytes: row.get(2)?,
                        created_at: row.get(3)?,
                    })
                },
            )
            .optional()
    }

    pub fn execute_turn(
        &self,
        reasoning: &dyn HouseholdAdministrationReasoning,
        input: HandleHouseholdAdministrationTurn,
        contextual_work_ids: Vec<String>,
    ) -> Result<ExecutedTurn, ExecuteTurnError> {
        let connection = self
            .connection()
            .map_err(|_| ExecuteTurnError::Persistence)?;
        connection
            .execute_batch("BEGIN IMMEDIATE")
            .map_err(|_| ExecuteTurnError::Persistence)?;
        let adapter = SqliteTurnAdapter {
            connection: Mutex::new(connection),
            source_dir: self.source_dir.clone(),
            contextual_work_ids,
            appended_messages: Mutex::new(Vec::new()),
            affected_work_ids: Mutex::new(Vec::new()),
        };
        let clock = WebClock;
        let result = {
            let engine =
                HouseholdAdministrationEngine::new(&adapter, &adapter, &adapter, reasoning, &clock);
            engine.handle_turn(input)
        };
        match result {
            Ok(outcome) => {
                adapter
                    .connection
                    .lock()
                    .map_err(|_| ExecuteTurnError::Persistence)?
                    .execute_batch("COMMIT")
                    .map_err(|_| ExecuteTurnError::Persistence)?;
                let messages = adapter
                    .appended_messages
                    .into_inner()
                    .map_err(|_| ExecuteTurnError::Persistence)?;
                let member_message = messages
                    .iter()
                    .find(|message| message.role == "member")
                    .cloned()
                    .ok_or(ExecuteTurnError::Persistence)?;
                let luna_message = messages
                    .iter()
                    .rev()
                    .find(|message| message.role == "luna")
                    .cloned()
                    .ok_or(ExecuteTurnError::Persistence)?;
                let affected_work_ids = adapter
                    .affected_work_ids
                    .into_inner()
                    .map_err(|_| ExecuteTurnError::Persistence)?;
                Ok(ExecutedTurn {
                    outcome,
                    member_message,
                    luna_message,
                    affected_work_ids,
                })
            }
            Err(error) => {
                if let Ok(connection) = adapter.connection.lock() {
                    let _ = connection.execute_batch("ROLLBACK");
                }
                Err(ExecuteTurnError::Engine(error))
            }
        }
    }

    pub fn execute_command(
        &self,
        reasoning: &dyn HouseholdAdministrationReasoning,
        input: HandleHouseholdWorkCommand,
    ) -> Result<ExecutedTurn, ExecuteTurnError> {
        let contextual_work_ids = vec![input.work_id.clone()];
        let connection = self
            .connection()
            .map_err(|_| ExecuteTurnError::Persistence)?;
        connection
            .execute_batch("BEGIN IMMEDIATE")
            .map_err(|_| ExecuteTurnError::Persistence)?;
        let adapter = SqliteTurnAdapter {
            connection: Mutex::new(connection),
            source_dir: self.source_dir.clone(),
            contextual_work_ids,
            appended_messages: Mutex::new(Vec::new()),
            affected_work_ids: Mutex::new(Vec::new()),
        };
        let clock = WebClock;
        let result = {
            let engine =
                HouseholdAdministrationEngine::new(&adapter, &adapter, &adapter, reasoning, &clock);
            engine.handle_work_command(input)
        };
        match result {
            Ok(outcome) => {
                adapter
                    .connection
                    .lock()
                    .map_err(|_| ExecuteTurnError::Persistence)?
                    .execute_batch("COMMIT")
                    .map_err(|_| ExecuteTurnError::Persistence)?;
                let messages = adapter
                    .appended_messages
                    .into_inner()
                    .map_err(|_| ExecuteTurnError::Persistence)?;
                let member_message = messages
                    .iter()
                    .find(|message| message.role == "member")
                    .cloned()
                    .ok_or(ExecuteTurnError::Persistence)?;
                let luna_message = messages
                    .iter()
                    .rev()
                    .find(|message| message.role == "luna")
                    .cloned()
                    .ok_or(ExecuteTurnError::Persistence)?;
                let affected_work_ids = adapter
                    .affected_work_ids
                    .into_inner()
                    .map_err(|_| ExecuteTurnError::Persistence)?;
                Ok(ExecutedTurn {
                    outcome,
                    member_message,
                    luna_message,
                    affected_work_ids,
                })
            }
            Err(error) => {
                if let Ok(connection) = adapter.connection.lock() {
                    let _ = connection.execute_batch("ROLLBACK");
                }
                Err(ExecuteTurnError::Engine(error))
            }
        }
    }
}

struct SqliteTurnAdapter {
    connection: Mutex<Connection>,
    source_dir: PathBuf,
    contextual_work_ids: Vec<String>,
    appended_messages: Mutex<Vec<PersistedConversationMessage>>,
    affected_work_ids: Mutex<Vec<String>>,
}

impl ConversationPort for SqliteTurnAdapter {
    fn recent_messages(
        &self,
        household_id: &str,
        conversation_id: i64,
        limit: usize,
    ) -> Result<Vec<HouseholdAdministrationMessage>, ConversationPortError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| ConversationPortError::Unavailable)?;
        query_conversation(&connection, household_id, conversation_id, Some(limit))
            .map(|messages| {
                messages
                    .into_iter()
                    .map(|message| HouseholdAdministrationMessage {
                        author: message.role,
                        body: message.body,
                    })
                    .collect()
            })
            .map_err(|_| ConversationPortError::Unavailable)
    }

    fn append_member_message(
        &self,
        household_id: &str,
        conversation_id: i64,
        body: &str,
    ) -> Result<(), ConversationPortError> {
        self.append_message(
            household_id,
            conversation_id,
            "member",
            body,
            self.contextual_work_ids.clone(),
            None,
        )
    }

    fn append_luna_message(
        &self,
        household_id: &str,
        conversation_id: i64,
        body: &str,
        source_reference: Option<&str>,
    ) -> Result<(), ConversationPortError> {
        let affected = self
            .affected_work_ids
            .lock()
            .map_err(|_| ConversationPortError::Unavailable)?
            .clone();
        self.append_message(
            household_id,
            conversation_id,
            "luna",
            body,
            if affected.is_empty() {
                self.contextual_work_ids.clone()
            } else {
                affected
            },
            source_reference,
        )
    }
}

impl SqliteTurnAdapter {
    fn append_message(
        &self,
        household_id: &str,
        conversation_id: i64,
        role: &str,
        body: &str,
        contextual_work_ids: Vec<String>,
        source_reference: Option<&str>,
    ) -> Result<(), ConversationPortError> {
        let message = PersistedConversationMessage {
            id: format!("message-{}", Uuid::new_v4()),
            role: role.to_owned(),
            body: body.to_owned(),
            created_at: now(),
            contextual_work_ids,
        };
        let contextual = serde_json::to_string(&message.contextual_work_ids)
            .map_err(|_| ConversationPortError::Unavailable)?;
        self.connection
            .lock()
            .map_err(|_| ConversationPortError::Unavailable)?
            .execute(
                "INSERT INTO conversation_messages
                 (id, household_id, conversation_id, role, body, created_at, contextual_work_ids, source_reference)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![message.id, household_id, conversation_id, role, body, message.created_at, contextual, source_reference],
            )
            .map_err(|_| ConversationPortError::Unavailable)?;
        self.appended_messages
            .lock()
            .map_err(|_| ConversationPortError::Unavailable)?
            .push(message);
        Ok(())
    }
}

impl HouseholdWorkPort for SqliteTurnAdapter {
    fn list(&self, household_id: &str) -> Result<Vec<HouseholdWork>, HouseholdWorkPortError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| HouseholdWorkPortError::Unavailable)?;
        query_work(&connection, household_id).map_err(|_| HouseholdWorkPortError::Unavailable)
    }

    fn save(&self, household_id: &str, work: &HouseholdWork) -> Result<(), HouseholdWorkPortError> {
        let payload =
            serde_json::to_string(work).map_err(|_| HouseholdWorkPortError::Unavailable)?;
        let connection = self
            .connection
            .lock()
            .map_err(|_| HouseholdWorkPortError::Unavailable)?;
        connection
            .execute(
                "INSERT INTO household_work (id, household_id, payload, updated_at)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(id) DO UPDATE SET payload = excluded.payload, updated_at = excluded.updated_at
                 WHERE household_work.household_id = excluded.household_id",
                params![work.id, household_id, payload, work.updated_at],
            )
            .map_err(|_| HouseholdWorkPortError::Unavailable)?;
        for (sequence, event) in work.audit_events.iter().enumerate() {
            connection
                .execute(
                    "INSERT OR IGNORE INTO audit_events
                     (id, household_id, work_id, sequence, body, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        format!("{}-audit-{sequence}", work.id),
                        household_id,
                        work.id,
                        sequence as i64,
                        event,
                        work.updated_at
                    ],
                )
                .map_err(|_| HouseholdWorkPortError::Unavailable)?;
        }
        drop(connection);
        let mut affected = self
            .affected_work_ids
            .lock()
            .map_err(|_| HouseholdWorkPortError::Unavailable)?;
        if !affected.contains(&work.id) {
            affected.push(work.id.clone());
        }
        Ok(())
    }
}

impl SourcePort for SqliteTurnAdapter {
    fn load(
        &self,
        household_id: &str,
        source_reference: &str,
    ) -> Result<HouseholdAdministrationSource, SourcePortError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| SourcePortError::Unavailable)?;
        let metadata = connection
            .query_row(
                "SELECT display_name, media_type, size_bytes, storage_key
                 FROM sources WHERE household_id = ?1 AND id = ?2",
                params![household_id, source_reference],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, u64>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            )
            .optional()
            .map_err(|_| SourcePortError::Unavailable)?
            .ok_or(SourcePortError::NotFound)?;
        drop(connection);
        if metadata.2 > MAX_MVP_DOCUMENT_BYTES {
            return Err(SourcePortError::TooLarge);
        }
        if !matches!(
            metadata.1.as_str(),
            "application/pdf" | "image/jpeg" | "image/png"
        ) {
            return Err(SourcePortError::Unsupported);
        }
        let bytes = fs::read(self.source_dir.join(&metadata.3))
            .map_err(|_| SourcePortError::Unavailable)?;
        if bytes.len() as u64 > MAX_MVP_DOCUMENT_BYTES {
            return Err(SourcePortError::TooLarge);
        }
        Ok(HouseholdAdministrationSource {
            reference: source_reference.to_owned(),
            filename: metadata.0,
            media_type: metadata.1,
            original_base64: BASE64.encode(bytes),
            extracted_text: None,
            original_size_bytes: metadata.2,
            extracted_text_truncated: false,
        })
    }
}

fn query_conversation(
    connection: &Connection,
    household_id: &str,
    conversation_id: i64,
    limit: Option<usize>,
) -> rusqlite::Result<Vec<PersistedConversationMessage>> {
    let sql = if limit.is_some() {
        "SELECT id, role, body, created_at, contextual_work_ids FROM (
            SELECT id, role, body, created_at, contextual_work_ids
            FROM conversation_messages
            WHERE household_id = ?1 AND conversation_id = ?2
            ORDER BY created_at DESC, rowid DESC LIMIT ?3
         ) ORDER BY created_at ASC"
    } else {
        "SELECT id, role, body, created_at, contextual_work_ids
         FROM conversation_messages
         WHERE household_id = ?1 AND conversation_id = ?2
         ORDER BY created_at ASC, rowid ASC"
    };
    let mut statement = connection.prepare(sql)?;
    if let Some(limit) = limit {
        statement
            .query_map(
                params![household_id, conversation_id, limit as i64],
                message_row,
            )?
            .collect()
    } else {
        statement
            .query_map(params![household_id, conversation_id], message_row)?
            .collect()
    }
}

fn message_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<PersistedConversationMessage> {
    let contextual: String = row.get(4)?;
    Ok(PersistedConversationMessage {
        id: row.get(0)?,
        role: row.get(1)?,
        body: row.get(2)?,
        created_at: row.get(3)?,
        contextual_work_ids: serde_json::from_str(&contextual).map_err(json_error)?,
    })
}

fn query_work(connection: &Connection, household_id: &str) -> rusqlite::Result<Vec<HouseholdWork>> {
    let mut statement = connection.prepare(
        "SELECT payload FROM household_work WHERE household_id = ?1 ORDER BY updated_at DESC, id ASC",
    )?;
    let work = statement
        .query_map([household_id], |row| {
            let payload: String = row.get(0)?;
            serde_json::from_str(&payload).map_err(json_error)
        })?
        .collect();
    work
}

fn json_error(error: serde_json::Error) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
}

fn now() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| OffsetDateTime::now_utc().unix_timestamp_nanos().to_string())
}
