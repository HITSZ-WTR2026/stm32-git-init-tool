use crate::config::Patch;
use regex::Regex;
use std::fs;

pub fn apply_patch(patch: &Patch) -> std::io::Result<()> {
    let content = match fs::read_to_string(&get_file(patch)) {
        Ok(c) => c,
        Err(_) => return Ok(()), // 文件不存在，跳过
    };

    let new_content = match patch {
        Patch::Append { after, insert, marker, .. } => {
            if content.contains(marker) { return Ok(()); }
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
                .join("\n") + "\n"
        }
        Patch::Replace { find, insert, .. } => {
            if content.contains(insert) { return Ok(()); }
            content.replace(find, insert)
        }
        Patch::RegexReplace { pattern, insert, .. } => {
            let re = Regex::new(pattern).unwrap();
            if re.is_match(&content) && content.contains(insert) {
                return Ok(());
            }
            re.replace_all(&content, insert.as_str()).to_string()
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
    }
}
