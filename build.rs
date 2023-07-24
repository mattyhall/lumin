use std::{error::Error, path::PathBuf};

fn build_lang(lang: &str, mut dir: PathBuf) -> Result<(), Box<dyn Error>> {
    dir.push("src");

    let mut build = cc::Build::new();
    build.include(&dir);

    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        if entry.path().extension().map(|e| e == "c").unwrap_or(false) {
            build.file(&entry.path());
        }
    }

    build.compile(&format!("tree-sitter-{}", lang));
    Ok(())
}

fn lang_filepath(out_dir: &str, lang: &str) -> PathBuf {
    [out_dir, &format!("{}.rs", lang)].iter().collect()
}

fn write_lang(lang: &str, mut dir: PathBuf) -> Result<(), Box<dyn Error>> {
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let filepath = lang_filepath(&out_dir, lang);

    dir.push("queries");

    let mut queries = String::new();
    let mut config_args = Vec::new();

    let vars = &[
        ("HIGHLIGHT_QUERY", "highlights.scm"),
        ("INJECTIONS_QUERY", "injections.scm"),
        ("LOCALS_QUERY", "locals.scm"),
    ];
    for (variable, filename) in vars {
        let path = dir.join(filename);
        if !path.exists() {
            config_args.push(r#""""#);
            continue;
        }

        let path = path.canonicalize()?;

        queries += &format!(
            r#"pub const {}: &str = include_str!("{}");{}"#,
            variable,
            path.to_str().expect("dir not valid path"),
            "\n"
        );
        config_args.push(*variable);
    }

    let config_args = config_args.join(", ");

    let contents = format!(
        r#"
mod {lang} {{
    use super::*;
    extern "C" {{
        fn tree_sitter_{lang}() -> Language;
    }}

    {queries}

    pub fn get_config(highlight_names: &[&str]) -> Result<HighlightConfiguration, Box<dyn Error>> {{
        let lang = unsafe {{ tree_sitter_{lang}() }};
        let mut config = HighlightConfiguration::new(lang, {config_args})?;
        config.configure(highlight_names);
        Ok(config)
    }}
}}
    "#,
        lang = lang,
        queries = queries,
        config_args = config_args
    );

    std::fs::write(filepath, contents)?;

    Ok(())
}

fn lang(lang: &str) -> Result<(), Box<dyn Error>> {
    let module = format!("tree-sitter-{}", lang);
    let dir: PathBuf = ["third_party", &module].iter().collect();

    build_lang(lang, dir.clone())?;
    write_lang(lang, dir)?;
    Ok(())
}

fn main() {
    let out_dir = std::env::var("OUT_DIR").unwrap();

    let langs = &["zig", "c", "rust", "haskell"];

    langs
        .iter()
        .try_for_each(|l| lang(l))
        .expect("could not build");

    let includes: Vec<_> = langs
        .iter()
        .map(|l| {
            format!(
                r#"include!("{}");"#,
                lang_filepath(&out_dir, l).to_string_lossy()
            )
        })
        .collect();
    let includes = includes.join("\n");

    let inserts: Vec<_> = langs
        .iter()
        .map(|l| format!(r#"hm.insert("{0}", {0}::get_config(highlight_names)?);"#, l))
        .collect();
    let inserts = inserts.join("\n");

    let contents = format!(
        r#"
use std::collections::HashMap;
use std::error::Error;
use tree_sitter::Language;
use tree_sitter_highlight::HighlightConfiguration;

{}

pub fn get_configs(highlight_names: &[&'static str]) -> Result<HashMap<&'static str, HighlightConfiguration>, Box<dyn Error>> {{
    let mut hm = HashMap::new();
    {}
    Ok(hm)
}}
"#,
        includes, inserts
    );

    let mod_path: PathBuf = [&out_dir, "generated_tree_sitter.rs"].iter().collect();
    std::fs::write(mod_path, contents).expect("could not write generated_tree_sitter.rs");

    println!("cargo:rerun-if-changed=build.rs");
}
