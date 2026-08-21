//! JSON Lines and single-document JSON readers/writers with LF-only line endings and precise
//! error locations.

use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde::de::DeserializeOwned;

use super::IoError;

/// Writes `items` to `path` as JSON Lines (one compact JSON document per line, LF-terminated).
///
/// Returns the number of items written.
pub fn write_jsonl<T: Serialize>(path: &Path, items: impl IntoIterator<Item = T>) -> Result<usize, IoError> {
    let file = File::create(path).map_err(|source| IoError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let mut w = BufWriter::new(file);
    let mut count = 0usize;
    for item in items {
        serde_json::to_writer(&mut w, &item).map_err(|source| IoError::Json {
            path: path.to_path_buf(),
            line: count + 1,
            source,
        })?;
        w.write_all(b"\n").map_err(|source| IoError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        count += 1;
    }
    w.flush().map_err(|source| IoError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(count)
}

/// Streaming JSONL writer: one compact JSON object per line, each terminated by `\n`.
/// Output is byte-identical to [`write_jsonl`] over the same items.
pub struct JsonlWriter {
    writer: BufWriter<File>,
    path: PathBuf,
    count: usize,
}

impl JsonlWriter {
    /// Creates (or truncates) `path` and opens it for streaming writes.
    pub fn create(path: &Path) -> Result<Self, IoError> {
        let file = File::create(path).map_err(|source| IoError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        Ok(Self {
            writer: BufWriter::new(file),
            path: path.to_path_buf(),
            count: 0,
        })
    }

    /// Appends one item as a compact JSON line.
    pub fn write<T: Serialize>(&mut self, item: &T) -> Result<(), IoError> {
        serde_json::to_writer(&mut self.writer, item).map_err(|source| IoError::Json {
            path: self.path.clone(),
            line: self.count + 1,
            source,
        })?;
        self.writer.write_all(b"\n").map_err(|source| IoError::Io {
            path: self.path.clone(),
            source,
        })?;
        self.count += 1;
        Ok(())
    }

    /// Number of items written so far.
    pub fn count(&self) -> usize {
        self.count
    }

    /// Flushes and returns the number of items written.
    pub fn finish(mut self) -> Result<usize, IoError> {
        self.writer.flush().map_err(|source| IoError::Io {
            path: self.path.clone(),
            source,
        })?;
        Ok(self.count)
    }
}

/// Streaming reader over a JSON Lines file, yielding one deserialized record per non-blank line.
pub struct JsonlReader<T> {
    path: PathBuf,
    lines: std::io::Lines<BufReader<File>>,
    line_no: usize,
    _marker: PhantomData<fn() -> T>,
}

impl<T: DeserializeOwned> JsonlReader<T> {
    /// Opens `path` for streaming JSONL reads.
    pub fn open(path: &Path) -> Result<Self, IoError> {
        let file = File::open(path).map_err(|source| IoError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        Ok(Self {
            path: path.to_path_buf(),
            lines: BufReader::new(file).lines(),
            line_no: 0,
            _marker: PhantomData,
        })
    }

    /// The file path this reader was opened from.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl<T: DeserializeOwned> Iterator for JsonlReader<T> {
    type Item = Result<T, IoError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let line = self.lines.next()?;
            self.line_no += 1;
            let line = match line {
                Ok(line) => line,
                Err(source) => {
                    return Some(Err(IoError::Io {
                        path: self.path.clone(),
                        source,
                    }));
                }
            };
            if line.trim().is_empty() {
                continue;
            }
            let line_no = self.line_no;
            return Some(serde_json::from_str(&line).map_err(|source| IoError::Json {
                path: self.path.clone(),
                line: line_no,
                source,
            }));
        }
    }
}

/// Reads all records from a JSONL file into a `Vec`.
pub fn read_jsonl<T: DeserializeOwned>(path: &Path) -> Result<Vec<T>, IoError> {
    JsonlReader::open(path)?.collect()
}

/// Writes `value` to `path` as pretty-printed JSON with a trailing LF.
pub fn write_json_pretty<T: Serialize>(path: &Path, value: &T) -> Result<(), IoError> {
    let mut text = serde_json::to_string_pretty(value).map_err(|source| IoError::Json {
        path: path.to_path_buf(),
        line: 0,
        source,
    })?;
    text.push('\n');
    std::fs::write(path, text).map_err(|source| IoError::Io {
        path: path.to_path_buf(),
        source,
    })
}

/// Reads a single JSON document from `path`.
pub fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T, IoError> {
    let text = std::fs::read_to_string(path).map_err(|source| IoError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&text).map_err(|source| IoError::Json {
        path: path.to_path_buf(),
        line: 0,
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
    struct Item {
        id: u32,
        name: String,
    }

    fn temp_path(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("strategy-discovery-{}-{}", std::process::id(), name));
        let _ = std::fs::remove_file(&path);
        path
    }

    #[test]
    fn jsonl_round_trips_and_uses_lf_only() {
        let path = temp_path("round-trip.jsonl");
        let count = write_jsonl(
            &path,
            vec![
                Item {
                    id: 1,
                    name: "a".to_string(),
                },
                Item {
                    id: 2,
                    name: "b".to_string(),
                },
                Item {
                    id: 3,
                    name: "c".to_string(),
                },
            ],
        )
        .unwrap();
        assert_eq!(count, 3);

        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(
            bytes,
            b"{\"id\":1,\"name\":\"a\"}\n{\"id\":2,\"name\":\"b\"}\n{\"id\":3,\"name\":\"c\"}\n"
        );

        let expected = vec![
            Item {
                id: 1,
                name: "a".to_string(),
            },
            Item {
                id: 2,
                name: "b".to_string(),
            },
            Item {
                id: 3,
                name: "c".to_string(),
            },
        ];
        let read_back: Vec<Item> = read_jsonl(&path).unwrap();
        assert_eq!(read_back, expected);

        let expected_via_reader = vec![
            Item {
                id: 1,
                name: "a".to_string(),
            },
            Item {
                id: 2,
                name: "b".to_string(),
            },
            Item {
                id: 3,
                name: "c".to_string(),
            },
        ];
        let via_reader: Vec<Item> = JsonlReader::<Item>::open(&path)
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(via_reader, expected_via_reader);
    }

    #[test]
    fn jsonl_reader_skips_blank_lines_and_reports_bad_line_numbers() {
        let path = temp_path("blank-lines.jsonl");
        std::fs::write(&path, "{\"id\":1,\"name\":\"a\"}\n\n{\"id\":2,\"name\":\"b\"}\nnot json\n").unwrap();

        let mut reader = JsonlReader::<Item>::open(&path).unwrap();

        let first = reader.next().unwrap().unwrap();
        assert_eq!(
            first,
            Item {
                id: 1,
                name: "a".to_string()
            }
        );

        let second = reader.next().unwrap().unwrap();
        assert_eq!(
            second,
            Item {
                id: 2,
                name: "b".to_string()
            }
        );

        match reader.next() {
            Some(Err(IoError::Json { line, .. })) => assert_eq!(line, 4),
            other => panic!("expected IoError::Json at line 4, got {other:?}"),
        }
    }

    #[test]
    fn json_pretty_round_trips_with_trailing_newline() {
        let path = temp_path("pretty.json");
        let item = Item {
            id: 7,
            name: "seven".to_string(),
        };
        write_json_pretty(&path, &item).unwrap();

        let text = std::fs::read_to_string(&path).unwrap();
        let expected = serde_json::to_string_pretty(&item).unwrap() + "\n";
        assert_eq!(text, expected);

        let read_back: Item = read_json(&path).unwrap();
        assert_eq!(read_back, item);
    }

    #[test]
    fn jsonl_writer_matches_write_jsonl() {
        let dir = std::env::temp_dir().join(format!("sd-jsonl-writer-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let items = vec![
            Item {
                id: 1,
                name: "a".to_string(),
            },
            Item {
                id: 2,
                name: "b".to_string(),
            },
            Item {
                id: 3,
                name: "c".to_string(),
            },
        ];

        let a = dir.join("a.jsonl");
        let count_a = write_jsonl(&a, items.iter()).unwrap();
        assert_eq!(count_a, 3);

        let b = dir.join("b.jsonl");
        let mut writer = JsonlWriter::create(&b).unwrap();
        for item in &items {
            writer.write(item).unwrap();
        }
        let count_b = writer.finish().unwrap();
        assert_eq!(count_b, 3);

        assert_eq!(std::fs::read(&a).unwrap(), std::fs::read(&b).unwrap());
    }

    #[test]
    fn missing_file_is_io_error() {
        let path = Path::new("definitely-missing-dir/x.json");
        assert!(matches!(read_json::<Item>(path), Err(IoError::Io { .. })));
        assert!(matches!(JsonlReader::<Item>::open(path), Err(IoError::Io { .. })));
    }
}
