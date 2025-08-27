use anyhow::Result;
use serde::{Serialize, de::DeserializeOwned};
use std::io::Write;
use std::{
    fs::File,
    io::{BufRead, BufReader},
};

pub fn read_lines<T: DeserializeOwned>(file_path: &str) -> Result<Vec<T>> {
    let file = File::open(file_path)?;
    let collection: Vec<T> = BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .map(|line| serde_json::from_str::<T>(&line).unwrap())
        .collect();

    Ok(collection)
}

// TODO: change variable type of items from slice to iter?
pub fn write_lines<T: Serialize>(file_path: &str, items: &[T]) {
    let mut file = File::create(file_path).expect("Error creating file handle");
    for line in items.iter().map(|i| serde_json::to_string(i).unwrap()) {
        file.write_all(line.as_bytes())
            .expect("Error writing to file");
        file.write_all("\n".as_bytes())
            .expect("Error writing to file");
    }
}

// TODO: add a unit test to verify we can read out what we wrote
