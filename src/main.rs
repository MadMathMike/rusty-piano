use std::thread::sleep;
use std::fs::File;
use rodio::{Decoder, OutputStreamBuilder};

fn main() {
    let stream_handle = OutputStreamBuilder::open_default_stream()
        .expect("Error opening default audio stream");
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
