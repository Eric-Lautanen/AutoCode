// Crate-agnostic persistence trait.
// Replaces direct eframe::Storage usage so core doesn't depend on eframe.

/// Load-only persistence (used for deserialization).
/// Separate from `AppStorage` because eframe gives us an immutable `&dyn Storage`
/// for loading but requires `&mut dyn Storage` for saving.
pub trait StorageLoad {
    fn get<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T>;
}

/// Full read-write persistence.
pub trait AppStorage: StorageLoad {
    fn set<T: serde::Serialize>(&mut self, key: &str, value: &T);
}
