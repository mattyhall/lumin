use crate::{
    highlight,
    store::{Resource, URLPath},
    ResourceProcessor,
};
use markdown;
use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};
use std::{
    error::Error,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
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

        let buf = std::fs::read(path)?;
        Ok(Resource {
            original_path: path.to_owned(),
            contents: buf,
            ..Default::default()
        })
    }
}

pub struct LiquidProcessor {
    partials_dir: PathBuf,
    parser: liquid::Parser,
    development: bool,
}

impl LiquidProcessor {
    pub fn new(
        partials_dir: PathBuf,
        parser: liquid::Parser,
        development: bool,
    ) -> LiquidProcessor {
        LiquidProcessor {
            partials_dir,
            parser,
            development,
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
        let obj = liquid::object!({"development": self.development});

        let mut buffer = Vec::new();
        tmpl.render_to(&mut buffer, &obj)?;

        let mut new_path = path.to_owned();
        new_path.set_extension("html");

        Ok(Resource {
            original_path: path.to_owned(),
            url_path: URLPath::Filepath(new_path),
            contents: buffer,
        })
    }
}

#[derive(Deserialize)]
struct PostMetadata {
    title: String,
    description: String,
    published: toml::value::Datetime,
}

#[derive(Serialize)]
struct PostItem {
    filename: String,
    title: String,
    description: String,
    published: String,
}

pub struct PostsProcessor {
    posts_dir: PathBuf,
    posts_template_path: PathBuf,
    post_template: liquid::Template,
    post_list_template_path: PathBuf,
    post_list_template: liquid::Template,

    code_regex: Regex,

    posts: Arc<Mutex<Vec<PostItem>>>,
    highlighter: Arc<Mutex<highlight::Highlight>>,

    development: bool,
}

impl PostsProcessor {
    pub fn new(
        posts_dir: PathBuf,
        posts_template_path: PathBuf,
        post_list_template_path: PathBuf,
        parser: &liquid::Parser,
        development: bool,
    ) -> Result<Self, Box<dyn Error>> {
        let post_template = parser.parse_file(&posts_template_path)?;
        let post_list_template = parser.parse_file(&post_list_template_path)?;
        Ok(Self {
            posts_dir,
            post_template,
            posts_template_path,
            post_list_template_path,
            post_list_template,
            development,
            posts: Arc::default(),
            highlighter: Arc::new(Mutex::new(highlight::Highlight::new()?)),
            code_regex: RegexBuilder::new(
                r#"<pre>\s*<code( class="language-(.*?)")?>(.+?)</code>\s*</pre>"#,
            )
            .multi_line(true)
            .dot_matches_new_line(true)
            .build()?,
        })
    }

    #[instrument]
    fn get_metadata(&self, mut path: PathBuf) -> Result<PostMetadata, Box<dyn Error>> {
        path.set_extension("toml");

        let buf = std::fs::read_to_string(path)?;
        Ok(toml::from_str(&buf)?)
    }

    fn render_post_list(
        &self,
        i: usize,
        last: bool,
        posts: &[PostItem],
    ) -> Result<Resource, Box<dyn Error>> {
        let new_path = if i == 0 {
            "posts/index.html".to_owned()
        } else {
            format!("posts/posts-{}.html", i)
        };

        let previous = match i {
            0 => "".to_owned(),
            1 => "/posts/index.html".to_owned(),
            _ => format!("/posts/posts-{}.html", i - 1),
        };
        let next = if last {
            "".to_owned()
        } else {
            format!("posts-{}.html", i + 1)
        };

        let obj = liquid::object!({"posts": posts, "previous": previous, "next": next ,"development": self.development});
        let mut buf = Vec::new();
        self.post_list_template.render_to(&mut buf, &obj)?;

        Ok(Resource {
            original_path: self.post_list_template_path.clone(),
            url_path: URLPath::Absolute(new_path),
            contents: buf,
        })
    }

    #[instrument(skip(src))]
    fn highlight_code(&self, src: &str) -> Result<String, Box<dyn Error>> {
        let mut contents = src.to_owned();
        for c in self.code_regex.captures_iter(src) {
            let all = c.get(0).unwrap();
            let code = c.get(3).unwrap().as_str();

            let language = c.get(2).map(|c| c.as_str()).unwrap_or("");
            debug!(
                start = all.start(),
                end = all.end(),
                language,
                "got code match"
            );

            let mut highlighter = self.highlighter.lock().map_err(|e| e.to_string())?;
            if !highlighter.supported(language) {
                contents = contents.replace(
                    all.as_str(),
                    &format!(r#"<pre class="code-listing"><code>{}</code></pre>"#, code),
                );
                continue;
            }

            let code = {
                let code = &html_escape::decode_html_entities(code);
                highlighter.highlight(&c[2], code.as_bytes())?
            };
            contents = contents.replace(
                all.as_str(),
                &format!(
                    r#"<pre class="code-listing"><code>{}</code></pre>"#,
                    std::str::from_utf8(&code)?
                ),
            );
        }

        Ok(contents)
    }
}

impl std::fmt::Debug for PostsProcessor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PostsProcessor")
    }
}

impl ResourceProcessor for PostsProcessor {
    fn matches(&self, path: &Path) -> bool {
        if path == self.posts_template_path || path == self.post_list_template_path {
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
        if path == self.posts_template_path || path == self.post_list_template_path {
            return Ok(Resource {
                contents: vec![],
                original_path: path.to_owned(),
                ..Default::default()
            });
        }

        info!("post processing");

        let buf = std::fs::read_to_string(path)?;
        let html = markdown::to_html(&buf);

        let meta = self.get_metadata(path.to_owned())?;

        let obj = liquid::object!({ "contents": html, "post_title": meta.title, "post_published": meta.published.to_string(), "development": self.development });
        let contents = self.highlight_code(&self.post_template.render(&obj)?)?;

        let mut new_path = path.to_owned();
        new_path.set_extension("html");

        {
            let mut handle = self.posts.lock().map_err(|e| e.to_string())?;
            handle.push(PostItem {
                filename: new_path.file_name().unwrap().to_string_lossy().into(),
                title: meta.title,
                description: meta.description,
                published: meta.published.to_string(),
            })
        }

        Ok(Resource {
            original_path: path.to_owned(),
            url_path: URLPath::Filepath(new_path),
            contents: contents.as_bytes().to_owned(),
        })
    }

    #[instrument]
    fn flush(&self) -> Result<Vec<Resource>, Box<dyn Error>> {
        let mut handle = self.posts.lock().map_err(|e| e.to_string())?;
        let mut posts = std::mem::take(&mut *handle);

        posts.sort_by(|a, b| a.published.cmp(&b.published).reverse());

        let chunks: Vec<_> = posts.chunks(10).collect();
        let len = chunks.len();
        let resources: Result<Vec<_>, Box<dyn Error>> = chunks
            .into_iter()
            .enumerate()
            .map(|(i, chunk)| self.render_post_list(i, i == len - 1, chunk))
            .collect();

        resources
    }
}
