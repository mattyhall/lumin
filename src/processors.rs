use crate::{
    store::{Resource, EXTENSIONS},
    ResourceProcessor,
};
use std::{error::Error, io::Read, path::Path};
use tracing::{info, instrument};

#[derive(Debug)]
pub struct StaticProcessor {}
impl ResourceProcessor for StaticProcessor {
    fn matches(&self, path: &Path) -> bool {
        path.extension()
            .map(|e| EXTENSIONS.iter().any(|wanted| *wanted == e))
            .unwrap_or(false)
    }

    #[instrument]
    fn process(&self, path: &Path) -> Result<Resource, Box<dyn Error>> {
        info!("statically processing");

        let mut buf = Vec::new();
        std::fs::File::open(path)?.read_to_end(&mut buf)?;

        Ok(Resource {
            path: path.to_owned(),
            contents: buf,
        })
    }
}
