use std::{error::Error, path::Path};

pub mod processors;
pub mod store;

pub trait ResourceProcessor: Send + Sync {
    fn matches(&self, path: &Path) -> bool;
    fn process(&self, path: &Path) -> Result<store::Resource, Box<dyn Error>>;
}
