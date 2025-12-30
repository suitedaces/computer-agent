// storage module for conversation persistence using SQLite
// stores conversations in Anthropic API-compatible format for seamless replay

use crate::api::{ContentBlock, Message};
use rusqlite::{params, Connection, Result as SqlResult};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;

/// usage stats from anthropic API response
/// see: https://docs.claude.com/en/api/messages
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    /// tokens written to cache (prompt caching)
    #[serde(default)]
    pub cache_creation_input_tokens: u32,
    /// tokens read from cache (prompt caching)
    #[serde(default)]
    pub cache_read_input_tokens: u32,
}

impl Usage {
    pub fn total_input(&self) -> u32 {
        self.input_tokens + self.cache_creation_input_tokens + self.cache_read_input_tokens
    }

    pub fn total(&self) -> u32 {
        self.total_input() + self.output_tokens
    }
}

/// per-turn usage tracking - one entry per API call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnUsage {
    pub turn_index: u32,
    pub usage: Usage,
    pub model: String,
    pub timestamp: i64,
}

/// conversation metadata for listing without loading full messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationMeta {
    pub id: String,
    pub title: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub model: String,
    pub mode: String,
    pub message_count: u32,
    pub total_input_tokens: u32,
    pub total_output_tokens: u32,
}

/// full conversation with messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: String,
    pub title: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub model: String,
    pub mode: String,
    /// messages in Anthropic API format - can be sent directly to API
    pub messages: Vec<Message>,
    /// per-turn usage for detailed cost tracking
    pub turn_usage: Vec<TurnUsage>,
    /// aggregated usage
    pub total_input_tokens: u32,
    pub total_output_tokens: u32,
}

impl Conversation {
    pub fn new(id: String, title: String, model: String, mode: String) -> Self {
        let now = timestamp();
        Self {
            id,
            title,
            created_at: now,
            updated_at: now,
            model,
            mode,
            messages: Vec::new(),
            turn_usage: Vec::new(),
            total_input_tokens: 0,
            total_output_tokens: 0,
        }
    }

    pub fn to_meta(&self) -> ConversationMeta {
        ConversationMeta {
            id: self.id.clone(),
            title: self.title.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
            model: self.model.clone(),
            mode: self.mode.clone(),
            message_count: self.messages.len() as u32,
            total_input_tokens: self.total_input_tokens,
            total_output_tokens: self.total_output_tokens,
        }
    }

    pub fn add_message(&mut self, message: Message) {
        self.messages.push(message);
        self.updated_at = timestamp();
    }

    pub fn add_usage(&mut self, usage: Usage, model: &str) {
        let turn = TurnUsage {
            turn_index: self.turn_usage.len() as u32,
            usage: usage.clone(),
            model: model.to_string(),
            timestamp: timestamp(),
        };
        self.turn_usage.push(turn);
        self.total_input_tokens += usage.total_input();
        self.total_output_tokens += usage.output_tokens;
    }

    /// generate title from first user message if not set
    pub fn auto_title(&mut self) {
        if !self.title.is_empty() && self.title != "New Conversation" {
            return;
        }

        for msg in &self.messages {
            if msg.role == "user" {
                for block in &msg.content {
                    if let ContentBlock::Text { text } = block {
                        let preview: String = text.chars().take(50).collect();
                        self.title = if preview.len() < text.len() {
                            format!("{}...", preview.trim())
                        } else {
                            preview.trim().to_string()
                        };
                        return;
                    }
                }
            }
        }
    }
}

fn timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn get_db_path() -> PathBuf {
    #[cfg(target_os = "macos")]
    let base = dirs::data_dir();
    #[cfg(not(target_os = "macos"))]
    let base = dirs::data_local_dir();

    base.unwrap_or_else(|| PathBuf::from("."))
        .join("taskhomie")
        .join("conversations.db")
}

/// database singleton
static DB: std::sync::OnceLock<Mutex<Connection>> = std::sync::OnceLock::new();

/// initialize database - call at app startup
pub fn init_db() -> Result<(), String> {
    let db_path = get_db_path();

    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("failed to create db dir: {e}"))?;
    }

    println!("[storage] initializing db at {:?}", db_path);

    let conn = Connection::open(&db_path).map_err(|e| format!("failed to open db: {e}"))?;

    // create tables
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS conversations (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            model TEXT NOT NULL,
            mode TEXT NOT NULL,
            messages_json TEXT NOT NULL,
            turn_usage_json TEXT NOT NULL,
            total_input_tokens INTEGER NOT NULL DEFAULT 0,
            total_output_tokens INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_conversations_updated ON conversations(updated_at DESC);
        ",
    )
    .map_err(|e| format!("failed to create tables: {e}"))?;

    DB.set(Mutex::new(conn))
        .map_err(|_| "db already initialized")?;

    println!("[storage] db initialized");
    Ok(())
}

fn with_db<T, F>(f: F) -> Result<T, String>
where
    F: FnOnce(&Connection) -> SqlResult<T>,
{
    let guard = DB
        .get()
        .ok_or("db not initialized")?
        .lock()
        .map_err(|e| format!("lock error: {e}"))?;
    f(&guard).map_err(|e| format!("db error: {e}"))
}

// --- public API ---

/// create new conversation
pub fn create_conversation(title: String, model: String, mode: String) -> Result<String, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let conv = Conversation::new(id.clone(), title, model, mode);
    save_conversation(&conv)?;
    println!("[storage] created conversation {}", id);
    Ok(id)
}

/// save/update conversation
pub fn save_conversation(conv: &Conversation) -> Result<(), String> {
    let messages_json =
        serde_json::to_string(&conv.messages).map_err(|e| format!("serialize error: {e}"))?;
    let turn_usage_json =
        serde_json::to_string(&conv.turn_usage).map_err(|e| format!("serialize error: {e}"))?;

    with_db(|conn| {
        conn.execute(
            "INSERT OR REPLACE INTO conversations
             (id, title, created_at, updated_at, model, mode, messages_json, turn_usage_json, total_input_tokens, total_output_tokens)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                conv.id,
                conv.title,
                conv.created_at,
                conv.updated_at,
                conv.model,
                conv.mode,
                messages_json,
                turn_usage_json,
                conv.total_input_tokens,
                conv.total_output_tokens,
            ],
        )?;
        Ok(())
    })?;

    println!("[storage] saved conversation {}", conv.id);
    Ok(())
}

/// load conversation by id
pub fn load_conversation(id: &str) -> Result<Option<Conversation>, String> {
    with_db(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, title, created_at, updated_at, model, mode, messages_json, turn_usage_json, total_input_tokens, total_output_tokens
             FROM conversations WHERE id = ?1",
        )?;

        let result = stmt.query_row(params![id], |row| {
            let messages_json: String = row.get(6)?;
            let turn_usage_json: String = row.get(7)?;

            Ok(Conversation {
                id: row.get(0)?,
                title: row.get(1)?,
                created_at: row.get(2)?,
                updated_at: row.get(3)?,
                model: row.get(4)?,
                mode: row.get(5)?,
                messages: serde_json::from_str(&messages_json).unwrap_or_default(),
                turn_usage: serde_json::from_str(&turn_usage_json).unwrap_or_default(),
                total_input_tokens: row.get(8)?,
                total_output_tokens: row.get(9)?,
            })
        });

        match result {
            Ok(conv) => Ok(Some(conv)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    })
}

/// list conversations by recency
pub fn list_conversations(limit: usize, offset: usize) -> Result<Vec<ConversationMeta>, String> {
    with_db(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, title, created_at, updated_at, model, mode, messages_json, total_input_tokens, total_output_tokens
             FROM conversations ORDER BY updated_at DESC LIMIT ?1 OFFSET ?2",
        )?;

        let rows = stmt.query_map(params![limit as i64, offset as i64], |row| {
            let messages_json: String = row.get(6)?;
            let messages: Vec<Message> = serde_json::from_str(&messages_json).unwrap_or_default();

            Ok(ConversationMeta {
                id: row.get(0)?,
                title: row.get(1)?,
                created_at: row.get(2)?,
                updated_at: row.get(3)?,
                model: row.get(4)?,
                mode: row.get(5)?,
                message_count: messages.len() as u32,
                total_input_tokens: row.get(7)?,
                total_output_tokens: row.get(8)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>()
    })
}

/// delete conversation
pub fn delete_conversation(id: &str) -> Result<(), String> {
    with_db(|conn| {
        conn.execute("DELETE FROM conversations WHERE id = ?1", params![id])?;
        Ok(())
    })?;
    println!("[storage] deleted conversation {}", id);
    Ok(())
}

/// search conversations by title
pub fn search_conversations(query: &str, limit: usize) -> Result<Vec<ConversationMeta>, String> {
    let pattern = format!("%{}%", query);

    with_db(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, title, created_at, updated_at, model, mode, messages_json, total_input_tokens, total_output_tokens
             FROM conversations WHERE title LIKE ?1 ORDER BY updated_at DESC LIMIT ?2",
        )?;

        let rows = stmt.query_map(params![pattern, limit as i64], |row| {
            let messages_json: String = row.get(6)?;
            let messages: Vec<Message> = serde_json::from_str(&messages_json).unwrap_or_default();

            Ok(ConversationMeta {
                id: row.get(0)?,
                title: row.get(1)?,
                created_at: row.get(2)?,
                updated_at: row.get(3)?,
                model: row.get(4)?,
                mode: row.get(5)?,
                message_count: messages.len() as u32,
                total_input_tokens: row.get(7)?,
                total_output_tokens: row.get(8)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>()
    })
}

/// get total usage across all conversations
pub fn get_total_usage() -> Result<(u32, u32), String> {
    with_db(|conn| {
        let mut stmt = conn.prepare(
            "SELECT COALESCE(SUM(total_input_tokens), 0), COALESCE(SUM(total_output_tokens), 0) FROM conversations",
        )?;

        stmt.query_row([], |row| {
            let input: i64 = row.get(0)?;
            let output: i64 = row.get(1)?;
            Ok((input as u32, output as u32))
        })
    })
}

/// count total conversations
pub fn count_conversations() -> Result<u32, String> {
    with_db(|conn| {
        let mut stmt = conn.prepare("SELECT COUNT(*) FROM conversations")?;
        stmt.query_row([], |row| {
            let count: i64 = row.get(0)?;
            Ok(count as u32)
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_usage_totals() {
        let usage = Usage {
            input_tokens: 100,
            output_tokens: 50,
            cache_creation_input_tokens: 200,
            cache_read_input_tokens: 0,
        };
        assert_eq!(usage.total_input(), 300);
        assert_eq!(usage.total(), 350);
    }

    #[test]
    fn test_conversation_auto_title() {
        let mut conv = Conversation::new(
            "test".to_string(),
            "New Conversation".to_string(),
            "claude-sonnet".to_string(),
            "computer".to_string(),
        );

        conv.add_message(Message {
            role: "user".to_string(),
            content: vec![ContentBlock::Text {
                text: "Hello, can you help me with something?".to_string(),
            }],
        });

        conv.auto_title();
        assert_eq!(conv.title, "Hello, can you help me with something?");
    }
}

// Rust guideline compliant 2025-12-29
