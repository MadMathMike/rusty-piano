use anyhow::Result;

const SERVICE: &str = "rusty-piano";

pub fn store(key: &'static str, secret: &str) -> Result<()> {
    // No check on incoming key string. Empty strings probably aren't valid,
    // but we don't need to expose a reasonable error.
    let entry = keyring::Entry::new(SERVICE, key)?;
    entry.set_secret(secret.as_bytes())?;
    Ok(())
}

pub fn retrieve(key: &'static str) -> Result<String> {
    let entry = keyring::Entry::new(SERVICE, key)?;
    let Ok(secret_bytes) = entry?;
    Ok(String::from_utf8(secret_bytes)?)
}
