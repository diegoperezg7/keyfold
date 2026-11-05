//! Unlock before a value is shown. The first reveal after the keys window opens asks for
//! Touch ID or the account password. The unlock ends when that window closes, when the app
//! quits, or at local midnight, whichever comes first. It lives only in this process's memory:
//! there is no file or flag another process (an agent) could write to skip it.
//!
//! macOS only for now. On Windows `authenticate` lets the reveal through (documented in
//! SECURITY.md as open).

use std::sync::Mutex;
use std::time::SystemTime;

#[derive(Default)]
pub struct SessionUnlock {
    unlocked_at: Mutex<Option<SystemTime>>,
}

impl SessionUnlock {
    /// Ok once the user is authenticated in this session and today. `authenticate` runs only
    /// when locked, and a failed or cancelled attempt leaves it locked.
    pub fn ensure(&self, is_today: impl Fn(SystemTime) -> bool, authenticate: impl FnOnce() -> bool) -> Result<(), Locked> {
        let mut unlocked_at = self.unlocked_at.lock().unwrap_or_else(|e| e.into_inner());
        if unlocked_at.is_some_and(&is_today) {
            return Ok(());
        }
        *unlocked_at = None;
        if authenticate() {
            *unlocked_at = Some(SystemTime::now());
            Ok(())
        } else {
            Err(Locked)
        }
    }

    pub fn lock(&self) {
        *self.unlocked_at.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

#[derive(Debug, PartialEq)]
pub struct Locked;

/// Same calendar day as now, in the user's calendar and time zone.
#[cfg(target_os = "macos")]
pub fn is_today(time: SystemTime) -> bool {
    use objc2_foundation::{NSCalendar, NSDate};
    let secs = time.duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0);
    NSCalendar::currentCalendar().isDateInToday(&NSDate::dateWithTimeIntervalSince1970(secs))
}

#[cfg(not(target_os = "macos"))]
pub fn is_today(time: SystemTime) -> bool {
    let day = |t: SystemTime| t.duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs() / 86_400).unwrap_or(0);
    day(time) == day(SystemTime::now())
}

/// Touch ID, Apple Watch or the account password, whatever the Mac offers. Blocks until the
/// user answers, so never call it on the main thread.
#[cfg(target_os = "macos")]
pub fn authenticate(reason: &str) -> bool {
    use block2::RcBlock;
    use objc2::runtime::Bool;
    use objc2_foundation::{NSError, NSString};
    use objc2_local_authentication::{LAContext, LAPolicy};
    use std::sync::mpsc;

    let (tx, rx) = mpsc::channel();
    let reply = RcBlock::new(move |ok: Bool, _error: *mut NSError| {
        let _ = tx.send(ok.as_bool());
    });
    // SAFETY: plain LocalAuthentication calls; the framework retains the reply block until it
    // runs, and we only wait on the channel.
    unsafe {
        let context = LAContext::new();
        let policy = LAPolicy::DeviceOwnerAuthentication;
        if context.canEvaluatePolicy_error(policy).is_err() {
            return false;
        }
        context.evaluatePolicy_localizedReason_reply(policy, &NSString::from_str(reason), &reply);
    }
    rx.recv().unwrap_or(false)
}

#[cfg(not(target_os = "macos"))]
pub fn authenticate(_reason: &str) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    const TODAY: fn(SystemTime) -> bool = |_| true;
    const YESTERDAY: fn(SystemTime) -> bool = |_| false;

    #[test]
    fn asks_once_per_session_and_again_after_lock() {
        let unlock = SessionUnlock::default();
        let asked = Cell::new(0);
        let ok = || {
            asked.set(asked.get() + 1);
            true
        };
        assert_eq!(unlock.ensure(TODAY, ok), Ok(()));
        assert_eq!(unlock.ensure(TODAY, ok), Ok(()));
        assert_eq!(asked.get(), 1, "the second reveal in the same session must not ask again");
        unlock.lock();
        assert_eq!(unlock.ensure(TODAY, ok), Ok(()));
        assert_eq!(asked.get(), 2, "closing the window locks it again");
    }

    #[test]
    fn asks_again_on_a_new_day() {
        let unlock = SessionUnlock::default();
        let asked = Cell::new(0);
        let ok = || {
            asked.set(asked.get() + 1);
            true
        };
        unlock.ensure(TODAY, ok).unwrap();
        unlock.ensure(YESTERDAY, ok).unwrap();
        assert_eq!(asked.get(), 2, "an unlock from another day does not count");
    }

    #[test]
    fn cancelled_authentication_stays_locked() {
        let unlock = SessionUnlock::default();
        assert_eq!(unlock.ensure(TODAY, || false), Err(Locked));
        let asked = Cell::new(false);
        let ok = || {
            asked.set(true);
            true
        };
        assert_eq!(unlock.ensure(TODAY, ok), Ok(()));
        assert!(asked.get(), "after a cancel, the next reveal asks again");
    }

    #[test]
    fn now_is_today() {
        assert!(is_today(SystemTime::now()));
        assert!(!is_today(SystemTime::now() - std::time::Duration::from_secs(3 * 86_400)));
    }
}
