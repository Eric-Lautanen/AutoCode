pub mod app_storage;
pub mod discovery;
pub mod messages;
pub mod persistence;
pub mod provider_file;
pub mod session_io;
pub mod session_meta;
pub mod shell_task;

// Re-export all public items so existing call sites continue to work.
pub use app_storage::{AppStorage, StorageLoad};
pub use discovery::{
    discover_projects_from_disk, discover_sessions_from_disk, load_project_identity,
    load_project_meta, project_meta_path, save_project_identity, save_project_meta,
    switch_to_project,
};
pub use messages::{
    append_messages, has_messages_file, read_all_messages, remove_messages_by_id, truncate_messages,
};
pub use persistence::{PanicInfo, PersistenceCommand, PersistenceThread};
pub use provider_file::{
    ModelEntry, ProviderEntry, ProviderFile, load_providers_file, save_providers_file,
};
pub use session_io::{
    append_messages_to_jsonl, delete_session_file, ensure_project_dirs, load_all_messages,
    load_messages_before, load_session, project_sessions_dir, remove_messages_after, save_session,
    save_session_meta, session_exists, session_messages_dir, truncate_messages_after,
};
pub use session_meta::SessionMeta;
pub use shell_task::{delete_task, list_tasks, load_task, prune_tasks, save_task};
