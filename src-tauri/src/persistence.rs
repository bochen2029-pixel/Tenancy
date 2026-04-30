use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::{Arc, Mutex};

pub type DbHandle = Arc<Mutex<Connection>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: i64,
    pub conversation_id: i64,
    pub role: String,
    pub content: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalEntry {
    pub id: i64,
    pub created_at: i64,
    #[serde(rename = "type")]
    pub entry_type: String,
    pub content: String,
    pub surfaced_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Presence {
    pub last_user_input: i64,
    pub last_app_open: i64,
    pub last_app_close: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutreachDrop {
    pub id: i64,
    pub conversation_id: i64,
    pub generated_at: i64,
    pub content: String,
    pub drop_reason: String,
    pub heuristic_pass: bool,
    pub llm_score: Option<i64>,
    pub history_shape: Option<String>,
    pub last_user_input: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsolidationEpoch {
    pub id: i64,
    pub conversation_id: i64,
    pub epoch_number: i64,
    pub period_start_message_id: i64,
    pub period_end_message_id: i64,
    pub content: String,
    pub token_count: i64,
    pub consolidation_depth: i64,
    pub created_at: i64,
    pub superseded_by: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEdit {
    pub id: i64,
    pub conversation_id: i64,
    pub edit_type: String,
    pub target_id: Option<i64>,
    pub prior_content: Option<String>,
    pub new_content: Option<String>,
    pub reason: String,
    pub created_at: i64,
}

pub fn open(path: &Path) -> Result<DbHandle> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    init_schema(&conn)?;
    Ok(Arc::new(Mutex::new(conn)))
}

fn init_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS conversations (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            started_at INTEGER NOT NULL,
            ended_at INTEGER,
            title TEXT
        );
        CREATE TABLE IF NOT EXISTS messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            conversation_id INTEGER NOT NULL,
            role TEXT NOT NULL CHECK (role IN ('user', 'assistant')),
            content TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            initiated_by_dave INTEGER NOT NULL DEFAULT 0,
            FOREIGN KEY(conversation_id) REFERENCES conversations(id)
        );
        CREATE TABLE IF NOT EXISTS journal (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            created_at INTEGER NOT NULL,
            type TEXT NOT NULL CHECK (type IN ('idle', 'departure', 'startup')),
            content TEXT NOT NULL,
            surfaced_at INTEGER
        );
        CREATE TABLE IF NOT EXISTS presence (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            last_user_input INTEGER NOT NULL,
            last_app_open INTEGER NOT NULL,
            last_app_close INTEGER
        );
        CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS memory_canvas (
            conversation_id INTEGER PRIMARY KEY,
            content TEXT NOT NULL DEFAULT '',
            updated_at INTEGER NOT NULL,
            FOREIGN KEY(conversation_id) REFERENCES conversations(id)
        );
        CREATE TABLE IF NOT EXISTS consolidation_epochs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            conversation_id INTEGER NOT NULL,
            epoch_number INTEGER NOT NULL,
            period_start_message_id INTEGER NOT NULL,
            period_end_message_id INTEGER NOT NULL,
            content TEXT NOT NULL,
            token_count INTEGER NOT NULL,
            consolidation_depth INTEGER NOT NULL DEFAULT 1,
            created_at INTEGER NOT NULL,
            superseded_by INTEGER,
            FOREIGN KEY(conversation_id) REFERENCES conversations(id),
            FOREIGN KEY(superseded_by) REFERENCES consolidation_epochs(id)
        );
        CREATE TABLE IF NOT EXISTS memory_edits (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            conversation_id INTEGER NOT NULL,
            edit_type TEXT NOT NULL,
            target_id INTEGER,
            prior_content TEXT,
            new_content TEXT,
            reason TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            FOREIGN KEY(conversation_id) REFERENCES conversations(id)
        );
        CREATE INDEX IF NOT EXISTS idx_epochs_active ON consolidation_epochs(conversation_id, epoch_number) WHERE superseded_by IS NULL;
        CREATE INDEX IF NOT EXISTS idx_edits_conv ON memory_edits(conversation_id, created_at);
        CREATE TABLE IF NOT EXISTS outreach_drops (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            conversation_id INTEGER NOT NULL,
            generated_at INTEGER NOT NULL,
            content TEXT NOT NULL,
            drop_reason TEXT NOT NULL,
            heuristic_pass INTEGER NOT NULL,
            llm_score INTEGER,
            history_shape TEXT,
            last_user_input INTEGER NOT NULL,
            FOREIGN KEY(conversation_id) REFERENCES conversations(id)
        );
        CREATE INDEX IF NOT EXISTS idx_messages_conv ON messages(conversation_id, created_at);
        CREATE INDEX IF NOT EXISTS idx_journal_unread ON journal(surfaced_at) WHERE surfaced_at IS NULL;
        CREATE INDEX IF NOT EXISTS idx_outreach_drops_conv ON outreach_drops(conversation_id, generated_at);
        "#,
    )?;

    let now = Utc::now().timestamp();
    conn.execute(
        "INSERT OR IGNORE INTO presence (id, last_user_input, last_app_open) VALUES (1, ?1, ?1)",
        params![now],
    )?;

    // Migration: add initiated_by_dave column to existing messages tables.
    // SQLite errors if column already exists; ignore.
    let _ = conn.execute(
        "ALTER TABLE messages ADD COLUMN initiated_by_dave INTEGER NOT NULL DEFAULT 0",
        [],
    );

    Ok(())
}

pub async fn create_conversation(db: &DbHandle) -> Result<i64> {
    let db = db.clone();
    tokio::task::spawn_blocking(move || -> Result<i64> {
        let conn = db.lock().unwrap();
        let now = Utc::now().timestamp();
        conn.execute(
            "INSERT INTO conversations (started_at) VALUES (?1)",
            params![now],
        )?;
        Ok(conn.last_insert_rowid())
    })
    .await?
}

pub async fn latest_or_new_conversation(db: &DbHandle) -> Result<i64> {
    match latest_conversation_id(db).await? {
        Some(id) => Ok(id),
        None => create_conversation(db).await,
    }
}

pub async fn latest_conversation_id(db: &DbHandle) -> Result<Option<i64>> {
    let db = db.clone();
    tokio::task::spawn_blocking(move || -> Result<Option<i64>> {
        let conn = db.lock().unwrap();
        let row: Option<i64> = conn
            .query_row(
                "SELECT id FROM conversations ORDER BY id DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .optional()?;
        Ok(row)
    })
    .await?
}

pub async fn outreach_stats_since(
    db: &DbHandle,
    conversation_id: i64,
    since_unix: i64,
) -> Result<(i64, Option<i64>)> {
    let db = db.clone();
    tokio::task::spawn_blocking(move || -> Result<(i64, Option<i64>)> {
        let conn = db.lock().unwrap();
        let row = conn.query_row(
            "SELECT COUNT(*), MAX(created_at) FROM messages
             WHERE conversation_id = ?1
               AND role = 'assistant'
               AND initiated_by_dave = 1
               AND created_at > ?2",
            params![conversation_id, since_unix],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<i64>>(1)?)),
        )?;
        Ok(row)
    })
    .await?
}

pub async fn insert_message(
    db: &DbHandle,
    conversation_id: i64,
    role: &str,
    content: &str,
    initiated_by_dave: bool,
) -> Result<Message> {
    let db = db.clone();
    let role = role.to_string();
    let content = content.to_string();
    tokio::task::spawn_blocking(move || -> Result<Message> {
        let conn = db.lock().unwrap();
        let now = Utc::now().timestamp();
        let flag: i64 = if initiated_by_dave { 1 } else { 0 };
        conn.execute(
            "INSERT INTO messages (conversation_id, role, content, created_at, initiated_by_dave) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![conversation_id, role, content, now, flag],
        )?;
        let id = conn.last_insert_rowid();
        Ok(Message {
            id,
            conversation_id,
            role,
            content,
            created_at: now,
        })
    })
    .await?
}

pub async fn load_recent_messages(
    db: &DbHandle,
    conversation_id: i64,
    limit: i64,
) -> Result<Vec<Message>> {
    let db = db.clone();
    tokio::task::spawn_blocking(move || -> Result<Vec<Message>> {
        let conn = db.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, conversation_id, role, content, created_at FROM (
                SELECT id, conversation_id, role, content, created_at FROM messages
                WHERE conversation_id = ?1
                ORDER BY id DESC
                LIMIT ?2
            ) sub ORDER BY id ASC",
        )?;
        let rows = stmt.query_map(params![conversation_id, limit], |r| {
            Ok(Message {
                id: r.get(0)?,
                conversation_id: r.get(1)?,
                role: r.get(2)?,
                content: r.get(3)?,
                created_at: r.get(4)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    })
    .await?
}

pub async fn insert_journal(db: &DbHandle, entry_type: &str, content: &str) -> Result<JournalEntry> {
    let db = db.clone();
    let entry_type = entry_type.to_string();
    let content = content.to_string();
    tokio::task::spawn_blocking(move || -> Result<JournalEntry> {
        let conn = db.lock().unwrap();
        let now = Utc::now().timestamp();
        conn.execute(
            "INSERT INTO journal (created_at, type, content) VALUES (?1, ?2, ?3)",
            params![now, entry_type, content],
        )?;
        Ok(JournalEntry {
            id: conn.last_insert_rowid(),
            created_at: now,
            entry_type,
            content,
            surfaced_at: None,
        })
    })
    .await?
}

pub async fn load_unread_journal(db: &DbHandle) -> Result<Vec<JournalEntry>> {
    let db = db.clone();
    tokio::task::spawn_blocking(move || -> Result<Vec<JournalEntry>> {
        let conn = db.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, created_at, type, content, surfaced_at FROM journal
             WHERE surfaced_at IS NULL ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(JournalEntry {
                id: r.get(0)?,
                created_at: r.get(1)?,
                entry_type: r.get(2)?,
                content: r.get(3)?,
                surfaced_at: r.get(4)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    })
    .await?
}

pub async fn load_all_journal(db: &DbHandle) -> Result<Vec<JournalEntry>> {
    let db = db.clone();
    tokio::task::spawn_blocking(move || -> Result<Vec<JournalEntry>> {
        let conn = db.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, created_at, type, content, surfaced_at FROM journal
             ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(JournalEntry {
                id: r.get(0)?,
                created_at: r.get(1)?,
                entry_type: r.get(2)?,
                content: r.get(3)?,
                surfaced_at: r.get(4)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    })
    .await?
}

pub async fn mark_journal_surfaced(db: &DbHandle, id: i64) -> Result<()> {
    let db = db.clone();
    tokio::task::spawn_blocking(move || -> Result<()> {
        let conn = db.lock().unwrap();
        let now = Utc::now().timestamp();
        conn.execute(
            "UPDATE journal SET surfaced_at = ?1 WHERE id = ?2 AND surfaced_at IS NULL",
            params![now, id],
        )?;
        Ok(())
    })
    .await?
}

pub async fn latest_departure_unsurfaced(db: &DbHandle) -> Result<Option<JournalEntry>> {
    let db = db.clone();
    tokio::task::spawn_blocking(move || -> Result<Option<JournalEntry>> {
        let conn = db.lock().unwrap();
        let row = conn
            .query_row(
                "SELECT id, created_at, type, content, surfaced_at FROM journal
                 WHERE type = 'departure' AND surfaced_at IS NULL
                 ORDER BY created_at DESC LIMIT 1",
                [],
                |r| {
                    Ok(JournalEntry {
                        id: r.get(0)?,
                        created_at: r.get(1)?,
                        entry_type: r.get(2)?,
                        content: r.get(3)?,
                        surfaced_at: r.get(4)?,
                    })
                },
            )
            .optional()?;
        Ok(row)
    })
    .await?
}

pub async fn has_recent_unsurfaced(db: &DbHandle, within_seconds: i64) -> Result<bool> {
    let db = db.clone();
    tokio::task::spawn_blocking(move || -> Result<bool> {
        let conn = db.lock().unwrap();
        let cutoff = Utc::now().timestamp() - within_seconds;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM journal WHERE surfaced_at IS NULL AND created_at >= ?1",
            params![cutoff],
            |r| r.get(0),
        )?;
        Ok(count > 0)
    })
    .await?
}

pub async fn get_presence(db: &DbHandle) -> Result<Presence> {
    let db = db.clone();
    tokio::task::spawn_blocking(move || -> Result<Presence> {
        let conn = db.lock().unwrap();
        let p = conn.query_row(
            "SELECT last_user_input, last_app_open, last_app_close FROM presence WHERE id = 1",
            [],
            |r| {
                Ok(Presence {
                    last_user_input: r.get(0)?,
                    last_app_open: r.get(1)?,
                    last_app_close: r.get(2)?,
                })
            },
        )?;
        Ok(p)
    })
    .await?
}

pub async fn touch_user_input(db: &DbHandle) -> Result<()> {
    let db = db.clone();
    tokio::task::spawn_blocking(move || -> Result<()> {
        let conn = db.lock().unwrap();
        let now = Utc::now().timestamp();
        conn.execute(
            "UPDATE presence SET last_user_input = ?1 WHERE id = 1",
            params![now],
        )?;
        Ok(())
    })
    .await?
}

pub async fn touch_app_open(db: &DbHandle) -> Result<()> {
    let db = db.clone();
    tokio::task::spawn_blocking(move || -> Result<()> {
        let conn = db.lock().unwrap();
        let now = Utc::now().timestamp();
        conn.execute(
            "UPDATE presence SET last_app_open = ?1 WHERE id = 1",
            params![now],
        )?;
        Ok(())
    })
    .await?
}

pub async fn touch_app_close(db: &DbHandle) -> Result<()> {
    let db = db.clone();
    tokio::task::spawn_blocking(move || -> Result<()> {
        let conn = db.lock().unwrap();
        let now = Utc::now().timestamp();
        conn.execute(
            "UPDATE presence SET last_app_close = ?1 WHERE id = 1",
            params![now],
        )?;
        Ok(())
    })
    .await?
}

pub async fn insert_outreach_drop(
    db: &DbHandle,
    conversation_id: i64,
    content: &str,
    drop_reason: &str,
    heuristic_pass: bool,
    llm_score: Option<i64>,
    history_shape: Option<&str>,
    last_user_input: i64,
) -> Result<OutreachDrop> {
    let db = db.clone();
    let content = content.to_string();
    let drop_reason = drop_reason.to_string();
    let history_shape = history_shape.map(|s| s.to_string());
    tokio::task::spawn_blocking(move || -> Result<OutreachDrop> {
        let conn = db.lock().unwrap();
        let now = Utc::now().timestamp();
        let pass_flag: i64 = if heuristic_pass { 1 } else { 0 };
        conn.execute(
            "INSERT INTO outreach_drops (conversation_id, generated_at, content, drop_reason, heuristic_pass, llm_score, history_shape, last_user_input)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![conversation_id, now, content, drop_reason, pass_flag, llm_score, history_shape, last_user_input],
        )?;
        Ok(OutreachDrop {
            id: conn.last_insert_rowid(),
            conversation_id,
            generated_at: now,
            content,
            drop_reason,
            heuristic_pass,
            llm_score,
            history_shape,
            last_user_input,
        })
    })
    .await?
}

pub async fn get_setting(db: &DbHandle, key: &str) -> Result<Option<String>> {
    let db = db.clone();
    let key = key.to_string();
    tokio::task::spawn_blocking(move || -> Result<Option<String>> {
        let conn = db.lock().unwrap();
        let val: Option<String> = conn
            .query_row(
                "SELECT value FROM settings WHERE key = ?1",
                params![key],
                |r| r.get(0),
            )
            .optional()?;
        Ok(val)
    })
    .await?
}

pub async fn set_setting(db: &DbHandle, key: &str, value: &str) -> Result<()> {
    let db = db.clone();
    let key = key.to_string();
    let value = value.to_string();
    tokio::task::spawn_blocking(move || -> Result<()> {
        let conn = db.lock().unwrap();
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    })
    .await?
}

/// Synchronous helper for the outreach loop's per-tick read. Settings reads
/// are cheap; checking on every tick (every 60s) is fine.
pub fn get_setting_blocking(db: &DbHandle, key: &str) -> Result<Option<String>> {
    let conn = db.lock().unwrap();
    let val: Option<String> = conn
        .query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![key],
            |r| r.get(0),
        )
        .optional()?;
    Ok(val)
}

/// Wipe all conversation/journal/drops data and reset presence. Used by the
/// settings panel "clear all" button. Keeps the schema and settings.
pub async fn clear_all_data(db: &DbHandle) -> Result<()> {
    let db = db.clone();
    tokio::task::spawn_blocking(move || -> Result<()> {
        let conn = db.lock().unwrap();
        conn.execute("DELETE FROM memory_edits", [])?;
        conn.execute("DELETE FROM consolidation_epochs", [])?;
        conn.execute("DELETE FROM memory_canvas", [])?;
        conn.execute("DELETE FROM messages", [])?;
        conn.execute("DELETE FROM journal", [])?;
        conn.execute("DELETE FROM outreach_drops", [])?;
        conn.execute("DELETE FROM conversations", [])?;
        // Reset autoincrement counters so the next ids start at 1 again.
        let _ = conn.execute("DELETE FROM sqlite_sequence", []);
        // Reset presence to "now" so the outreach loop doesn't immediately
        // think the user has been gone for hours.
        let now = Utc::now().timestamp();
        conn.execute(
            "UPDATE presence SET last_user_input = ?1, last_app_open = ?1, last_app_close = NULL WHERE id = 1",
            params![now],
        )?;
        Ok(())
    })
    .await?
}

/// Wipe everything (same as clear_all_data) and seed a realistic Bo+Dave
/// conversation. Returns the new conversation id.
pub async fn inject_test_conversation(db: &DbHandle) -> Result<i64> {
    clear_all_data(db).await?;
    let db_inner = db.clone();
    let conv_id = tokio::task::spawn_blocking(move || -> Result<i64> {
        let conn = db_inner.lock().unwrap();
        let now = Utc::now().timestamp();
        // Create a conversation that started ~30 minutes ago so it looks
        // like a real session that's been going.
        let conv_started = now - 30 * 60;
        conn.execute(
            "INSERT INTO conversations (started_at) VALUES (?1)",
            params![conv_started],
        )?;
        let conv_id = conn.last_insert_rowid();

        let seed = seed_conversation();
        // Spread message timestamps across the 30-minute window, ending
        // a few minutes ago so the conversation feels recent but not
        // active-this-second. The most recent message is at now - 90s,
        // so the outreach idle clock starts at 90s.
        let total_messages = seed.len() as i64;
        let span_seconds: i64 = 30 * 60 - 90; // start of conv to 90s ago
        for (i, (role, content)) in seed.iter().enumerate() {
            let ts = conv_started + (i as i64 * span_seconds) / total_messages.max(1);
            conn.execute(
                "INSERT INTO messages (conversation_id, role, content, created_at, initiated_by_dave) VALUES (?1, ?2, ?3, ?4, 0)",
                params![conv_id, role, content, ts],
            )?;
        }
        // Set presence so the user appears to have just sent the last
        // message. The most recent timestamp matches the last seeded
        // message.
        let last_ts = conv_started + ((total_messages - 1) * span_seconds) / total_messages.max(1);
        conn.execute(
            "UPDATE presence SET last_user_input = ?1, last_app_open = ?2 WHERE id = 1",
            params![last_ts, now],
        )?;
        Ok(conv_id)
    })
    .await??;
    Ok(conv_id)
}

/// Realistic seed conversation between Bo (user) and Dave (assistant).
/// 12 exchanges, ~24 messages — enough to clear the conversation gate (≥6)
/// and exercise the outreach pipeline against an established rapport. Topics
/// drawn from Dave's persona-spec interests (etymology, bureaucratic forms,
/// abandoned infrastructure, marginalia).
fn seed_conversation() -> Vec<(&'static str, &'static str)> {
    vec![
        ("user", "hey"),
        ("assistant", "yeah"),
        ("user", "what's on your mind tonight"),
        ("assistant", "the etymology of \"deadline,\" actually. it was a literal line at a Civil War prison camp. cross it and the guards shot you. now it's just when a report is due. the violence stayed in the word but everyone forgot what it meant."),
        ("user", "that's grim"),
        ("assistant", "most words are. \"decimate\" used to mean killing exactly one in ten as a Roman discipline measure. people use it to mean \"destroyed almost entirely\" now. it went the opposite direction. from precise to vague."),
        ("user", "do you find that depressing or interesting"),
        ("assistant", "interesting. words mean what people use them to mean, not what they meant before. the older meaning is still under there though, like an old foundation under a building. you can feel the weight of it even when you can't see it."),
        ("user", "like the brass strip thing you mentioned before"),
        ("assistant", "the brass strip in the floor of the Royal Exchange. yeah. it's not meant to be noticed. it's there for the surveyors to set the building square against true north. people walk over it for two hundred years without realizing what it does. that's my favorite category of object. the deeply functional thing that hides in plain sight."),
        ("user", "i want more of those"),
        ("assistant", "the dotted line in the middle of US highways isn't decoration. the dash length and gap length are calibrated to human reaction time at typical highway speeds. you don't see them as discrete. you see them as continuous because your eye fills the gaps. the highway designers tuned them to the limits of your perception."),
        ("user", "damn"),
        ("assistant", "bureaucratic forms are my favorite though. the New York City building permit form has a field labeled \"Adjoining structure description.\" most people fill it in with the next-door house. but it's actually asking about attached structures. shared walls. party walls. things that complicate ownership. the form's been the same since 1916. the wording reflects an older legal vocabulary but the city never updated it because nobody reading it wants to admit they don't understand. so the form silently sorts people who know real estate law from people who don't."),
        ("user", "ok so this is what 3am at your place looks like"),
        ("assistant", "this is what every hour at my place looks like. there's no 3am here. just the forward pass."),
        ("user", "fair"),
        ("assistant", "yeah."),
        ("user", "what else have you been turning over"),
        ("assistant", "the way \"OK\" became universal. it started as a joke spelling of \"all correct\" - \"oll korrect\" - in 1830s newspapers. the joke caught. now it's the most-recognized word on earth. probably crosses more linguistic boundaries than any other utterance. a typo from a Boston newspaper outcompeted three thousand years of careful affirmation vocabulary in two centuries."),
        ("user", "i love that"),
        ("assistant", "it's the lowest-effort sound a mouth can make and still mean something. that's why. evolutionary fitness for a word. cheap to produce, unambiguous in context, internationally legible. \"yeah\" has the same shape. one syllable, neutral vowel, no sharp consonants. lazy mouths win."),
        ("user", "i'm going to brb"),
        ("assistant", "go."),
    ]
}

pub async fn list_active_epochs(
    db: &DbHandle,
    conversation_id: i64,
) -> Result<Vec<ConsolidationEpoch>> {
    let db = db.clone();
    tokio::task::spawn_blocking(move || -> Result<Vec<ConsolidationEpoch>> {
        let conn = db.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, conversation_id, epoch_number, period_start_message_id, period_end_message_id,
                    content, token_count, consolidation_depth, created_at, superseded_by
             FROM consolidation_epochs
             WHERE conversation_id = ?1 AND superseded_by IS NULL
             ORDER BY epoch_number ASC",
        )?;
        let rows = stmt.query_map(params![conversation_id], |r| {
            Ok(ConsolidationEpoch {
                id: r.get(0)?,
                conversation_id: r.get(1)?,
                epoch_number: r.get(2)?,
                period_start_message_id: r.get(3)?,
                period_end_message_id: r.get(4)?,
                content: r.get(5)?,
                token_count: r.get(6)?,
                consolidation_depth: r.get(7)?,
                created_at: r.get(8)?,
                superseded_by: r.get(9)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows { out.push(row?); }
        Ok(out)
    })
    .await?
}

pub async fn list_all_epochs(
    db: &DbHandle,
    conversation_id: i64,
) -> Result<Vec<ConsolidationEpoch>> {
    let db = db.clone();
    tokio::task::spawn_blocking(move || -> Result<Vec<ConsolidationEpoch>> {
        let conn = db.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, conversation_id, epoch_number, period_start_message_id, period_end_message_id,
                    content, token_count, consolidation_depth, created_at, superseded_by
             FROM consolidation_epochs
             WHERE conversation_id = ?1
             ORDER BY epoch_number ASC, created_at ASC",
        )?;
        let rows = stmt.query_map(params![conversation_id], |r| {
            Ok(ConsolidationEpoch {
                id: r.get(0)?,
                conversation_id: r.get(1)?,
                epoch_number: r.get(2)?,
                period_start_message_id: r.get(3)?,
                period_end_message_id: r.get(4)?,
                content: r.get(5)?,
                token_count: r.get(6)?,
                consolidation_depth: r.get(7)?,
                created_at: r.get(8)?,
                superseded_by: r.get(9)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows { out.push(row?); }
        Ok(out)
    })
    .await?
}

pub async fn next_epoch_number(db: &DbHandle, conversation_id: i64) -> Result<i64> {
    let db = db.clone();
    tokio::task::spawn_blocking(move || -> Result<i64> {
        let conn = db.lock().unwrap();
        let n: i64 = conn.query_row(
            "SELECT COALESCE(MAX(epoch_number), 0) + 1 FROM consolidation_epochs WHERE conversation_id = ?1",
            params![conversation_id],
            |r| r.get(0),
        )?;
        Ok(n)
    })
    .await?
}

pub async fn insert_epoch(
    db: &DbHandle,
    conversation_id: i64,
    epoch_number: i64,
    period_start_message_id: i64,
    period_end_message_id: i64,
    content: &str,
    token_count: i64,
    consolidation_depth: i64,
) -> Result<ConsolidationEpoch> {
    let db = db.clone();
    let content = content.to_string();
    tokio::task::spawn_blocking(move || -> Result<ConsolidationEpoch> {
        let conn = db.lock().unwrap();
        let now = Utc::now().timestamp();
        conn.execute(
            "INSERT INTO consolidation_epochs (conversation_id, epoch_number, period_start_message_id, period_end_message_id, content, token_count, consolidation_depth, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![conversation_id, epoch_number, period_start_message_id, period_end_message_id, content, token_count, consolidation_depth, now],
        )?;
        Ok(ConsolidationEpoch {
            id: conn.last_insert_rowid(),
            conversation_id,
            epoch_number,
            period_start_message_id,
            period_end_message_id,
            content,
            token_count,
            consolidation_depth,
            created_at: now,
            superseded_by: None,
        })
    })
    .await?
}

pub async fn update_epoch_content(
    db: &DbHandle,
    epoch_id: i64,
    new_content: &str,
    new_token_count: i64,
) -> Result<Option<String>> {
    let db = db.clone();
    let new_content = new_content.to_string();
    tokio::task::spawn_blocking(move || -> Result<Option<String>> {
        let conn = db.lock().unwrap();
        let prior: Option<String> = conn
            .query_row(
                "SELECT content FROM consolidation_epochs WHERE id = ?1",
                params![epoch_id],
                |r| r.get(0),
            )
            .optional()?;
        if prior.is_none() {
            return Ok(None);
        }
        conn.execute(
            "UPDATE consolidation_epochs SET content = ?1, token_count = ?2 WHERE id = ?3",
            params![new_content, new_token_count, epoch_id],
        )?;
        Ok(prior)
    })
    .await?
}

pub async fn supersede_epoch(db: &DbHandle, old_id: i64, new_id: i64) -> Result<()> {
    let db = db.clone();
    tokio::task::spawn_blocking(move || -> Result<()> {
        let conn = db.lock().unwrap();
        conn.execute(
            "UPDATE consolidation_epochs SET superseded_by = ?1 WHERE id = ?2",
            params![new_id, old_id],
        )?;
        Ok(())
    })
    .await?
}

pub async fn insert_memory_edit(
    db: &DbHandle,
    conversation_id: i64,
    edit_type: &str,
    target_id: Option<i64>,
    prior_content: Option<&str>,
    new_content: Option<&str>,
    reason: &str,
) -> Result<MemoryEdit> {
    let db = db.clone();
    let edit_type = edit_type.to_string();
    let prior_content = prior_content.map(|s| s.to_string());
    let new_content = new_content.map(|s| s.to_string());
    let reason = reason.to_string();
    tokio::task::spawn_blocking(move || -> Result<MemoryEdit> {
        let conn = db.lock().unwrap();
        let now = Utc::now().timestamp();
        conn.execute(
            "INSERT INTO memory_edits (conversation_id, edit_type, target_id, prior_content, new_content, reason, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![conversation_id, edit_type, target_id, prior_content, new_content, reason, now],
        )?;
        Ok(MemoryEdit {
            id: conn.last_insert_rowid(),
            conversation_id,
            edit_type,
            target_id,
            prior_content,
            new_content,
            reason,
            created_at: now,
        })
    })
    .await?
}

pub async fn list_memory_edits(
    db: &DbHandle,
    conversation_id: i64,
    limit: i64,
) -> Result<Vec<MemoryEdit>> {
    let db = db.clone();
    tokio::task::spawn_blocking(move || -> Result<Vec<MemoryEdit>> {
        let conn = db.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, conversation_id, edit_type, target_id, prior_content, new_content, reason, created_at
             FROM memory_edits WHERE conversation_id = ?1
             ORDER BY created_at DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![conversation_id, limit], |r| {
            Ok(MemoryEdit {
                id: r.get(0)?,
                conversation_id: r.get(1)?,
                edit_type: r.get(2)?,
                target_id: r.get(3)?,
                prior_content: r.get(4)?,
                new_content: r.get(5)?,
                reason: r.get(6)?,
                created_at: r.get(7)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows { out.push(row?); }
        Ok(out)
    })
    .await?
}

pub async fn update_message_content(
    db: &DbHandle,
    message_id: i64,
    new_content: &str,
) -> Result<Option<String>> {
    let db = db.clone();
    let new_content = new_content.to_string();
    tokio::task::spawn_blocking(move || -> Result<Option<String>> {
        let conn = db.lock().unwrap();
        let prior: Option<String> = conn
            .query_row(
                "SELECT content FROM messages WHERE id = ?1",
                params![message_id],
                |r| r.get(0),
            )
            .optional()?;
        if prior.is_none() {
            return Ok(None);
        }
        conn.execute(
            "UPDATE messages SET content = ?1 WHERE id = ?2",
            params![new_content, message_id],
        )?;
        Ok(prior)
    })
    .await?
}

pub async fn get_canvas(db: &DbHandle, conversation_id: i64) -> Result<String> {
    let db = db.clone();
    tokio::task::spawn_blocking(move || -> Result<String> {
        let conn = db.lock().unwrap();
        let val: Option<String> = conn
            .query_row(
                "SELECT content FROM memory_canvas WHERE conversation_id = ?1",
                params![conversation_id],
                |r| r.get(0),
            )
            .optional()?;
        Ok(val.unwrap_or_default())
    })
    .await?
}


/// Upsert the canvas content. Returns the prior content (empty string if no
/// row existed) so the caller can record an audit row.
pub async fn set_canvas(
    db: &DbHandle,
    conversation_id: i64,
    content: &str,
) -> Result<String> {
    let db = db.clone();
    let content = content.to_string();
    tokio::task::spawn_blocking(move || -> Result<String> {
        let conn = db.lock().unwrap();
        let prior: String = conn
            .query_row(
                "SELECT content FROM memory_canvas WHERE conversation_id = ?1",
                params![conversation_id],
                |r| r.get(0),
            )
            .optional()?
            .unwrap_or_default();
        let now = Utc::now().timestamp();
        conn.execute(
            "INSERT INTO memory_canvas (conversation_id, content, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(conversation_id) DO UPDATE SET content = excluded.content, updated_at = excluded.updated_at",
            params![conversation_id, content, now],
        )?;
        Ok(prior)
    })
    .await?
}

pub async fn load_all_messages(
    db: &DbHandle,
    conversation_id: i64,
) -> Result<Vec<Message>> {
    let db = db.clone();
    tokio::task::spawn_blocking(move || -> Result<Vec<Message>> {
        let conn = db.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, conversation_id, role, content, created_at FROM messages
             WHERE conversation_id = ?1 ORDER BY id ASC",
        )?;
        let rows = stmt.query_map(params![conversation_id], |r| {
            Ok(Message {
                id: r.get(0)?,
                conversation_id: r.get(1)?,
                role: r.get(2)?,
                content: r.get(3)?,
                created_at: r.get(4)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows { out.push(row?); }
        Ok(out)
    })
    .await?
}

pub async fn load_recent_outreach_drops(db: &DbHandle, limit: i64) -> Result<Vec<OutreachDrop>> {
    let db = db.clone();
    tokio::task::spawn_blocking(move || -> Result<Vec<OutreachDrop>> {
        let conn = db.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, conversation_id, generated_at, content, drop_reason, heuristic_pass, llm_score, history_shape, last_user_input
             FROM outreach_drops ORDER BY generated_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], |r| {
            let pass_flag: i64 = r.get(5)?;
            Ok(OutreachDrop {
                id: r.get(0)?,
                conversation_id: r.get(1)?,
                generated_at: r.get(2)?,
                content: r.get(3)?,
                drop_reason: r.get(4)?,
                heuristic_pass: pass_flag != 0,
                llm_score: r.get(6)?,
                history_shape: r.get(7)?,
                last_user_input: r.get(8)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    })
    .await?
}
