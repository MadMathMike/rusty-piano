use rusty_piano::{app::authenticate_with_bandcamp, collection::write_collection};

fn main() {
    let client = authenticate_with_bandcamp();

    let items = client.get_entire_collection(5);

    let unique_count = items
        .iter()
        .map(|i| i.tralbum_id)
        .collect::<std::collections::HashSet<_>>()
        .len();

    // Maybe this goes in an integration test? Requires authentication, so probably not
    assert_eq!(
        unique_count,
        items.len(),
        "There should not be any duplicates in the collection"
    );
    // assert_eq!(7, unique_count);

    write_collection(items);
}
