use anyhow::Result;
use serde::{Serialize, de::DeserializeOwned};
use std::io::Write;
use std::path::Path;
use std::{
    fs::File,
    io::{BufRead, BufReader},
};

// TODO: it feels like this JSON L stuff should live with other collection management in a
// collection module that maybe handles downloading?
pub fn read_lines<T: DeserializeOwned>(file_path: &Path) -> Result<Vec<T>> {
    let file = File::open(file_path)?;
    let collection: Vec<T> = BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .map(|line| serde_json::from_str::<T>(&line).unwrap())
        .collect();

    Ok(collection)
}

pub fn write_lines<'a, T: Serialize + 'a>(file_path: &Path, items: impl Iterator<Item = &'a T>) {
    let mut file = File::create(file_path).expect("Error creating file handle");
    items
        .map(|item| serde_json::to_string(item).unwrap())
        .for_each(|item| {
            file.write_all(format!("{item}\n").as_bytes())
                .expect("Error writing to file")
        });
}

#[cfg(test)]
mod tests {
    use std::{fs::remove_file, path::PathBuf, str::FromStr};

    use serde::Deserialize;

    use super::*;

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct TestStruct {
        field: String,
    }

    #[test]
    fn read_lines_can_read_file_created_by_write_lines() {
        let file_path = PathBuf::from_str("test.jsonl").unwrap();
        let _ = remove_file(&file_path);

        let items = vec![
            TestStruct {
                field: "struct 1".to_owned(),
            },
            TestStruct {
                field: "struct 2".to_owned(),
            },
        ];

        write_lines(&file_path, items.iter());

        let items_from_file: Vec<TestStruct> = read_lines(&file_path).expect("read_lines failed");

        assert_eq!(items, items_from_file);
    }
}
