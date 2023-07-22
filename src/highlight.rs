use std::{collections::HashMap, error::Error};

use tree_sitter::Language;
use tree_sitter_highlight::HighlightEvent;

extern "C" {
    fn tree_sitter_zig() -> Language;
}

const ZIG_HIGHLIGHT_QUERY: &'static str =
    include_str!("../third_party/tree-sitter-zig/queries/highlights.scm");
const ZIG_INJECTIONS_QUERY: &'static str =
    include_str!("../third_party/tree-sitter-zig/queries/injections.scm");

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
        let mut c = HashMap::new();

        let zig = unsafe { tree_sitter_zig() };
        let mut zig_config = tree_sitter_highlight::HighlightConfiguration::new(
            zig,
            ZIG_HIGHLIGHT_QUERY,
            ZIG_INJECTIONS_QUERY,
            "",
        )?;
        zig_config.configure(HIGHLIGHT_NAMES);
        c.insert("zig", zig_config);

        Ok(Self {
            configs: c,
            highlighter: tree_sitter_highlight::Highlighter::new(),
        })
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
                },
                HighlightEvent::HighlightStart(h) => {
                    let class = HIGHLIGHT_NAMES[h.0].replace(".", "-");
                    buf.extend_from_slice(format!(r#"<span class="{}">"#, class).as_bytes());
                }
                HighlightEvent::HighlightEnd => buf.extend_from_slice(b"</span>"),
            }
        }

        Ok(buf)
    }
}
