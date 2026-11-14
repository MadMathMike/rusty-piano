use anyhow::Result;
use serde::{Serialize, de::DeserializeOwned};
use std::io::{BufRead, BufReader};
use std::io::{Read, Write};

pub fn read_lines<T: DeserializeOwned>(reader: impl Read) -> Result<Vec<T>> {
    let mut items = Vec::new();
    for line in BufReader::new(reader).lines() {
        items.push(serde_json::from_str::<T>(&line?)?);
    }

    Ok(items)
}

pub fn write_lines<'a, T: Serialize + 'a>(
    mut writer: impl Write,
    mut items: impl Iterator<Item = &'a T>,
) -> Result<()> {
    items.try_for_each(|item| {
        let serialized_item = serde_json::to_string(item)?;
        let line = format!("{}\n", serialized_item);
        writer.write_all(line.as_bytes())
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct TestStruct {
        field: String,
    }

    #[test]
    fn read_lines_can_read_file_created_by_write_lines() {
        let items = vec![
            TestStruct {
                field: "struct 1".to_owned(),
            },
            TestStruct {
                field: "struct 2".to_owned(),
            },
        ];

        let mut vec: Vec<u8> = Vec::new();
        write_lines(&mut vec, items.iter()).unwrap();

        let items_from_file = read_lines(vec.as_slice()).unwrap();
        assert_eq!(items, items_from_file);
    }
}
