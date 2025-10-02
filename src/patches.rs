use regex::Regex;
use serde::Deserialize;
use std::fs;

#[derive(Debug, Deserialize)]
#[serde(tag = "mode")]
pub enum Patch {
    #[serde(rename = "append")]
    Append {
        file: String,
        after: String,
        insert: String,
        marker: String,
    },
    #[serde(rename = "replace")]
    Replace {
        file: String,
        find: String,
        insert: String,
    },
    #[serde(rename = "regex_replace")]
    RegexReplace {
        file: String,
        pattern: String,
        insert: String,
    },
    #[serde(rename = "uncomment_block")]
    UncommentBlock { file: String, marker: String },
}

pub fn apply_patch(patch: &Patch) -> std::io::Result<()> {
    let content = match fs::read_to_string(&get_file(patch)) {
        Ok(c) => c,
        Err(_) => return Ok(()), // 文件不存在，跳过
    };

    let new_content = match patch {
        Patch::Append {
            after,
            insert,
            marker,
            ..
        } => {
            if content.contains(marker) {
                return Ok(());
            }
            content
                .lines()
                .map(|line| {
                    if line.contains(after) {
                        format!("{}\n{}", line, insert)
                    } else {
                        line.to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
                + "\n"
        }
        Patch::Replace { find, insert, .. } => {
            if content.contains(insert) {
                return Ok(());
            }
            content.replace(find, insert)
        }
        Patch::RegexReplace {
            pattern, insert, ..
        } => {
            let re = Regex::new(pattern).unwrap();
            if re.is_match(&content) && content.contains(insert) {
                return Ok(());
            }
            re.replace_all(&content, insert.as_str()).to_string()
        }
        Patch::UncommentBlock { marker, .. } => {
            let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();

            let mut in_block = false;
            for i in 0..lines.len() {
                if lines[i].contains(marker) {
                    in_block = true;
                    continue; // marker 行本身保留
                }

                if in_block {
                    if lines[i].starts_with('#') {
                        // 去掉行首 "# " 或 "#"
                        lines[i] = lines[i].trim_start_matches('#').trim().to_string();
                    } else {
                        // 遇到非注释行/空行，说明 block 结束
                        break;
                    }
                }
            }

            lines.join("\n") + "\n"
        }
    };

    fs::write(get_file(patch), new_content)?;
    Ok(())
}

fn get_file(patch: &Patch) -> &str {
    match patch {
        Patch::Append { file, .. } => file,
        Patch::Replace { file, .. } => file,
        Patch::RegexReplace { file, .. } => file,
        Patch::UncommentBlock { file, .. } => file,
    }
}
