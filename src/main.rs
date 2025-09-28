mod contexts;
mod generate_gitignore;
mod patches;
mod render;
mod stm32cubemx;
mod templates;
mod utils;

use crate::contexts::{CreateContext, EIDEConfigContext};
use crate::generate_gitignore::generate_gitignore;
use crate::patches::{apply_patch, Patch};
use crate::render::{render_file, render_string};
use crate::stm32cubemx::{generate_code, get_toolchain, run_script, Toolchain};
use crate::templates::{
    APP_C, APP_H, CLANG_FORMAT, CREATE_PROJECT_CMD1, CREATE_PROJECT_CMD2, EIDE_CONFIG,
    EIDE_WORKSPACE, README_MD,
};
use crate::utils::get_author;
use anyhow::anyhow;
use chrono::Local;
use clap::{Parser, Subcommand, ValueEnum};
use dialoguer::{Confirm, Select};
use serde::Serialize;
use std::path::Path;
use std::process::{Command, Stdio};
use std::{env, fs};
use tracing::{error, info, warn};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum FPUType {
    Hard,
    Soft,
}

#[derive(Subcommand)]
enum Commands {
    /// 初始化 STM32 项目
    Init(InitArgs),

    /// 创建新项目
    Create {
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
    },
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
    /// 选择 FPU 类型
    #[arg(long, short, default_value = "hard")]
    fpu: FPUType,
    /// 强制重新生成
    #[arg(long)]
    force: bool,
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
            run_init(
                args.skip_generate_user_code,
                args.skip_generate_clang_format,
                args.skip_non_intrusive_headers,
                args.fpu,
                args.force,
            )?;
        }
        Commands::Create {
            project_name,
            toolchain,
            run_init,
            init_args,
        } => {
            run_create(project_name, toolchain, run_init, init_args)?;
        }
    }

    Ok(())
}

fn run_init(
    skip_generate_user_code: bool,
    skip_generate_clang_format: bool,
    skip_non_intrusive_headers: bool,
    fpu: FPUType,
    force: bool,
) -> std::io::Result<()> {
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
    generate_gitignore(None, force)?;

    if !skip_generate_clang_format {
        info!("Generating .clang-format file");
        render_file(".clang-format", CLANG_FORMAT, &ctx, force)?;
    }

    if !skip_generate_user_code {
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
        render_file("UserCode/app/app.h", APP_H, &ctx, force)?;
        render_file("UserCode/app/app.c", APP_C, &ctx, force)?;
        render_file("UserCode/README.md", README_MD, &ctx, force)?;
    }

    if !skip_non_intrusive_headers {
        if skip_generate_user_code {
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
        clion_custom_init(fpu)?;
    }
    if Path::new("Makefile").exists() {
        info!("Found `Makefile`, initializing Makefile project...");
        let choice = Select::new()
            .with_prompt("Choose your ide")
            .item("VSCode + EIDE")
            .item("None")
            .default(0)
            .interact()?;
        match choice {
            0_usize => eide_custom_init(force)?,
            1_usize => {
                warn!("--");
            }
            2_usize.. => todo!(),
        }
    }

    info!("STM32 project initialized!");
    Ok(())
}

#[derive(Serialize)]
struct EIDEProjectFile<'a> {
    path: &'a String,
}

fn eide_custom_init(force: bool) -> std::io::Result<()> {
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

fn clion_custom_init(fpu: FPUType) -> std::io::Result<()> {
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
    match fpu {
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
    Ok(())
}

fn run_create(
    project_name: String,
    toolchain: Toolchain,
    run_init_: bool,
    init_args: InitArgs,
) -> anyhow::Result<()> {
    let path = Path::new(&project_name);
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
    fs::create_dir_all(&project_name)?;
    env::set_current_dir(&project_name)?;
    let current_dir = env::current_dir()?;

    let ctx = CreateContext {
        project_name: &project_name,
        project_dir: &current_dir.to_string_lossy().to_string(),
        ioc_file_path: &current_dir
            .join(format!("{project_name}.ioc"))
            .to_string_lossy()
            .to_string(),
        toolchain: get_toolchain(&toolchain),
        generate_under_root: toolchain == Toolchain::STM32CubeIDE,
    };
    info!("Using toolchain {}", get_toolchain(&toolchain));

    // 渲染初次运行的脚本
    let script = render_string(CREATE_PROJECT_CMD1, &ctx)?;
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
        file: format!("{project_name}.ioc"),
        pattern: r"RCC\.HSE_VALUE=(\d+)".to_string(),
        insert: "RCC.HSE_VALUE=8000000".to_string(),
    })?;
    // 渲染第二次运行的脚本
    let script = render_string(CREATE_PROJECT_CMD2, &ctx)?;
    info!("Running second script");
    match run_script(script) {
        Ok(_) => {}
        Err(e) => {
            error!("Failed to run second script: {}", e);
            return Err(anyhow!("Failed to run second script: {}", e));
        }
    };

    if run_init_ {
        info!("Running init process");
        run_init(
            init_args.skip_generate_user_code,
            init_args.skip_generate_clang_format,
            init_args.skip_non_intrusive_headers,
            init_args.fpu,
            init_args.force,
        )?;
    }
    Ok(())
}
