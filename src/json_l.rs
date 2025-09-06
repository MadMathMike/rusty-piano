use anyhow::Result;
use serde::{Serialize, de::DeserializeOwned};
use std::io::Write;
use std::{
    fs::File,
    io::{BufRead, BufReader},
};

pub fn read_lines_from_file<T: DeserializeOwned>(file: File) -> Vec<T> {
    BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .map(|line| serde_json::from_str::<T>(&line))
        .map_while(Result::ok)
        .collect()
}

pub fn write_lines_to_file<'a, T: Serialize + 'a>(
    mut file: File,
    items: impl Iterator<Item = &'a T>,
) {
    items
        .map(serde_json::to_string)
        .map_while(Result::ok)
        .map(|item| format!("{}\n", item))
        .for_each(|line| file.write_all(line.as_bytes()).unwrap());
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

        write_lines_to_file(File::create(&file_path).unwrap(), items.iter());

        let items_from_file = read_lines_from_file(File::open(&file_path).unwrap());

        assert_eq!(items, items_from_file);
    }
}
