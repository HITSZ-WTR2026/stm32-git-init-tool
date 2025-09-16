use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(tag = "mode")]
pub enum Patch {
    #[serde(rename = "append")]
    Append { file: String, after: String, insert: String, marker: String },
    #[serde(rename = "replace")]
    Replace { file: String, find: String, insert: String },
    #[serde(rename = "regex_replace")]
    RegexReplace { file: String, pattern: String, insert: String },
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub directories: Vec<String>,
    pub patches: Vec<Patch>,
}