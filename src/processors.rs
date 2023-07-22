use crate::{store::Resource, ResourceProcessor};
use markdown;
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
        LiquidProcessor {
            partials_dir,
            parser,
        }
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

pub struct PostsProcessor {
    posts_dir: PathBuf,
    posts_template_path: PathBuf,
    post_template: liquid::Template,
}

impl PostsProcessor {
    pub fn new(
        posts_dir: PathBuf,
        posts_template_path: PathBuf,
        parser: &liquid::Parser,
    ) -> Result<Self, Box<dyn Error>> {
        let post_template = parser.parse_file(&posts_template_path)?;
        Ok(Self {
            posts_dir,
            post_template,
            posts_template_path,
        })
    }
}

impl std::fmt::Debug for PostsProcessor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PostsProcessor")
    }
}

impl ResourceProcessor for PostsProcessor {
    fn matches(&self, path: &Path) -> bool {
        if path == self.posts_template_path {
            return true;
        }

        path.starts_with(&self.posts_dir)
            && path
                .extension()
                .map(|e| e == "md" || e == "markdown")
                .unwrap_or(false)
    }

    #[instrument]
    fn process(&self, path: &Path) -> Result<Resource, Box<dyn Error>> {
        info!("post processing");

        if path == self.posts_template_path {
            return Ok(Resource {
                contents: vec![],
                original_path: path.to_owned(),
                renamed_path: None,
            });
        }

        let mut f = std::fs::File::open(path)?;
        let mut buf = String::new();
        f.read_to_string(&mut buf)?;

        let html = markdown::to_html(&buf);
        let obj = liquid::object!({ "contents": html });
        let mut contents_buf = Vec::new();
        self.post_template.render_to(&mut contents_buf, &obj)?;

        let mut new_path = path.to_owned();
        new_path.set_extension("html");

        Ok(Resource {
            original_path: path.to_owned(),
            renamed_path: Some(new_path),
            contents: contents_buf,
        })
    }
}
