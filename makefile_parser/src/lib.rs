mod model;

use crate::model::MakefileConfig;
use regex::Regex;
use std::collections::HashSet;

/// 展开多行续行
fn unfold_multiline(lines: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    for l in lines {
        let trimmed = l.trim_end();
        if trimmed.ends_with('\\') {
            current.push_str(&trimmed[..trimmed.len() - 1]);
            current.push(' ');
        } else {
            current.push_str(trimmed);
            result.push(current.clone());
            current.clear();
        }
    }
    if !current.is_empty() {
        result.push(current);
    }
    result
}

/// 解析 Makefile
pub fn parse_makefile(content: &str) -> MakefileConfig {
    let lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    let lines = unfold_multiline(&lines);

    let mut cfg = MakefileConfig {
        target: None,
        build_dir: None,
        c_sources: vec![],
        asm_sources: vec![],
        includes: vec![],
        defines: vec![],
        cflags: vec![],
        asflags: vec![],
        ldflags: vec![],
        libs: vec![],
        ldscript: None,
    };

    let mut include_set = HashSet::new();
    let mut define_set = HashSet::new();

    let re_assign = Regex::new(r"^([A-Z0-9_-]+)\s*[:+]?=\s*(.*)$").unwrap();

    for line in lines {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some(cap) = re_assign.captures(line) {
            let key = &cap[1];
            let val = &cap[2];

            match key {
                "TARGET" => cfg.target = Some(val.into()),
                "BUILD_DIR" => cfg.build_dir = Some(val.into()),
                "C_SOURCES" => cfg
                    .c_sources
                    .extend(val.split_whitespace().map(|s| s.to_string())),
                "ASM_SOURCES" => cfg
                    .asm_sources
                    .extend(val.split_whitespace().map(|s| s.to_string())),
                "C_INCLUDES" | "AS_INCLUDES" => {
                    for token in val.split_whitespace() {
                        if token.starts_with("-I") {
                            let path = token.trim_start_matches("-I").to_string();
                            if include_set.insert(path.clone()) {
                                cfg.includes.push(path);
                            }
                        }
                    }
                }
                "C_DEFS" | "AS_DEFS" => {
                    for token in val.split_whitespace() {
                        let name = if token.starts_with("-D") {
                            token.trim_start_matches("-D").to_string()
                        } else if token.starts_with("-include") {
                            token.trim_start_matches("-include").trim().to_string()
                        } else {
                            token.to_string()
                        };
                        if !name.is_empty() && define_set.insert(name.clone()) {
                            cfg.defines.push(name);
                        }
                    }
                }
                "CFLAGS" => cfg
                    .cflags
                    .extend(val.split_whitespace().map(|s| s.to_string())),
                "ASFLAGS" => cfg
                    .asflags
                    .extend(val.split_whitespace().map(|s| s.to_string())),
                "LDFLAGS" => cfg
                    .ldflags
                    .extend(val.split_whitespace().map(|s| s.to_string())),
                "LIBS" => cfg
                    .libs
                    .extend(val.split_whitespace().map(|s| s.to_string())),
                "LDSCRIPT" => cfg.ldscript = Some(val.into()),
                _ => {}
            }
        }
    }

    cfg
}
