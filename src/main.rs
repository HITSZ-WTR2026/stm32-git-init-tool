mod generate_gitignore;
mod patches;
mod render;
mod stm32cubemx;
mod templates;
mod utils;

use crate::generate_gitignore::generate_gitignore;
use crate::patches::{apply_patch, Patch};
use crate::render::render_file;
use crate::stm32cubemx::{generate_code, Toolchain};
use crate::templates::{APP_C, APP_H, CLANG_FORMAT, README_MD};
use crate::utils::get_author;
use chrono::Local;
use clap::{Parser, ValueEnum};
use serde::Serialize;
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};
use tracing::{error, info, warn};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum FPUType {
    Hard,
    Soft,
}

#[derive(Parser)]
struct Cli {
    /// 跳过生成 UserCode 目录结构
    #[arg(long, default_value_t = false)]
    skip_generate_user_code: bool,
    /// 跳过生成 .clang-format
    #[arg(long, default_value_t = false)]
    skip_generate_clang_format: bool,
    /// 跳过非侵入式头文件配置
    ///
    /// 只有当 skip_generate_user_code 未启用时生效
    #[arg(
        long,
        requires_if("false", "skip_generate_user_code"),
        default_value_t = false
    )]
    skip_non_intrusive_headers: bool,
    /// 选择 FPU 类型
    #[arg(long, short, default_value = "hard")]
    fpu: FPUType,
    /// 强制重新生成
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

    // 初始化项目配置
    info!("Initializing git repository...");
    let status = Command::new("git")
        .arg("init")
        .stdout(Stdio::null()) // 屏蔽 stdout
        .stderr(Stdio::null()) // 屏蔽 stderr
        .status();
    match status {
        Ok(status) if status.success() => {
            info!("Git repository initialized successfully!");
        }
        Ok(status) => {
            error!("Git init failed with status: {}", status);
        }
        Err(e) => {
            error!("Failed to execute git: {}", e);
        }
    }
    info!("Generating .gitignore file...");
    generate_gitignore(None, cli.force)?;

    if !cli.skip_generate_clang_format {
        info!("Generating .clang-format file");
        render_file(".clang-format", CLANG_FORMAT, &ctx, cli.force)?;
    }

    if !cli.skip_generate_user_code {
        info!("Generating user code directories...");
        let directories: Vec<&str> = vec![
            "UserCode/bsp",
            "UserCode/drivers",
            "UserCode/third_party",
            "UserCode/libs",
            "UserCode/interfaces",
            "UserCode/controllers",
            "UserCode/app",
        ];
        for dir in directories {
            fs::create_dir_all(dir)?;
            info!("Created dir {}", dir);
        }
        render_file("UserCode/app/app.h", APP_H, &ctx, cli.force)?;
        render_file("UserCode/app/app.c", APP_C, &ctx, cli.force)?;
        render_file("UserCode/README.md", README_MD, &ctx, cli.force)?;
    }

    if !cli.skip_non_intrusive_headers {
        if cli.skip_generate_user_code {
            info!("Skipping non-intrusive headers due to skip_generate_user_code");
        } else {
            info!("Generating non-intrusive headers");
            apply_patch(
                &Patch::Append {
                    file: "CMakeLists_template.txt".to_string(),
                    after: "add_executable".to_string(),
                    insert: "\n# 非侵入式引入头文件\ntarget_compile_options(${PROJECT_NAME}.elf PRIVATE -include ${CMAKE_SOURCE_DIR}/UserCode/app/app.h)\n".to_string(),
                    marker: "UserCode/app/app.h".to_string(),
                })?;
            apply_patch(&Patch::Append {
                file: "Makefile".to_string(),
                after: "CFLAGS += $(MCU)".to_string(),
                insert: "\n# 非侵入式引入头文件\nCFLAGS += -include UserCode/app/app.h\n"
                    .to_string(),
                marker: "UserCode/app/app.h".to_string(),
            })?;
        }
    }

    if Path::new("CMakeLists_template.txt").exists() {
        info!("Found `CMakeLists_template.txt`, initializing CLion project...");
        apply_patch(&Patch::Replace {
            file: "CMakeLists_template.txt".to_string(),
            find: "include_directories(${includes})".to_string(),
            insert: "include_directories(${includes} UserCode)".to_string(),
        })?;
        apply_patch(&Patch::Replace {
            file: "CMakeLists_template.txt".to_string(),
            find: "file(GLOB_RECURSE SOURCES ${sources})".to_string(),
            insert: "file(GLOB_RECURSE SOURCES ${sources} \"UserCode/*.*\")".to_string(),
        })?;
        match cli.fpu {
            FPUType::Hard => apply_patch(&Patch::RegexReplace {
                file: "CMakeLists_template.txt".to_string(),
                pattern: "(?ms)^#Uncomment for hardware floating point(?:\n#.*?)*\n?(?:\n|$)"
                    .to_string(),
                insert: "${0/#/}".to_string(),
            }),
            FPUType::Soft => apply_patch(&Patch::RegexReplace {
                file: "CMakeLists_template.txt".to_string(),
                pattern: "(?ms)^#Uncomment for hardware floating point(?:\n#.*?)*\n?(?:\n|$)"
                    .to_string(),
                insert: "${0/#/}".to_string(),
            }),
        }?;
        info!("Try to regenerate code(using STM32CubeMX)...");
        match generate_code(Some(Toolchain::STM32CubeIDE)) {
            Ok(_) => {
                info!("Regenerate code successfully!")
            }
            Err(_) => {
                warn!("Regenerate code failed, please regenerate code manually!");
            }
        };
    }

    info!("STM32 project initialized!");
    Ok(())
}
