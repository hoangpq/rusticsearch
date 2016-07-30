pub mod index;
pub mod index_registry;

use std::sync::RwLock;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs;

use slog::Logger;

use search::store::IndexStore;
use search::store::memory::MemoryIndexStore;

use system::index::Index;
use system::index_registry::IndexRegistry;


pub struct System {
    pub log: Logger,
    data_dir: PathBuf,
    pub indices: RwLock<IndexRegistry>,
}


impl System {
    pub fn new(log: Logger, data_dir: PathBuf) -> System {
        System {
            log: log,
            data_dir: data_dir,
            indices: RwLock::new(IndexRegistry::new()),
        }
    }

    pub fn get_indices_dir(&self) -> PathBuf {
        let mut dir = self.data_dir.clone();
        dir.push("indices");
        dir
    }

    fn load_index(&self, path: &Path) -> Index {
        Index::new(MemoryIndexStore::new())
    }

    pub fn load_indices(&self) -> HashMap<String, Index> {
        let mut indices = HashMap::new();

        let indices_dir = self.get_indices_dir();
        match fs::read_dir(indices_dir.clone()) {
            Ok(files) => {
                for file in files {
                    let path = file.unwrap().path();
                    let index_name: String = path.file_stem().unwrap().to_str().unwrap().to_owned();

                    if let Some(ext) = path.extension() {
                        if ext.to_str() == Some("rsi") {
                            self.log.info("[sys] loaded index", b!("index" => index_name));
                            self.indices.write().unwrap().insert(index_name, self.load_index(path.as_path()));
                        }
                    }
                }
            }
            Err(error) => {
                self.log.warn("[sys] cannot open indices directory", b!(
                    "dir" => indices_dir.to_str().unwrap(),
                    "error" => format!("{}", error)
                ));
            }
        }

        indices
    }
}