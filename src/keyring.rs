use crate::error::CliError;

const SERVICE_NAME: &str = "granola-cli";
const DEFAULT_ACCOUNT: &str = "default";

pub fn get_key() -> Result<Option<String>, CliError> {
    let entry = keyring::Entry::new(SERVICE_NAME, DEFAULT_ACCOUNT)
        .map_err(|error| CliError::general(format!("failed to create keyring entry: {error}")))?;

    match entry.get_password() {
        Ok(password) => Ok(Some(password)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(CliError::general(format!(
            "failed to read API key from keyring: {error}"
        ))),
    }
}

pub fn set_key(api_key: &str) -> Result<(), CliError> {
    let entry = keyring::Entry::new(SERVICE_NAME, DEFAULT_ACCOUNT)
        .map_err(|error| CliError::general(format!("failed to create keyring entry: {error}")))?;

    entry.set_password(api_key).map_err(|error| {
        CliError::general(format!("failed to store API key in keyring: {error}"))
    })?;

    match get_key()? {
        Some(actual) if actual == api_key => Ok(()),
        Some(_) => Err(CliError::general(
            "API key was written to the keyring but could not be read back unchanged",
        )),
        None => Err(CliError::general(
            "API key could not be read back from the keyring after writing it",
        )),
    }
}

pub fn delete_key() -> Result<(), CliError> {
    let entry = keyring::Entry::new(SERVICE_NAME, DEFAULT_ACCOUNT)
        .map_err(|error| CliError::general(format!("failed to create keyring entry: {error}")))?;

    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(CliError::general(format!(
            "failed to delete API key from keyring: {error}"
        ))),
    }
}

pub fn is_available() -> bool {
    let account = format!("__granola_cli_probe_{}__", std::process::id());
    let secret = format!("probe-{}", std::process::id());

    let Ok(entry) = keyring::Entry::new(SERVICE_NAME, &account) else {
        return false;
    };
    if entry.set_password(&secret).is_err() {
        return false;
    }

    let verified = keyring::Entry::new(SERVICE_NAME, &account)
        .ok()
        .and_then(|entry| entry.get_password().ok())
        .map(|actual| actual == secret)
        .unwrap_or(false);
    let _ = entry.delete_credential();
    verified
}
