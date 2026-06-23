use std::sync::atomic::Ordering;

use serde::{Deserialize, Serialize};

/// A string that zeroizes its heap memory on drop.
/// Uses `ptr::write_volatile` to prevent the compiler from
/// optimizing away the clearing (unlike `mem::drop` which
/// just deallocates). Not a full substitute for `mlock()`
/// or the `zeroize` crate, but significantly better than bare `String`.
#[derive(Clone)]
pub struct SecretString {
    data: String,
}

impl SecretString {
    pub fn new(s: String) -> Self {
        Self { data: s }
    }

    pub fn as_str(&self) -> &str {
        &self.data
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn clone_inner(&self) -> String {
        self.data.clone()
    }

    pub fn into_inner(self) -> String {
        let s = self.data.clone();
        // self drops here, zeroizing the original
        s
    }
}

impl Drop for SecretString {
    fn drop(&mut self) {
        let bytes = unsafe { self.data.as_mut_vec() };
        for byte in bytes.iter_mut() {
            unsafe { std::ptr::write_volatile(byte, 0) };
        }
        std::sync::atomic::compiler_fence(Ordering::SeqCst);
    }
}

impl std::fmt::Debug for SecretString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SecretString")
            .field("data", &"[REDACTED]")
            .finish()
    }
}

impl Serialize for SecretString {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.data)
    }
}

impl<'de> Deserialize<'de> for SecretString {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(Self::new(String::deserialize(d)?))
    }
}
