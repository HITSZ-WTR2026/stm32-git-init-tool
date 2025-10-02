mod generate_gitignore;
mod initializers;
mod patches;
mod render;
mod stm32cubemx;
mod utils;

use crate::contexts::{CreateContext, EIDEConfigContext};
use crate::generate_gitignore::generate_gitignore;
use crate::initializers::IdeInitArgs;
use crate::patches::{apply_patch, Patch};
use crate::render::{render_file, render_string};
use crate::stm32cubemx::{generate_code, get_toolchain, run_script, Toolchain};
use crate::utils::get_author;
use anyhow::anyhow;
use chrono::Local;
use clap::{Parser, Subcommand};
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Confirm, MultiSelect};
use serde::Serialize;
use std::path::Path;
use std::process::{Command, Stdio};
use std::{env, fs};
use tracing::{error, info};

#[derive(Subcommand)]
enum Commands {
    /// 初始化 STM32 项目
    Init(InitArgs),

    /// 创建新项目
    Create(CreateArgs),
}

#[derive(Parser, Debug)]
struct CreateArgs {
    /// 项目名
    project_name: String,

    /// 使用的工具链
    #[clap(short, long)]
    #[arg(default_value = "stm32cubeide")]
    toolchain: Toolchain,

    /// 是否在创建后立即初始化项目
    #[arg(long)]
    run_init: bool,

    /// 使用 init 的参数
    #[command(flatten)]
    init_args: InitArgs,
}

#[derive(Parser, Debug)]
struct InitArgs {
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
    /// 强制重新生成
    #[arg(long)]
    force: bool,
    #[command(flatten)]
    init_args: IdeInitArgs,
}

#[derive(Parser)]
#[command(name = "stm32-project-tool")]
#[command(about = "STM32 project helper tool", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Serialize)]
struct InitContext {
    author: String,
    date: String,
    year: String,
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    match cli.command {
        Commands::Init(args) => {
            run_init(args)?;
        }
        Commands::Create(args) => {
            run_create(args)?;
        }
    }

    Ok(())
}

fn run_init(args: InitArgs) -> anyhow::Result<()> {
    let ides = initializers::all();
    let items: Vec<&str> = ides.iter().map(|i| i.name()).collect();

    let chosen = MultiSelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Select IDEs to initialize")
        .items(&items)
        .interact()?;

    // 渲染上下文
    let author = get_author();

    let now = Local::now();
    let ctx = InitContext {
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
    generate_gitignore(None, args.force)?;

    if !args.skip_generate_clang_format {
        info!("Generating .clang-format file");
        render_file(
            ".clang-format",
            include_str!("templates/clang-format.tmpl"),
            &ctx,
            args.force,
        )?;
    }

    if !args.skip_generate_user_code {
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
        render_file(
            "UserCode/app/app.h",
            include_str!("templates/app.h.tmpl"),
            &ctx,
            args.force,
        )?;
        render_file(
            "UserCode/app/app.c",
            include_str!("templates/app.c.tmpl"),
            &ctx,
            args.force,
        )?;
        render_file(
            "UserCode/README.md",
            include_str!("templates/README.md.tmpl"),
            &ctx,
            args.force,
        )?;
    }

    for idx in chosen {
        let _ = ides[idx].init(&args.init_args, args.force);
    }

    if !args.skip_non_intrusive_headers {
        if args.skip_generate_user_code {
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

    let status = Command::new("git").args(&["add", "."]).status()?;
    if status.success() {
        let status = Command::new("git")
            .args(&["commit", "-m", "Initial commit"])
            .status()?;
        if !status.success() {
            error!("Git first commit failed");
        }
    } else {
        error!("Git first commit failed");
    }

    info!("STM32 project initialized!");
    Ok(())
}

#[derive(Serialize)]
pub struct CreateContext<'a> {
    pub project_name: &'a String,
    pub project_dir: &'a String,
    pub ioc_file_path: &'a String,
    pub toolchain: &'a str,
    pub generate_under_root: bool,
}
fn run_create(args: CreateArgs) -> anyhow::Result<()> {
    let path = Path::new(&args.project_name);
    if path.exists() {
        let result = Confirm::new()
            .with_prompt(
                "Project already exists. Regenerate? This will delete all existing content.",
            )
            .default(false) // false 对应 [y/N] 的 N
            .interact()?;
        if !result {
            info!("Creation aborted!");
            return Err(anyhow!("Creation aborted!"));
        }
        fs::remove_dir_all(path)?;
    }
    fs::create_dir_all(&args.project_name)?;
    env::set_current_dir(&args.project_name)?;
    let current_dir = env::current_dir()?;

    let ctx = CreateContext {
        project_name: &args.project_name,
        project_dir: &current_dir.to_string_lossy().to_string(),
        ioc_file_path: &current_dir
            .join(format!("{}.ioc", args.project_name))
            .to_string_lossy()
            .to_string(),
        toolchain: get_toolchain(&args.toolchain),
        generate_under_root: args.toolchain == Toolchain::STM32CubeIDE,
    };
    info!("Using toolchain {}", get_toolchain(&args.toolchain));

    // 渲染初次运行的脚本
    let script = render_string(include_str!("templates/create-project-cmd1.tmpl"), &ctx)?;
    info!("Running first script");
    match run_script(script) {
        Ok(_) => {}
        Err(e) => {
            error!("Failed to run first script: {}", e);
            return Err(anyhow!("Failed to run first script: {}", e));
        }
    };
    info!("Patching .ioc file");
    apply_patch(&Patch::RegexReplace {
        file: format!("{}.ioc", args.project_name),
        pattern: r"RCC\.HSE_VALUE=(\d+)".to_string(),
        insert: "RCC.HSE_VALUE=8000000".to_string(),
    })?;
    // 渲染第二次运行的脚本
    let script = render_string(include_str!("templates/create-project-cmd2.tmpl"), &ctx)?;
    info!("Running second script");
    match run_script(script) {
        Ok(_) => {}
        Err(e) => {
            error!("Failed to run second script: {}", e);
            return Err(anyhow!("Failed to run second script: {}", e));
        }
    };

    if args.run_init {
        info!("Running init process");
        run_init(args.init_args)?;
    }
    Ok(())
}
