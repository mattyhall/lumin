use std::{error::Error, path::Path};

pub mod highlight;
pub mod processors;
pub mod store;

pub trait ResourceProcessor: Send + Sync + std::fmt::Debug {
    fn matches(&self, path: &Path) -> bool;
    fn process(&self, path: &Path) -> Result<store::Resource, Box<dyn Error>>;

    fn flush(&self) -> Result<Vec<store::Resource>, Box<dyn Error>> {
        Ok(Vec::new())
    }
}
