use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileAccessLog {
    pub entries: HashMap<String, AccessEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccessEntry {
    pub last_turn: u64,
    pub first_turn: u64,
    pub access_count: u64,
    pub ops: u8,
}

impl FileAccessLog {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub fn record(&mut self, path: &str, op: FileOp, turn: u64) {
        let entry = self.entries.entry(path.to_string()).or_insert(AccessEntry {
            last_turn: turn,
            first_turn: turn,
            access_count: 0,
            ops: 0,
        });
        entry.last_turn = entry.last_turn.max(turn);
        entry.first_turn = entry.first_turn.min(turn);
        entry.access_count = entry.access_count.saturating_add(1);
        entry.ops |= op as u8;
    }

    pub fn active_working_set(&self, current_turn: u64, within_turns: u64) -> HashSet<&str> {
        let threshold = current_turn.saturating_sub(within_turns);
        self.entries
            .iter()
            .filter(|(_, e)| e.last_turn >= threshold)
            .map(|(p, _)| p.as_str())
            .collect()
    }
}

impl Default for FileAccessLog {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileOp {
    Read = 1,
    Edit = 2,
    Grep = 4,
    Glob = 8,
    Search = 16,
}

pub fn tool_name_to_op(tool_name: &str) -> Option<FileOp> {
    match tool_name {
        "read_file" | "read_entire_file" | "read_files" => Some(FileOp::Read),
        "write_file" | "patch_file" | "patch_lines" | "delete_file" | "rename_file"
        | "create_dir" => Some(FileOp::Edit),
        "grep" => Some(FileOp::Grep),
        "glob" => Some(FileOp::Glob),
        "list_dir" | "project_tree" => Some(FileOp::Search),
        _ => None,
    }
}
