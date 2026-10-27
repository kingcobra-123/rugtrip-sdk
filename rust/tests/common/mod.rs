#![allow(dead_code)]
use std::path::PathBuf;

pub fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("fixtures")
}

pub fn fixture(name: &str) -> String {
    std::fs::read_to_string(fixtures_dir().join(name)).unwrap_or_else(|e| panic!("fixture {name}: {e}"))
}

pub fn json<T: serde::de::DeserializeOwned>(name: &str) -> T {
    serde_json::from_str(&fixture(name)).unwrap_or_else(|e| panic!("parse {name}: {e}"))
}
