use crate::initializers::{FPUType, IdeInitArgs, IdeInitializer};
use crate::render::render_file;
use anyhow::anyhow;
use serde::Serialize;
use std::fs;
use std::path::Path;
use tracing::{error, info};

#[derive(Serialize)]
struct EIDEConfigContext<'a> {
    project_name: &'a String,
    ld_file_path: &'a String,
    src_dirs: &'a String,
    include_list: &'a String,
    define_list: &'a String,
    src_files: &'a String,
    floating_point_hardware: &'a str,
    fpu_type: &'a str,
}

#[derive(Serialize)]
struct EIDEProjectFile<'a> {
    path: &'a String,
}
const EIDE_CONFIG: &str = include_str!("../templates/eide-config.tmpl");
const EIDE_WORKSPACE: &str = include_str!("../templates/eide-workspace.tmpl");
pub struct EIDE;

impl IdeInitializer for EIDE {
    fn name(&self) -> &'static str {
        "VSCode + EIDE (toolchain: Makefile)"
    }

    fn init(&self, args: &IdeInitArgs, force: bool) -> anyhow::Result<()> {
        if !Path::new("Makefile").exists() {
            error!("Makefile is not exists, initialization failed");
            return Err(anyhow!("Makefile is not exists, initialization failed"));
        }

        let makefile = fs::read_to_string("Makefile")?;
        let parsed_makefile = makefile_parser::parse_makefile(makefile.as_str());

        let mut files = Vec::with_capacity(parsed_makefile.asm_sources.len());
        for source in parsed_makefile.asm_sources.iter() {
            files.push(EIDEProjectFile { path: source });
        }

        let project_name = parsed_makefile.target.unwrap_or("".to_string());

        // list dir
        let mut src = Vec::new();
        let path = Path::new(".");
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = path.file_name() {
                    if let Some(name_str) = name.to_str() {
                        if !name_str.starts_with('.') {
                            src.push(name_str.to_string());
                        }
                    }
                }
            }
        }
        let mut includes = parsed_makefile.includes;
        includes.push("UserCode".to_string());

        let ctx = EIDEConfigContext {
            project_name: &project_name,
            ld_file_path: &parsed_makefile.ldscript.unwrap_or_default(),
            src_dirs: &serde_json::to_string(&src)?,
            include_list: &serde_json::to_string(&includes)?,
            define_list: &serde_json::to_string(&parsed_makefile.defines)?,
            src_files: &serde_json::to_string(&files)?,
            floating_point_hardware: match args.fpu {
                FPUType::Hard => "single",
                FPUType::Soft => "none",
            },
            fpu_type: match args.fpu {
                FPUType::Hard => "hard",
                FPUType::Soft => "soft",
            },
        };

        info!("Generating EIDE config file...");
        render_file(".eide/eide.json", EIDE_CONFIG, &ctx, force)?;
        info!("Generating EIDE workspace file...");
        render_file(
            format!("{project_name}.code-workspace").as_str(),
            EIDE_WORKSPACE,
            &ctx,
            force,
        )?;

        Ok(())
    }
}
