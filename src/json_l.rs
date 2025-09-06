use anyhow::Result;
use serde::{Serialize, de::DeserializeOwned};
use std::io::Write;
use std::{
    fs::File,
    io::{BufRead, BufReader},
};

pub fn read_lines_from_file<T: DeserializeOwned>(file: File) -> Result<Vec<T>> {
    let mut items = Vec::new();
    for line in BufReader::new(file).lines() {
        items.push(serde_json::from_str::<T>(&line?)?);
    }

    Ok(items)
}

pub fn write_lines_to_file<'a, T: Serialize + 'a>(
    mut file: File,
    mut items: impl Iterator<Item = &'a T>,
) -> Result<()> {
    items.try_for_each(|item| {
        let serialized_item = serde_json::to_string(item)?;
        let line = format!("{}\n", serialized_item);
        file.write_all(line.as_bytes())
    })?;
    Ok(())
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

        write_lines_to_file(File::create(&file_path).unwrap(), items.iter()).unwrap();

        let items_from_file = read_lines_from_file(File::open(&file_path).unwrap()).unwrap();

        assert_eq!(items, items_from_file);
    }
}
