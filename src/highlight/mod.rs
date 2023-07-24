use std::{collections::HashMap, error::Error};
use tree_sitter_highlight::HighlightEvent;

mod generated {
    include!(concat!(env!("OUT_DIR"), "/generated_tree_sitter.rs"));
}

const HIGHLIGHT_NAMES: &[&str] = &[
    "attribute",
    "boolean",
    "conditional",
    "comment",
    "constant",
    "constant.builtin",
    "float",
    "function",
    "function.builtin",
    "function_call",
    "function.keyword",
    "keyword",
    "label",
    "number",
    "operator",
    "property",
    "punctuation",
    "punctuation.bracket",
    "punctuation.delimiter",
    "repeat",
    "string",
    "string.special",
    "tag",
    "type",
    "type.builtin",
    "type.qualified",
    "variable",
    "variable.builtin",
    "variable.parameter",
];

pub struct Highlight {
    configs: HashMap<&'static str, tree_sitter_highlight::HighlightConfiguration>,
    highlighter: tree_sitter_highlight::Highlighter,
}

impl Highlight {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            configs: generated::get_configs(HIGHLIGHT_NAMES)?,
            highlighter: tree_sitter_highlight::Highlighter::new(),
        })
    }

    pub fn supported(&self, lang: &str) -> bool {
        self.configs.contains_key(lang)
    }

    pub fn highlight(&mut self, language: &str, code: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
        let config = &self.configs[language];
        let highlights = self.highlighter.highlight(config, code, None, |_| None)?;

        let mut buf = Vec::with_capacity(code.len());
        for event in highlights {
            match event? {
                HighlightEvent::Source { start, end } => {
                    let s = std::str::from_utf8(&code[start..end])?;
                    html_escape::encode_safe_to_vec(s, &mut buf);
                }
                HighlightEvent::HighlightStart(h) => {
                    let class = HIGHLIGHT_NAMES[h.0].replace('.', "-");
                    buf.extend_from_slice(format!(r#"<span class="{}">"#, class).as_bytes());
                }
                HighlightEvent::HighlightEnd => buf.extend_from_slice(b"</span>"),
            }
        }

        Ok(buf)
    }
}
