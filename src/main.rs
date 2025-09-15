mod config;
mod patches;
mod templates;
mod utils;

use crate::config::Config;
use crate::patches::apply_patch;
use crate::templates::{APP_C, APP_H, CLANG_FORMAT, GITIGNORE, README_MD};
use crate::utils::get_author;
use chrono::Local;
use clap::Parser;
use serde::Serialize;
use std::fs;
use std::path::Path;
use tinytemplate::TinyTemplate;
use tracing::{info, warn};

#[derive(Parser)]
struct Cli {
    /// 配置文件地址
    #[arg(short, long, default_value = "")]
    config: String,
    #[arg(long)]
    force: bool,
}

#[derive(Serialize)]
struct Context {
    author: String,
    date: String,
    year: String,
}

fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    // 渲染上下文
    let author = get_author();

    let now = Local::now();
    let ctx = Context {
        author,
        date: now.format("%Y-%m-%d").to_string(),
        year: now.format("%Y").to_string(),
    };

    // 渲染模板文件
    render_file(".gitignore", GITIGNORE, &ctx, cli.force)?;
    render_file(".clang-format", CLANG_FORMAT, &ctx, cli.force)?;
    render_file("UserCode/app/app.h", APP_H, &ctx, cli.force)?;
    render_file("UserCode/app/app.c", APP_C, &ctx, cli.force)?;
    render_file("UserCode/README.md", README_MD, &ctx, cli.force)?;

    // 内置 patch 配置
    let default_config: Config = serde_yaml_ng::from_str(include_str!("config.yaml")).unwrap();

    for patch in default_config.patches {
        apply_patch(&patch)?;
        info!("Patched {:?}", patch);
    }

    Ok(())
}

pub fn render_file<T: Serialize>(path: &str, template: &str, ctx: &T, force: bool) -> std::io::Result<()> {
    if Path::new(path).exists() && !force {
        warn!("Skip existing {}", path);
        return Ok(());
    }

    if let Some(parent) = Path::new(path).parent() {
        fs::create_dir_all(parent)?;
    }

    let mut tt = TinyTemplate::new();
    tt.add_template("tpl", template).unwrap();

    // 渲染模板
    let content = tt.render("tpl", ctx).unwrap();

    fs::write(path, content)?;
    info!("Generated {}", path);
    Ok(())
}
