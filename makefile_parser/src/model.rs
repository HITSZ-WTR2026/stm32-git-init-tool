use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct MakefileConfig {
    pub target: Option<String>,
    pub build_dir: Option<String>,
    pub c_sources: Vec<String>,
    pub asm_sources: Vec<String>,
    pub includes: Vec<String>,
    pub defines: Vec<String>, // 简化为字符串
    pub cflags: Vec<String>,
    pub asflags: Vec<String>,
    pub ldflags: Vec<String>,
    pub libs: Vec<String>,
    pub ldscript: Option<String>,
}
