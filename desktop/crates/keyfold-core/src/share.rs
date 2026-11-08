//! Sharing keys between people and between one person's computers, never in clear.
//!
//! Each install has an identity: an age X25519 key pair. The private half lives in the OS
//! credential store next to the values and never leaves this machine; the public half
//! (`age1…`) is what others encrypt to. A share package is an age file (ASCII armor)
//! encrypted to one or more public keys: names, projects and values inside, readable only
//! by the holders of those identities. Values are in clear only in memory.

use crate::keyfold::{validate_name, validate_project, Keyfold, KeyfoldError, SlotInfo};
use age::armor::{ArmoredReader, ArmoredWriter, Format};
use age::secrecy::ExposeSecret;
use age::x25519;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};

/// Credential-store account of this install's identity. Lowercase and a dot, so it can never
/// collide with a key name (`UPPER_SNAKE_CASE`).
const IDENTITY_ACCOUNT: &str = "keyfold.identity";
const FORMAT: &str = "keyfold-share-1";

#[derive(Serialize, Deserialize)]
struct Package {
    format: String,
    keys: Vec<SharedKey>,
}

#[derive(Serialize, Deserialize)]
struct SharedKey {
    name: String,
    value: String,
    projects: Vec<String>,
}

impl Keyfold {
    /// This install's public key (`age1…`), created on first use. Safe to show and send.
    pub fn public_key(&self) -> Result<String, KeyfoldError> {
        Ok(self.identity()?.to_public().to_string())
    }

    /// Encrypts `names` (with their projects) to `recipients`. Keys without a value are skipped.
    /// The caller must have confirmed the user is present (Touch ID): this reads values.
    pub fn share(&self, names: &[String], recipients: &[String]) -> Result<String, KeyfoldError> {
        let recipients = recipients
            .iter()
            .map(|r| r.trim().parse::<x25519::Recipient>().map_err(|_| KeyfoldError::InvalidRecipient(r.trim().into())))
            .collect::<Result<Vec<_>, _>>()?;
        if recipients.is_empty() {
            return Err(KeyfoldError::InvalidRecipient(String::new()));
        }
        let mut keys = Vec::new();
        for name in names {
            let info = self.slot(name)?;
            if let Some(value) = self.reveal(name)? {
                keys.push(SharedKey { name: info.name, value, projects: info.projects });
            }
        }
        if keys.is_empty() {
            return Err(KeyfoldError::NothingToShare);
        }
        let plain = serde_json::to_vec(&Package { format: FORMAT.into(), keys }).map_err(|e| KeyfoldError::Io(e.to_string()))?;
        encrypt(&plain, &recipients)
    }

    /// Every key that has a value, for moving them all to another of your computers.
    pub fn share_all(&self, recipients: &[String]) -> Result<String, KeyfoldError> {
        let names: Vec<String> = self.list(None)?.into_iter().filter(|k| k.has_value).map(|k| k.name).collect();
        self.share(&names, recipients)
    }

    /// Opens a package made for this install and stores its keys and projects. A key that
    /// already exists gets the new value. Returns the imported keys.
    pub fn import(&self, package: &str) -> Result<Vec<SlotInfo>, KeyfoldError> {
        let plain = decrypt(package, &self.identity()?)?;
        let package: Package = serde_json::from_slice(&plain).map_err(|_| KeyfoldError::InvalidPackage)?;
        if package.format != FORMAT {
            return Err(KeyfoldError::InvalidPackage);
        }
        // Check every key before storing any, so a bad entry never leaves half a package behind.
        for key in &package.keys {
            validate_name(&key.name)?;
            if key.value.is_empty() {
                return Err(KeyfoldError::EmptyValue);
            }
            for project in &key.projects {
                validate_project(project)?;
            }
        }
        let mut imported = Vec::new();
        for key in package.keys {
            self.create_slot(&key.name, None)?;
            self.set_value(&key.name, &key.value)?;
            for project in &key.projects {
                self.assign(&key.name, project)?;
            }
            imported.push(self.slot(&key.name)?);
        }
        Ok(imported)
    }

    fn identity(&self) -> Result<x25519::Identity, KeyfoldError> {
        if let Some(stored) = self.store.get(IDENTITY_ACCOUNT)? {
            return stored.parse().map_err(|_| KeyfoldError::Store("Keyfold identity is damaged".into()));
        }
        let identity = x25519::Identity::generate();
        self.store.set(IDENTITY_ACCOUNT, identity.to_string().expose_secret())?;
        Ok(identity)
    }
}

fn encrypt(plain: &[u8], recipients: &[x25519::Recipient]) -> Result<String, KeyfoldError> {
    let io = |e: std::io::Error| KeyfoldError::Io(e.to_string());
    let encryptor = age::Encryptor::with_recipients(recipients.iter().map(|r| r as &dyn age::Recipient))
        .map_err(|e| KeyfoldError::Io(e.to_string()))?;
    let mut out = Vec::new();
    let armor = ArmoredWriter::wrap_output(&mut out, Format::AsciiArmor).map_err(io)?;
    let mut writer = encryptor.wrap_output(armor).map_err(io)?;
    writer.write_all(plain).map_err(io)?;
    writer.finish().and_then(|armor| armor.finish()).map_err(io)?;
    String::from_utf8(out).map_err(|e| KeyfoldError::Io(e.to_string()))
}

fn decrypt(package: &str, identity: &x25519::Identity) -> Result<Vec<u8>, KeyfoldError> {
    let decryptor = age::Decryptor::new(ArmoredReader::new(package.trim().as_bytes())).map_err(|_| KeyfoldError::InvalidPackage)?;
    let mut reader = decryptor.decrypt(std::iter::once(identity as &dyn age::Identity)).map_err(|e| match e {
        age::DecryptError::NoMatchingKeys => KeyfoldError::NotForThisDevice,
        _ => KeyfoldError::InvalidPackage,
    })?;
    let mut plain = Vec::new();
    reader.read_to_end(&mut plain).map_err(|_| KeyfoldError::InvalidPackage)?;
    Ok(plain)
}

#[cfg(test)]
mod tests {
    use crate::registry::RegistryStore;
    use crate::store::{MemoryStore, SecretStore};
    use crate::{Keyfold, KeyfoldError};
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    const VALUE: &str = "sk-live-9fK2mQ7xVb4nR1tY6wL3pZ8cH5jD0sAe"; // gitleaks:allow (made-up test value)

    /// A Keyfold with its own registry folder, and a handle on its in-memory credential store.
    fn device(tag: &str) -> (Keyfold, Arc<MemoryStore>, PathBuf) {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("keyfold-share-{tag}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = Arc::new(MemoryStore::default());
        (Keyfold::new(Box::new(Shared(store.clone())), RegistryStore::new(&dir)), store, dir)
    }

    /// Lets a test look inside the store a Keyfold owns.
    struct Shared(Arc<MemoryStore>);
    impl SecretStore for Shared {
        fn set(&self, n: &str, v: &str) -> Result<(), crate::store::StoreError> {
            self.0.set(n, v)
        }
        fn get(&self, n: &str) -> Result<Option<String>, crate::store::StoreError> {
            self.0.get(n)
        }
        fn has_value(&self, n: &str) -> Result<bool, crate::store::StoreError> {
            self.0.has_value(n)
        }
        fn delete(&self, n: &str) -> Result<(), crate::store::StoreError> {
            self.0.delete(n)
        }
    }

    fn files_contain(dir: &Path, needle: &str) -> bool {
        let Ok(entries) = std::fs::read_dir(dir) else { return false };
        entries.flatten().any(|e| {
            let p = e.path();
            if p.is_dir() {
                files_contain(&p, needle)
            } else {
                std::fs::read(&p).is_ok_and(|bytes| bytes.windows(needle.len()).any(|w| w == needle.as_bytes()))
            }
        })
    }

    #[test]
    fn a_shared_key_opens_only_for_the_intended_recipient() {
        let (alice, _, a_dir) = device("alice");
        let (bob, _, b_dir) = device("bob");
        let (eve, _, e_dir) = device("eve");
        alice.create_slot("STRIPE_KEY", Some("shop")).unwrap();
        alice.set_value("STRIPE_KEY", VALUE).unwrap();

        let package = alice.share(&["STRIPE_KEY".into()], &[bob.public_key().unwrap()]).unwrap();

        let imported = bob.import(&package).unwrap();
        assert_eq!(imported.len(), 1);
        assert_eq!(imported[0].projects, vec!["shop"]);
        assert_eq!(bob.reveal("STRIPE_KEY").unwrap().as_deref(), Some(VALUE));

        // Someone else with the package, and the sender, cannot open it.
        assert_eq!(eve.import(&package), Err(KeyfoldError::NotForThisDevice));
        assert!(eve.list(None).unwrap().is_empty());
        assert_eq!(alice.import(&package), Err(KeyfoldError::NotForThisDevice));

        // A package for two of your computers opens on both.
        let (laptop, _, l_dir) = device("laptop");
        let all = alice.share_all(&[bob.public_key().unwrap(), laptop.public_key().unwrap()]).unwrap();
        assert_eq!(laptop.import(&all).unwrap()[0].name, "STRIPE_KEY");
        assert_eq!(laptop.reveal("STRIPE_KEY").unwrap().as_deref(), Some(VALUE));

        assert!(matches!(alice.share(&["STRIPE_KEY".into()], &["not-a-key".into()]), Err(KeyfoldError::InvalidRecipient(_))));
        assert_eq!(bob.import("-----BEGIN AGE ENCRYPTED FILE-----\nnope\n-----END AGE ENCRYPTED FILE-----"), Err(KeyfoldError::InvalidPackage));
        for d in [a_dir, b_dir, e_dir, l_dir] {
            let _ = std::fs::remove_dir_all(d);
        }
    }

    #[test]
    fn a_package_with_one_bad_entry_imports_nothing() {
        let (bob, bob_store, b_dir) = device("bob-bad");
        let recipient: age::x25519::Recipient = bob.public_key().unwrap().parse().unwrap();
        for (name, project) in [("bad name", "shop"), ("GOOD_TWO", "bad/project")] {
            let plain = serde_json::to_vec(&super::Package {
                format: super::FORMAT.into(),
                keys: vec![
                    super::SharedKey { name: "GOOD_KEY".into(), value: VALUE.into(), projects: vec!["shop".into()] },
                    super::SharedKey { name: name.into(), value: VALUE.into(), projects: vec![project.into()] },
                ],
            })
            .unwrap();
            let package = super::encrypt(&plain, &[recipient.clone()]).unwrap();
            assert!(matches!(bob.import(&package), Err(KeyfoldError::InvalidName(_) | KeyfoldError::InvalidProject(_))));
            assert!(bob.list(None).unwrap().is_empty(), "nothing may be stored from a bad package");
            assert_eq!(bob_store.get("GOOD_KEY").unwrap(), None);
        }
        let _ = std::fs::remove_dir_all(b_dir);
    }

    #[test]
    fn the_value_is_never_in_clear_in_a_package_or_a_file() {
        let (alice, alice_store, a_dir) = device("alice-clear");
        let (bob, bob_store, b_dir) = device("bob-clear");
        alice.create_slot("STRIPE_KEY", Some("shop")).unwrap();
        alice.set_value("STRIPE_KEY", VALUE).unwrap();

        let package = alice.share_all(&[bob.public_key().unwrap()]).unwrap();
        // What travels (chat, AirDrop, a shared folder): armored ciphertext, no name, no value.
        assert!(package.starts_with("-----BEGIN AGE ENCRYPTED FILE-----"));
        assert!(!package.contains(VALUE) && !package.contains("STRIPE_KEY"));
        // Even the raw bytes under the armor do not hold it.
        let package_file = a_dir.join("outbox.age");
        std::fs::create_dir_all(&a_dir).unwrap();
        std::fs::write(&package_file, &package).unwrap();
        bob.import(&std::fs::read_to_string(&package_file).unwrap()).unwrap();

        // Neither registry folder (or anything written next to it) holds the value or an identity.
        for dir in [&a_dir, &b_dir] {
            assert!(!files_contain(dir, VALUE), "value in clear under {}", dir.display());
            assert!(!files_contain(dir, "AGE-SECRET-KEY-"), "identity in clear under {}", dir.display());
        }
        // Values and identities only live in each credential store.
        assert_eq!(bob_store.get("STRIPE_KEY").unwrap().as_deref(), Some(VALUE));
        assert!(bob_store.get("keyfold.identity").unwrap().unwrap().starts_with("AGE-SECRET-KEY-"));
        assert!(alice_store.get("keyfold.identity").unwrap().is_none(), "sharing needs no identity of your own");
        for d in [a_dir, b_dir] {
            let _ = std::fs::remove_dir_all(d);
        }
    }
}
