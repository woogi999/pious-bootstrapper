//! Secure session storage backed by the operating system's credential store
//! (Windows Credential Manager, macOS Keychain, Secret Service on Linux).
//!
//! Pious never stores passwords. It only keeps the Roblox session
//! token that the official sign-in flow produces, and only in the OS vault.

use uuid::Uuid;

// The vault entry keeps its original name so existing sign-ins carry over.
const SERVICE: &str = "Pious Library";

fn entry(account: Uuid) -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, &format!("roblox-session:{account}")).map_err(|e| e.to_string())
}

pub fn store_session(account: Uuid, token: &str) -> Result<(), String> {
    entry(account)?
        .set_password(token)
        .map_err(|e| format!("Could not save the session to the system vault: {e}"))
}

pub fn load_session(account: Uuid) -> Result<String, String> {
    match entry(account)?.get_password() {
        Ok(token) => Ok(token),
        Err(keyring::Error::NoEntry) => {
            Err("This account isn't signed in. Sign in again from Accounts.".into())
        }
        Err(e) => Err(format!("Could not read the session from the system vault: {e}")),
    }
}

pub fn delete_session(account: Uuid) {
    if let Ok(entry) = entry(account) {
        let _ = entry.delete_credential();
    }
}
