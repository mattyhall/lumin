use crate::{store::Resource, ResourceProcessor};
use std::{error::Error, io::Read, path::Path};
use tracing::{debug, info, instrument};

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
            path: path.to_owned(),
            contents: buf,
        })
    }
}

pub struct LiquidProcessor {
    parser: liquid::Parser,
}

impl LiquidProcessor {
    pub fn new(partial_dir: impl AsRef<Path>) -> Result<LiquidProcessor, Box<dyn Error>> {
        let mut ims = liquid::partials::InMemorySource::new();

        for entry in std::fs::read_dir(partial_dir.as_ref())? {
            let entry = entry?;
            if entry.metadata()?.is_dir() {
                continue;
            }

            let path = entry.path();
            debug!(?path, "found partial");

            let short_path = path.strip_prefix(partial_dir.as_ref())?;

            let mut f = std::fs::File::open(&path)?;
            let mut buf = String::new();
            f.read_to_string(&mut buf)?;

            ims.add(short_path.to_string_lossy(), buf);
        }

        let partials = liquid::partials::EagerCompiler::new(ims);

        Ok(LiquidProcessor {
            parser: liquid::ParserBuilder::new()
                .stdlib()
                .partials(partials)
                .build()?,
        })
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
    }

    #[instrument]
    fn process(&self, path: &Path) -> Result<crate::store::Resource, Box<dyn Error>> {
        info!("liquid processing");

        let tmpl = self.parser.parse_file(&path)?;
        let obj = liquid::Object::new();

        let mut buffer = Vec::new();
        tmpl.render_to(&mut buffer, &obj)?;

        let mut path = path.to_owned();
        path.set_extension("html");

        Ok(Resource {
            path,
            contents: buffer,
        })
    }
}
