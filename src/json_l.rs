use anyhow::Result;
use serde::{Serialize, de::DeserializeOwned};
use std::io::{BufRead, BufReader, BufWriter};
use std::io::{Read, Write};

/// Reads newline-separated JSON (with an internal [`BufReader`]) and
/// deserializes the JSON into items of type [`T`] to place them into an owned
/// vector, which is returned.
pub fn read_lines_buf<T: DeserializeOwned>(reader: impl Read) -> Result<Vec<T>> {
    let mut items = Vec::new();
    for line in BufReader::new(reader).lines() {
        items.push(serde_json::from_str::<T>(&line?)?);
    }

    Ok(items)
}

/// Serializes items to JSON and writes them (via an internal [`BufWriter`])
/// with a newline separator.
pub fn write_lines_buf<'a, T: Serialize + 'a>(
    writer: impl Write,
    mut items: impl Iterator<Item = &'a T>,
) -> Result<()> {
    let mut writer = BufWriter::new(writer);
    items.try_for_each(|item| {
        serde_json::to_writer(&mut writer, item)?;
        writer.write_all(b"\n")
    })?;
    writer.flush()?;
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
        write_lines_buf(&mut vec, items.iter()).unwrap();

        let items_from_file = read_lines_buf(vec.as_slice()).unwrap();
        assert_eq!(items, items_from_file);
    }
}
