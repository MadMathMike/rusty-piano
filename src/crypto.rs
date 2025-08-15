use hmac::{Hmac, Mac};
use sha1::Sha1;

pub fn hmac_sha1_as_hex(key: &str, input: &str) -> String {
    // The hmac-sha1 crate recommended using the sha1 and hmac crates directly:
    let mut hasher: Hmac<Sha1> =
        Mac::new_from_slice(key.as_bytes()).expect("HMAC algoritms can take keys of any size");
    hasher.update(input.as_bytes());
    let hmac: [u8; 20] = hasher.finalize().into_bytes().into();

    hex::encode(hmac)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha1_hex() {
        let key = "dtmfa";
        let input = "grant_type=password&username=&password=&client_id=134&client_secret=1myK12VeCL3dWl9o%2FncV2VyUUbOJuNPVJK6bZZJxHvk%3D";

        let hash = hmac_sha1_as_hex(key, input);

        assert_eq!(hash, "09a83c762449f224f79ee514b9f3202b5798de10");
    }
}
