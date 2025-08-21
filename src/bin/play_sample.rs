use rusty_piano::{app::authenticate_with_bandcamp, collection::read_collection, player::play_track};

fn main() {
    // TODO: Add option to bypass authentication to support offline-mode play

    // let client = authenticate_with_bandcamp();

    // let collection = client.get_collection(1, "");
    let collection = read_collection();

    println!("{collection:?}");

    // Download track to temp location
    let track = collection.first().unwrap().tracks.first().unwrap();
    play_track(track);
}
