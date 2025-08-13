pub mod sound {
    use std::{fs::File, thread::sleep};

    use rodio::{Decoder, OutputStreamBuilder};

    pub fn play_sample_sound() {
        let stream_handle =
            OutputStreamBuilder::open_default_stream().expect("Error opening default audio stream");
        // TODO: What is this doing?
        // Note that playback stops when the sink is dropped
        let _sink = rodio::Sink::connect_new(stream_handle.mixer());
        let path = "file_example_MP3_2MG.mp3";
        let file = File::open(path).expect("Error opening file");
        let source = Decoder::try_from(file).expect("Error decoding file");
        // Play the sound directly on the device
        stream_handle.mixer().add(source);

        sleep(std::time::Duration::from_secs(5));
    }
}

pub mod crypto {
    use hmac::{Hmac, Mac};
    use sha1::Sha1;

    pub fn sha1_hex(key: &str, input: &str) -> String {
        // Copied from https://crates.io/crates/hmac-sha1
        // That crate recommended just using the sha1 and hmac crates directly like this:
        let mut hasher: Hmac<Sha1> =
            Mac::new_from_slice(key.as_bytes()).expect("HMAC algoritms can take keys of any size");
        hasher.update(input.as_bytes());
        let hmac: [u8; 20] = hasher.finalize().into_bytes().into();

        hex::encode(hmac)
    }

    #[cfg(test)]
    mod tests {
        use super::*; // Import items from the parent module

        #[test]
        fn test_sha1_hex() {
            let key = "dtmfa";
            let input = "grant_type=password&username=&password=&client_id=134&client_secret=1myK12VeCL3dWl9o%2FncV2VyUUbOJuNPVJK6bZZJxHvk%3D";

            let hash = sha1_hex(key, input);

            assert_eq!(hash, "09a83c762449f224f79ee514b9f3202b5798de10");
        }
    }
}
