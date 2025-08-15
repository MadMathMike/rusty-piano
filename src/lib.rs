pub mod bandcamp;
pub mod crypto;
pub mod secrets;

pub mod sound {
    use std::{fs::File, thread::sleep};

    use rodio::{Decoder, OutputStreamBuilder};

    pub fn play_source_sample(file: File) {
        let stream_handle =
            OutputStreamBuilder::open_default_stream().expect("Error opening default audio stream");
        // TODO: What is this doing?
        // Note that playback stops when the sink is dropped
        let _sink = rodio::Sink::connect_new(stream_handle.mixer());

        let source = Decoder::try_from(file).expect("Error decoding file");
        // Play the sound directly on the device
        stream_handle.mixer().add(source);

        sleep(std::time::Duration::from_secs(5));
    }
}
