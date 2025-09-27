use anyhow::Result;
use rand::distr::Alphanumeric;
use rand::{rng, Rng};
use std::env::VarError;
use std::fmt::Write;
use std::fs::{remove_file, File};
use std::io::Write as IoWrite;
use std::process::{Command, Stdio};
use std::{env, fs};
use tracing::{error, warn};

fn generate_random_string(length: usize) -> String {
    let mut rng = rng();
    (0..length)
        .map(|_| rng.sample(Alphanumeric))
        .map(char::from)
        .collect()
}

fn get_ioc_files() -> Vec<String> {
    let mut ioc_files: Vec<String> = Vec::new();
    let current_dir = std::env::current_dir().expect("Failed to get current directory");
    if let Ok(entries) = fs::read_dir(current_dir) {
        for entry in entries {
            if let Ok(entry) = entry {
                let path = entry.path();
                if let Some(extension) = path.extension() {
                    if extension == "ioc" {
                        ioc_files.push(path.to_str().unwrap().to_string());
                    }
                }
            }
        }
    }
    ioc_files
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Toolchain {
    /// EWARM V8.32
    EwarmV832,
    /// EWARM V8
    EwarmV800,
    /// EWARM V7
    EwarmV700,
    /// MDK-ARM V5.32
    MdmArmV532,
    /// MDK-ARM V5.27
    MdmArmV527,
    /// MDK-ARM V5
    MdmArmV500,
    /// MDK-ARM V4
    MdmArmV400,
    /// STM32CubeIDE
    STM32CubeIDE,
    /// Makefile
    Makefile,
    /// CMake
    CMake,
}

fn get_toolchain(toolchain: &Toolchain) -> &'static str {
    match toolchain {
        Toolchain::EwarmV832 => "EWARM V8.32",
        Toolchain::EwarmV800 => "EWARM V8",
        Toolchain::EwarmV700 => "EWARM V7",
        Toolchain::MdmArmV532 => "MDK-ARM V5.32",
        Toolchain::MdmArmV527 => "MDK-ARM V5.27",
        Toolchain::MdmArmV500 => "MDK-ARM V5",
        Toolchain::MdmArmV400 => "MDK-ARM V4",
        Toolchain::STM32CubeIDE => "STM32CubeIDE",
        Toolchain::Makefile => "Makefile",
        Toolchain::CMake => "CMake",
    }
}

pub fn generate_code(toolchain: Option<Toolchain>) -> Result<()> {
    let ioc_files = get_ioc_files();
    if ioc_files.len() != 1 {
        warn!("No ioc file is provided or multiple ioc files are provided.");
        return Err(anyhow::anyhow!(
            "No ioc file is provided or multiple ioc files are provided."
        ));
    }
    let ioc_file = ioc_files.first().unwrap();
    let mut script = String::new();
    write!(script, "config load {}\n", ioc_file)?;
    if let Some(toolchain) = toolchain {
        write!(
            script,
            "project toolchain \"{}\"\n",
            get_toolchain(&toolchain)
        )?;
        if let Toolchain::STM32CubeIDE = toolchain {
            // Generate Under Root on
            write!(script, "project generateunderroot 1")?;
        }
    }
    // Generate peripheral initialization as a pair of '.c/.h' files per peripheral
    write!(script, "project couplefilesbyip 1\n")?;
    write!(script, "project generate\n")?;
    write!(script, "exit")?;

    run_script(script)
}

pub fn run_script(script: String) -> Result<()> {
    let tmp_path = format!("./tmp-script-{}", generate_random_string(8));
    let mut temp_script_file = File::create_new(&tmp_path)?;
    temp_script_file.write_all(script.as_bytes())?;
    let status = if cfg!(target_os = "windows") {
        // return Err(anyhow::anyhow!("not support windows"));
        let dir = match env::var("STM32CubeMX_dir") {
            Ok(dir) => dir,
            Err(_) => {
                error!(
                    "Environment variable STM32CubeMX_dir is not set. Please configure the STM32CubeMX installation path."
                );
                return Err(anyhow::anyhow!(
                    "Missing environment variable: STM32CubeMX_dir"
                ));
            }
        };
        Command::new("cmd")
            .args([
                "/C",
                format!("{dir}\\jre\\bin\\java.exe -jar {dir}\\STM32CubeMX.exe -s {tmp_path} -q")
                    .as_str(),
            ])
            .status()
    } else {
        Command::new("stm32cubemx")
            .arg("-s")
            .arg(&tmp_path)
            .stdout(Stdio::null()) // 屏蔽 stdout
            .stderr(Stdio::null()) // 屏蔽 stderr
            .arg("-q")
            .status()
    };
    remove_file(tmp_path)?;
    match status {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => {
            error!("Run script failed with status: {}", status);
            Err(anyhow::anyhow!("Run script failed with status: {}", status))
        }
        Err(e) => {
            error!("Failed to execute stm32cubemx: {}", e);
            Err(anyhow::anyhow!("Failed to execute stm32cubemx: {}", e))
        }
    }
}
