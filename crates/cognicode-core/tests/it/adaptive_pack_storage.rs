//! The storage fixture: one contract, three implementations, a family of
//! near-identical helpers of the same class, a consumer and a test.

use cognicode_core::domain::value_objects::{DependencyType::*, SymbolKind};

use super::adaptive_pack_world::{World, WorldBuilder};

const STORAGE: &str = "\
/// Where bytes are kept, by key.
pub trait Storage {
    fn save(&self, key: &str, bytes: &[u8]) -> Result<(), StorageError>;
    fn load(&self, key: &str) -> Result<Vec<u8>, StorageError>;
}
";

const FILE_STORAGE: &str = "\
use std::fs;
use std::path::{Path, PathBuf};

pub struct FileStorage {
    root: PathBuf,
}

impl FileStorage {
    fn save_header(&self, key: &str) -> Vec<u8> {
        format!(\"v1:{key}\").into_bytes()
    }

    fn save_checksum(&self, bytes: &[u8]) -> u32 {
        bytes.iter().fold(0, |sum, byte| sum.wrapping_add(*byte as u32))
    }

    fn save_atomic(&self, path: &Path, payload: &[u8]) -> Result<(), StorageError> {
        let temp = path.with_extension(\"tmp\");
        fs::write(&temp, payload)?;
        fs::rename(&temp, path)?;
        Ok(())
    }

    fn save_index(&self, key: &str) -> Result<(), StorageError> {
        let index = self.root.join(\"index\");
        fs::write(index, key)?;
        Ok(())
    }
}

impl Storage for FileStorage {
    fn save(&self, key: &str, bytes: &[u8]) -> Result<(), StorageError> {
        let mut payload = self.save_header(key);
        payload.extend_from_slice(bytes);
        let checksum = self.save_checksum(bytes);
        payload.extend_from_slice(&checksum.to_le_bytes());
        self.save_atomic(&self.root.join(key), &payload)?;
        self.save_index(key)
    }

    fn load(&self, key: &str) -> Result<Vec<u8>, StorageError> {
        Ok(fs::read(self.root.join(key))?)
    }
}
";

const MEMORY_STORAGE: &str = "\
use std::collections::HashMap;

pub struct MemoryStorage {
    entries: HashMap<String, Vec<u8>>,
}

impl Storage for MemoryStorage {
    fn save(&self, key: &str, bytes: &[u8]) -> Result<(), StorageError> {
        self.entries.insert(key.to_string(), bytes.to_vec());
        Ok(())
    }

    fn load(&self, key: &str) -> Result<Vec<u8>, StorageError> {
        self.entries.get(key).cloned().ok_or(StorageError::Missing)
    }
}
";

const S3_STORAGE: &str = "\
pub struct S3Storage {
    bucket: String,
}

impl Storage for S3Storage {
    fn save(&self, key: &str, bytes: &[u8]) -> Result<(), StorageError> {
        put_object(&self.bucket, key, bytes)
    }

    fn load(&self, key: &str) -> Result<Vec<u8>, StorageError> {
        get_object(&self.bucket, key)
    }
}
";

const BACKUP: &str = "\
pub fn backup_job(storage: &FileStorage, keys: &[String]) -> Result<(), StorageError> {
    for key in keys {
        storage.save(key, b\"snapshot\")?;
    }
    Ok(())
}

#[test]
fn a_file_survives_a_roundtrip() {
    let storage = FileStorage::open(\"/tmp/roundtrip\");
    storage.save(\"k\", b\"v\").unwrap();
    assert_eq!(storage.load(\"k\").unwrap(), b\"v\");
}
";

const NOISE: &str = "\
pub fn log_event(message: &str) {
    println!(\"{message}\");
}

pub fn render_report(rows: &[Row]) -> String {
    log_event(\"report\");
    rows.iter().map(|row| row.to_string()).collect()
}

pub fn parse_options(args: &[String]) -> Options {
    log_event(\"options\");
    Options::from(args)
}

pub fn schedule_nightly() {
    log_event(\"schedule\");
}
";

/// The keys of the helpers of `FileStorage::save`, all of one class.
pub const HELPERS: [&str; 4] = ["save_header", "save_checksum", "save_atomic", "save_index"];

/// The storage files and symbols, with no relation drawn between them.
pub fn storage_symbols() -> WorldBuilder {
    let mut world = WorldBuilder::new()
        .file("src/storage.rs", STORAGE)
        .file("src/file_storage.rs", FILE_STORAGE)
        .file("src/memory_storage.rs", MEMORY_STORAGE)
        .file("src/s3_storage.rs", S3_STORAGE)
        .file("src/backup.rs", BACKUP)
        .file("src/noise.rs", NOISE)
        .symbol(
            "storage",
            "src/storage.rs",
            "Storage",
            SymbolKind::Trait,
            "pub trait Storage",
        )
        .symbol(
            "file",
            "src/file_storage.rs",
            "FileStorage",
            SymbolKind::Struct,
            "pub struct FileStorage",
        )
        .symbol(
            "memory",
            "src/memory_storage.rs",
            "MemoryStorage",
            SymbolKind::Struct,
            "pub struct MemoryStorage",
        )
        .symbol(
            "s3",
            "src/s3_storage.rs",
            "S3Storage",
            SymbolKind::Struct,
            "pub struct S3Storage",
        )
        .symbol(
            "file_save",
            "src/file_storage.rs",
            "save",
            SymbolKind::Method,
            "fn save(",
        )
        .symbol(
            "file_load",
            "src/file_storage.rs",
            "load",
            SymbolKind::Method,
            "fn load(",
        )
        .symbol(
            "memory_save",
            "src/memory_storage.rs",
            "save",
            SymbolKind::Method,
            "fn save(",
        )
        .symbol(
            "s3_save",
            "src/s3_storage.rs",
            "save",
            SymbolKind::Method,
            "fn save(",
        )
        .symbol(
            "backup_job",
            "src/backup.rs",
            "backup_job",
            SymbolKind::Function,
            "pub fn backup_job",
        )
        .test(
            "roundtrip",
            "src/backup.rs",
            "a_file_survives_a_roundtrip",
            "fn a_file_survives_a_roundtrip",
        )
        .symbol(
            "log_event",
            "src/noise.rs",
            "log_event",
            SymbolKind::Function,
            "pub fn log_event",
        )
        .symbol(
            "render_report",
            "src/noise.rs",
            "render_report",
            SymbolKind::Function,
            "pub fn render_report",
        )
        .symbol(
            "parse_options",
            "src/noise.rs",
            "parse_options",
            SymbolKind::Function,
            "pub fn parse_options",
        )
        .symbol(
            "schedule_nightly",
            "src/noise.rs",
            "schedule_nightly",
            SymbolKind::Function,
            "pub fn schedule_nightly",
        );
    for helper in HELPERS {
        world = world.symbol(
            helper,
            "src/file_storage.rs",
            helper,
            SymbolKind::Method,
            &format!("fn {helper}("),
        );
    }
    world
}

/// The storage world with every relation the extractor would draw.
pub fn storage_world() -> World {
    let mut world = storage_symbols()
        // The types declare the contract they satisfy.
        .edge("file", Inherits, "storage")
        .edge("memory", Inherits, "storage")
        .edge("s3", Inherits, "storage")
        // Each type contains its methods.
        .edge("file", Contains, "file_save")
        .edge("file", Contains, "file_load")
        .edge("memory", Contains, "memory_save")
        .edge("s3", Contains, "s3_save")
        // `FileStorage::save` and the code that depends on it.
        .edge("backup_job", References, "file")
        .edge("backup_job", Calls, "file_save")
        .edge("roundtrip", References, "file")
        .edge("roundtrip", Calls, "file_save")
        // The noise: everything logs.
        .edge("render_report", Calls, "log_event")
        .edge("parse_options", Calls, "log_event")
        .edge("schedule_nightly", Calls, "log_event")
        .edge("backup_job", Calls, "log_event");
    for helper in HELPERS {
        world = world
            .edge("file", Contains, helper)
            .edge("file_save", Calls, helper);
    }
    world.build()
}
