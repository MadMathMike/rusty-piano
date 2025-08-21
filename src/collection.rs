use std::{
    fs::File,
    io::{BufRead, BufReader},
};

use crate::{app::authenticate_with_bandcamp, bandcamp::Item};

use std::io::Write;

pub fn read_collection() -> Vec<Item> {
    let file = File::open("collection.jsonl").expect("Should be able to open file");
    BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .map(|line| serde_json::from_str::<Item>(&line).unwrap())
        .collect()
}

pub fn cache_collection() {
    let client = authenticate_with_bandcamp();

    // TODO: move this to bandcamp module (probably) and support multiple page sizes
    let page_size = 5;
    let mut offset = String::new();
    let mut items: Vec<Item> = Vec::new();
    loop {
        let response = client.get_collection(page_size, &offset);
        let token = response
            .items
            .last()
            .map_or(String::new(), |i| i.token.clone());
        items.extend(response.items);
        if token.is_empty() {
            break;
        }
        offset = token;
    }

    let unique_count = items
        .iter()
        .map(|i| i.tralbum_id)
        .collect::<std::collections::HashSet<_>>()
        .len();
    assert_eq!(7, unique_count);

    let mut file = File::create("collection.jsonl").expect("Error creating file handle");
    for line in items.iter().map(|i| serde_json::to_string(i).unwrap()) {
        file.write_all(line.as_bytes())
            .expect("Error writing to file");
        file.write_all("\n".as_bytes())
            .expect("Error writing to file");
    }
}
