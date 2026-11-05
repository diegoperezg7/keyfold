//! Where values live. Values go nowhere else.

use std::collections::HashMap;
use std::fmt;
use std::sync::Mutex;

/// Same service as the Swift version, so keys stored there keep working.
pub const DEFAULT_SERVICE: &str = "com.diego.llavero";

#[derive(Debug, Clone, PartialEq)]
pub struct StoreError(pub String);

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

pub trait SecretStore: Send + Sync {
    fn set(&self, name: &str, value: &str) -> Result<(), StoreError>;
    fn get(&self, name: &str) -> Result<Option<String>, StoreError>;
    /// Must not decrypt the value (no OS prompt).
    fn has_value(&self, name: &str) -> bool;
    fn delete(&self, name: &str) -> Result<(), StoreError>;
}

/// The credential store of the current OS.
pub fn system_store(service: &str) -> Box<dyn SecretStore> {
    #[cfg(target_os = "macos")]
    return Box::new(mac::KeychainStore { service: service.to_string() });
    #[cfg(windows)]
    return Box::new(win::CredentialStore { service: service.to_string() });
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = service;
        panic!("llavero: no credential store for this OS yet")
    }
}

/// For tests only: keeps values in memory.
#[derive(Default)]
pub struct MemoryStore {
    values: Mutex<HashMap<String, String>>,
}

impl SecretStore for MemoryStore {
    fn set(&self, name: &str, value: &str) -> Result<(), StoreError> {
        self.values.lock().unwrap().insert(name.into(), value.into());
        Ok(())
    }
    fn get(&self, name: &str) -> Result<Option<String>, StoreError> {
        Ok(self.values.lock().unwrap().get(name).cloned())
    }
    fn has_value(&self, name: &str) -> bool {
        self.values.lock().unwrap().contains_key(name)
    }
    fn delete(&self, name: &str) -> Result<(), StoreError> {
        self.values.lock().unwrap().remove(name);
        Ok(())
    }
}

#[cfg(target_os = "macos")]
mod mac {
    use super::{SecretStore, StoreError};
    use security_framework::item::{ItemClass, ItemSearchOptions, Limit};
    use security_framework::passwords::{delete_generic_password, get_generic_password, set_generic_password};

    const ERR_NOT_FOUND: i32 = -25300; // errSecItemNotFound

    /// Login Keychain, encrypted at rest by macOS. Service = app id, account = secret name.
    pub struct KeychainStore {
        pub service: String,
    }

    fn error(e: security_framework::base::Error) -> StoreError {
        StoreError(format!("Keychain error {}: {}", e.code(), e.message().unwrap_or_default()))
    }

    impl SecretStore for KeychainStore {
        fn set(&self, name: &str, value: &str) -> Result<(), StoreError> {
            set_generic_password(&self.service, name, value.as_bytes()).map_err(error)
        }

        fn get(&self, name: &str) -> Result<Option<String>, StoreError> {
            match get_generic_password(&self.service, name) {
                Ok(bytes) => Ok(Some(String::from_utf8_lossy(&bytes).into_owned())),
                Err(e) if e.code() == ERR_NOT_FOUND => Ok(None),
                Err(e) => Err(error(e)),
            }
        }

        fn has_value(&self, name: &str) -> bool {
            ItemSearchOptions::new()
                .class(ItemClass::generic_password())
                .service(&self.service)
                .account(name)
                .load_attributes(true)
                .limit(Limit::Max(1))
                .search()
                .map(|items| !items.is_empty())
                .unwrap_or(false)
        }

        fn delete(&self, name: &str) -> Result<(), StoreError> {
            match delete_generic_password(&self.service, name) {
                Ok(()) => Ok(()),
                Err(e) if e.code() == ERR_NOT_FOUND => Ok(()),
                Err(e) => Err(error(e)),
            }
        }
    }
}

#[cfg(windows)]
mod win {
    use super::{SecretStore, StoreError};
    use std::ptr::null_mut;
    use windows_sys::Win32::Foundation::{GetLastError, ERROR_NOT_FOUND};
    use windows_sys::Win32::Security::Credentials::{
        CredDeleteW, CredFree, CredReadW, CredWriteW, CREDENTIALW, CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC,
    };

    /// Windows Credential Manager (DPAPI-encrypted per user). Target = "service:name".
    pub struct CredentialStore {
        pub service: String,
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    fn last_error() -> StoreError {
        StoreError(format!("Credential Manager error {}", unsafe { GetLastError() }))
    }

    impl CredentialStore {
        fn target(&self, name: &str) -> Vec<u16> {
            wide(&format!("{}:{}", self.service, name))
        }
    }

    impl SecretStore for CredentialStore {
        fn set(&self, name: &str, value: &str) -> Result<(), StoreError> {
            let mut target = self.target(name);
            let mut user = wide(name);
            let blob = value.as_bytes();
            let mut credential: CREDENTIALW = unsafe { std::mem::zeroed() };
            credential.Type = CRED_TYPE_GENERIC;
            credential.TargetName = target.as_mut_ptr();
            credential.UserName = user.as_mut_ptr();
            credential.CredentialBlobSize = blob.len() as u32;
            credential.CredentialBlob = blob.as_ptr() as *mut u8;
            credential.Persist = CRED_PERSIST_LOCAL_MACHINE;
            if unsafe { CredWriteW(&credential, 0) } == 0 {
                return Err(last_error());
            }
            Ok(())
        }

        fn get(&self, name: &str) -> Result<Option<String>, StoreError> {
            let target = self.target(name);
            let mut credential: *mut CREDENTIALW = null_mut();
            if unsafe { CredReadW(target.as_ptr(), CRED_TYPE_GENERIC, 0, &mut credential) } == 0 {
                return if unsafe { GetLastError() } == ERROR_NOT_FOUND { Ok(None) } else { Err(last_error()) };
            }
            let value = unsafe {
                let c = &*credential;
                let bytes = std::slice::from_raw_parts(c.CredentialBlob, c.CredentialBlobSize as usize);
                let value = String::from_utf8_lossy(bytes).into_owned();
                CredFree(credential as *const _);
                value
            };
            Ok(Some(value))
        }

        fn has_value(&self, name: &str) -> bool {
            let target = self.target(name);
            let mut credential: *mut CREDENTIALW = null_mut();
            if unsafe { CredReadW(target.as_ptr(), CRED_TYPE_GENERIC, 0, &mut credential) } == 0 {
                return false;
            }
            unsafe { CredFree(credential as *const _) };
            true
        }

        fn delete(&self, name: &str) -> Result<(), StoreError> {
            let target = self.target(name);
            if unsafe { CredDeleteW(target.as_ptr(), CRED_TYPE_GENERIC, 0) } == 0 && unsafe { GetLastError() } != ERROR_NOT_FOUND {
                return Err(last_error());
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real OS store with a throwaway service; same process creates and reads, so no prompt.
    #[cfg(any(target_os = "macos", windows))]
    #[test]
    fn system_store_round_trip() {
        let service = format!("com.diego.llavero.tests.core-{}", std::process::id());
        let store = system_store(&service);
        assert!(!store.has_value("CORE_TEST_KEY"));
        assert_eq!(store.get("CORE_TEST_KEY").unwrap(), None);
        store.set("CORE_TEST_KEY", "first").unwrap();
        store.set("CORE_TEST_KEY", "second").unwrap(); // update path
        assert!(store.has_value("CORE_TEST_KEY"));
        assert_eq!(store.get("CORE_TEST_KEY").unwrap().as_deref(), Some("second"));
        store.delete("CORE_TEST_KEY").unwrap();
        store.delete("CORE_TEST_KEY").unwrap(); // deleting a missing key is fine
        assert!(!store.has_value("CORE_TEST_KEY"));
    }
}
