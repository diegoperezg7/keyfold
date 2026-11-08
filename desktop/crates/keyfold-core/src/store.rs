//! Where values live. Values go nowhere else.

use std::collections::HashMap;
use std::fmt;
use std::sync::Mutex;

/// Keychain service of the original prototype ("Llavero"). Kept on purpose: changing it
/// would orphan every key already stored. It is internal and never shown to users.
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
    /// Must not decrypt the value (no OS prompt). An error from the store is an error, never
    /// "no value": callers like `generate` would otherwise overwrite a value they could not see.
    fn has_value(&self, name: &str) -> Result<bool, StoreError>;
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
        Box::new(UnsupportedStore)
    }
}

/// Stored values must be UTF-8 text (environment variables). Anything else is reported, never
/// silently altered.
fn utf8_value(name: &str, bytes: Vec<u8>) -> Result<String, StoreError> {
    String::from_utf8(bytes).map_err(|_| {
        StoreError(crate::l10n::t(
            &format!("El valor guardado de {name} no es texto UTF-8 y no se puede usar. Vuelve a guardarlo con: keyfold set {name}"),
            &format!("The stored value of {name} is not UTF-8 text and cannot be used. Store it again with: keyfold set {name}"),
        ))
    })
}

/// Every call fails with a clear message, on platforms with no credential store support yet.
#[cfg(not(any(target_os = "macos", windows)))]
struct UnsupportedStore;

#[cfg(not(any(target_os = "macos", windows)))]
impl UnsupportedStore {
    fn error() -> StoreError {
        StoreError(crate::l10n::t(
            "Keyfold aún no tiene almacén de credenciales para este sistema operativo (solo macOS y Windows).",
            "Keyfold has no credential store for this operating system yet (macOS and Windows only).",
        ))
    }
}

#[cfg(not(any(target_os = "macos", windows)))]
impl SecretStore for UnsupportedStore {
    fn set(&self, _name: &str, _value: &str) -> Result<(), StoreError> {
        Err(Self::error())
    }
    fn get(&self, _name: &str) -> Result<Option<String>, StoreError> {
        Err(Self::error())
    }
    fn has_value(&self, _name: &str) -> Result<bool, StoreError> {
        Err(Self::error())
    }
    fn delete(&self, _name: &str) -> Result<(), StoreError> {
        Err(Self::error())
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
    fn has_value(&self, name: &str) -> Result<bool, StoreError> {
        Ok(self.values.lock().unwrap().contains_key(name))
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
                Ok(bytes) => super::utf8_value(name, bytes).map(Some),
                Err(e) if e.code() == ERR_NOT_FOUND => Ok(None),
                Err(e) => Err(error(e)),
            }
        }

        fn has_value(&self, name: &str) -> Result<bool, StoreError> {
            match ItemSearchOptions::new()
                .class(ItemClass::generic_password())
                .service(&self.service)
                .account(name)
                .load_attributes(true)
                .limit(Limit::Max(1))
                .search()
            {
                Ok(items) => Ok(!items.is_empty()),
                Err(e) if e.code() == ERR_NOT_FOUND => Ok(false),
                Err(e) => Err(error(e)),
            }
        }

        fn delete(&self, name: &str) -> Result<(), StoreError> {
            match delete_generic_password(&self.service, name) {
                Ok(()) => Ok(()),
                Err(e) if e.code() == ERR_NOT_FOUND => Ok(()),
                // Saved by another build (e.g. an older unsigned one): the Keychain only lets that
                // binary delete it. Apple's `security` tool can, asking the user if macOS needs to.
                Err(e) if e.code() == ERR_INVALID_OWNER_EDIT => delete_with_security_tool(&self.service, name),
                Err(e) => Err(error(e)),
            }
        }
    }

    const ERR_INVALID_OWNER_EDIT: i32 = -25244; // errSecInvalidOwnerEdit
    const SECURITY_NOT_FOUND: i32 = 44; // `security` exit code when the item does not exist

    fn delete_with_security_tool(service: &str, name: &str) -> Result<(), StoreError> {
        // stdout lists the item's attributes (never its value); it is discarded all the same.
        let output = std::process::Command::new("/usr/bin/security")
            .args(["delete-generic-password", "-s", service, "-a", name])
            .stdout(std::process::Stdio::null())
            .output()
            .map_err(|e| StoreError(format!("security: {e}")))?;
        match output.status.code() {
            Some(0) | Some(SECURITY_NOT_FOUND) => Ok(()),
            _ => Err(StoreError(format!("security: {}", String::from_utf8_lossy(&output.stderr).trim()))),
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

    /// CRED_MAX_CREDENTIAL_BLOB_SIZE: the most a generic credential can hold.
    const MAX_BLOB_BYTES: usize = 5 * 512;

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
            if blob.len() > MAX_BLOB_BYTES {
                return Err(StoreError(crate::l10n::t(
                    &format!("El valor de {name} ocupa {} bytes y el Administrador de credenciales de Windows admite como máximo {MAX_BLOB_BYTES}.", blob.len()),
                    &format!("The value of {name} is {} bytes and Windows Credential Manager holds at most {MAX_BLOB_BYTES}.", blob.len()),
                )));
            }
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
            let bytes = unsafe {
                let c = &*credential;
                let bytes = std::slice::from_raw_parts(c.CredentialBlob, c.CredentialBlobSize as usize).to_vec();
                CredFree(credential as *const _);
                bytes
            };
            super::utf8_value(name, bytes).map(Some)
        }

        fn has_value(&self, name: &str) -> Result<bool, StoreError> {
            let target = self.target(name);
            let mut credential: *mut CREDENTIALW = null_mut();
            if unsafe { CredReadW(target.as_ptr(), CRED_TYPE_GENERIC, 0, &mut credential) } == 0 {
                return if unsafe { GetLastError() } == ERROR_NOT_FOUND { Ok(false) } else { Err(last_error()) };
            }
            unsafe { CredFree(credential as *const _) };
            Ok(true)
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

    #[test]
    fn a_value_that_is_not_utf8_is_an_error_not_a_lossy_copy() {
        assert_eq!(utf8_value("A_KEY", b"plain".to_vec()).unwrap(), "plain");
        let error = utf8_value("A_KEY", vec![b'a', 0xff, b'b']).unwrap_err();
        assert!(error.0.contains("A_KEY") && error.0.contains("UTF-8"), "{error}");
    }

    /// Real OS store with a throwaway service; same process creates and reads, so no prompt.
    #[cfg(any(target_os = "macos", windows))]
    #[test]
    fn system_store_round_trip() {
        let service = format!("com.diego.llavero.tests.core-{}", std::process::id());
        let store = system_store(&service);
        assert!(!store.has_value("CORE_TEST_KEY").unwrap());
        assert_eq!(store.get("CORE_TEST_KEY").unwrap(), None);
        store.set("CORE_TEST_KEY", "first").unwrap();
        store.set("CORE_TEST_KEY", "second").unwrap(); // update path
        assert!(store.has_value("CORE_TEST_KEY").unwrap());
        assert_eq!(store.get("CORE_TEST_KEY").unwrap().as_deref(), Some("second"));
        store.delete("CORE_TEST_KEY").unwrap();
        store.delete("CORE_TEST_KEY").unwrap(); // deleting a missing key is fine
        assert!(!store.has_value("CORE_TEST_KEY").unwrap());
    }
}
