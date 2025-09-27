use chrono::Local;
use include_dir::{include_dir, Dir};
use serde::Deserialize;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;
use tracing::{error, warn};

static DEFAULT_GITIGNORE_CONFIG_DIR: Dir = include_dir!("src/configs/gitignore");

#[derive(Debug, Deserialize)]
struct GitignoreConfig {
    name: String,
    description: String,
    enabled: bool,
    ignore: Option<Vec<String>>,
    sections: Option<std::collections::HashMap<String, SubSection>>,
}

#[derive(Debug, Deserialize)]
struct SubSection {
    enabled: bool,
    files: Option<Vec<String>>,
    files_disabled: Option<Vec<String>>, // 可选：关闭忽略专用
}
fn iter_gitignore_configs(config_dir: Option<&str>) -> Box<dyn Iterator<Item=GitignoreConfig>> {
    if let Some(dir) = config_dir {
        // 外部目录：读取文件系统
        let iter = fs::read_dir(dir)
            .unwrap()
            .filter_map(|entry| {
                entry.ok().and_then(|e| {
                    let path = e.path();
                    if path.extension().and_then(|s| s.to_str()) != Some("toml") {
                        return None;
                    }
                    let content = fs::read_to_string(&path).ok()?;
                    toml::from_str(&content).ok()
                })
            });
        Box::new(iter)
    } else {
        // 内嵌目录：使用 include_dir
        let iter = DEFAULT_GITIGNORE_CONFIG_DIR.files().filter_map(|f| {
            if f.path().extension().and_then(|s| s.to_str()) != Some("toml") {
                return None;
            }
            let content = f.contents_utf8()?;
            toml::from_str(content).ok()
        });
        Box::new(iter)
    }
}

pub fn generate_gitignore(config_dir: Option<&str>, is_force: bool) -> io::Result<()> {
    const PATH: &str = ".gitignore";

    if Path::new(PATH).exists() && !is_force {
        warn!("Skip existing {}", PATH);
        return Ok(());
    }

    let mut file = File::create(PATH)?;

    let now = Local::now();
    writeln!(file, "# generated on {}", now.format("%Y-%m-%d %H:%M:%S"))?;

    // 扫描所有 TOML 文件
    for config in iter_gitignore_configs(config_dir) {
        if !config.enabled {
            continue;
        }

        writeln!(file, "### {} ###", config.name)?;
        writeln!(file, "# {}", config.description)?;

        if let Some(ignore_list) = config.ignore {
            for line in ignore_list {
                writeln!(file, "{}", line)?;
            }
        }

        if let Some(sections) = config.sections {
            for (sec_name, sec) in sections {
                if sec.enabled {
                    writeln!(file, "# section: {}", sec_name)?;
                    if let Some(files) = sec.files {
                        for f in files {
                            writeln!(file, "{}", f)?;
                        }
                    } else {
                        error!("{sec_name} is enabled, but `files` is None");
                    }
                } else if let Some(files) = sec.files_disabled {
                    writeln!(file, "# section: {}", sec_name)?;
                    for f in files {
                        writeln!(file, "{}", f)?;
                    }
                }
            }
        }

        writeln!(file)?; // 空行分隔
    }
    Ok(())
}
