use serde::{Deserialize, Serialize};

/// Sync status exposed to frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncStatus {
    pub state: SyncState,
    pub last_sync_at: Option<String>,
    pub pending_changes: u64,
    pub error_message: Option<String>,
    pub connected_email: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SyncState {
    Disabled,
    Idle,
    Syncing,
    Error,
    Offline,
}

/// Sync settings stored in DB (sync_meta + settings tables)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncSettings {
    pub enabled: bool,
    pub interval_seconds: u64,
    pub sync_images: bool,
}

impl Default for SyncSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            interval_seconds: 300, // 5 minutes
            sync_images: true,
        }
    }
}

/// Represents a clip serialized for sync (uploaded to Drive)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncClip {
    pub uuid: String,
    pub clip_type: String,
    pub text_preview: String,
    pub content_hash: String,
    pub folder_uuid: Option<String>,
    pub source_app: Option<String>,
    pub metadata: Option<String>,
    pub subtype: Option<String>,
    pub note: Option<String>,
    pub paste_count: i64,
    pub is_pinned: bool,
    pub is_sensitive: bool,
    pub created_at: String,
    pub updated_at: String,
    /// For text clips: full content. For image clips: None (image synced separately).
    pub text_content: Option<String>,
}

/// Represents a folder serialized for sync
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncFolder {
    pub uuid: String,
    pub name: String,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub position: i64,
    pub created_at: String,
    pub updated_at: String,
}

/// Represents a scratchpad note serialized for sync
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncScratchpad {
    pub uuid: String,
    pub title: String,
    pub content: String,
    #[serde(default)]
    pub fields_json: Option<String>,
    #[serde(default)]
    pub is_pinned: bool,
    #[serde(default)]
    pub color: Option<String>,
    pub position: i64,
    pub created_at: String,
    pub updated_at: String,
}

/// Deletion tombstone — propagates deletes across devices
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tombstone {
    pub uuid: String,
    pub entity_type: String, // "clip" or "folder"
    pub deleted_at: String,
}

/// Lightweight sync state index stored on Drive
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SyncIndex {
    /// All known clip UUIDs with their updated_at timestamps
    pub clips: Vec<SyncIndexEntry>,
    /// All known folder UUIDs with their updated_at timestamps
    pub folders: Vec<SyncIndexEntry>,
    /// Active tombstones
    pub tombstones: Vec<Tombstone>,
    /// Device that last wrote this index
    pub last_device_id: String,
    /// Timestamp of last index update
    pub last_updated: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncIndexEntry {
    pub uuid: String,
    pub updated_at: String,
    pub content_hash: Option<String>,
}

/// Info about the connected Google account
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleAccountInfo {
    pub email: String,
    pub display_name: Option<String>,
}

/// OAuth2 tokens
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: i64, // Unix timestamp
}
