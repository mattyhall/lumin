use crate::{store::Resource, ResourceProcessor};
use std::{
    error::Error,
    io::Read,
    path::{Path, PathBuf},
};
use tracing::{info, instrument};

const STATIC_EXTENSIONS: &[&str] = &["css", "html", "jpg", "jpeg", "woff2"];

#[derive(Debug)]
pub struct StaticProcessor {}

impl ResourceProcessor for StaticProcessor {
    fn matches(&self, path: &Path) -> bool {
        path.extension()
            .map(|e| STATIC_EXTENSIONS.iter().any(|wanted| *wanted == e))
            .unwrap_or(false)
    }

    #[instrument]
    fn process(&self, path: &Path) -> Result<Resource, Box<dyn Error>> {
        info!("statically processing");

        let mut buf = Vec::new();
        std::fs::File::open(path)?.read_to_end(&mut buf)?;

        Ok(Resource {
            original_path: path.to_owned(),
            renamed_path: None,
            contents: buf,
        })
    }
}

pub struct LiquidProcessor {
    partials_dir: PathBuf,
    parser: liquid::Parser,
}

impl LiquidProcessor {
    pub fn new(partials_dir: PathBuf, parser: liquid::Parser) -> LiquidProcessor {
        LiquidProcessor { partials_dir, parser }
    }
}

impl std::fmt::Debug for LiquidProcessor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("LiquidProcessor{}")
    }
}

impl ResourceProcessor for LiquidProcessor {
    fn matches(&self, path: &Path) -> bool {
        path.extension().map(|e| e == "liquid").unwrap_or(false)
            && !path.starts_with(&self.partials_dir)
    }

    #[instrument]
    fn process(&self, path: &Path) -> Result<crate::store::Resource, Box<dyn Error>> {
        info!("liquid processing");

        let tmpl = self.parser.parse_file(path)?;
        let obj = liquid::Object::new();

        let mut buffer = Vec::new();
        tmpl.render_to(&mut buffer, &obj)?;

        let mut new_path = path.to_owned();
        new_path.set_extension("html");

        Ok(Resource {
            original_path: path.to_owned(),
            renamed_path: Some(new_path),
            contents: buffer,
        })
    }
}
