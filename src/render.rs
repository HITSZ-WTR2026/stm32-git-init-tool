use serde::Serialize;
use std::fs;
use std::path::Path;
use tinytemplate::TinyTemplate;
use tracing::warn;

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
    Ok(())
}