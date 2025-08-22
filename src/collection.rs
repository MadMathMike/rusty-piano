use std::{
    fs::File,
    io::{BufRead, BufReader},
};

use crate::bandcamp::{Item};

use std::io::Write;

pub fn read_collection() -> Vec<Item> {
    let file = File::open("collection.jsonl").expect("Should be able to open file");
    BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .map(|line| serde_json::from_str::<Item>(&line).unwrap())
        .collect()
}

pub fn write_collection(items: Vec<Item>) {
    let mut file = File::create("collection.jsonl").expect("Error creating file handle");
    for line in items.iter().map(|i| serde_json::to_string(i).unwrap()) {
        file.write_all(line.as_bytes())
            .expect("Error writing to file");
        file.write_all("\n".as_bytes())
            .expect("Error writing to file");
    }
}

// TODO: Add unit tests