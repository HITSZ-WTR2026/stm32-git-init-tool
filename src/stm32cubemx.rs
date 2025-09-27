use rand::distr::Alphanumeric;
use rand::{rng, Rng};
use std::fs;
use std::fs::{remove_file, File};
use std::io::Write;
use std::process::{Command, Stdio};
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

pub fn generate_code(toolchain: Option<Toolchain>) -> Result<(), ()> {
    let ioc_files = get_ioc_files();
    if ioc_files.len() != 1 {
        warn!("No ioc file is provided or multiple ioc files are provided.");
        return Err(());
    }
    let ioc_file = ioc_files.first().unwrap();
    let tmp_path = format!("./tmp-script-{}", generate_random_string(8));
    {
        let mut temp_script_file = File::create_new(&tmp_path).unwrap();
        temp_script_file
            .write_all(format!("config load {}\n", ioc_file).as_bytes())
            .unwrap();
        if let Some(toolchain) = toolchain {
            temp_script_file
                .write_all(
                    format!("project toolchain \"{}\"\n", get_toolchain(&toolchain)).as_bytes(),
                )
                .unwrap();
            if let Toolchain::STM32CubeIDE = toolchain {
                // Generate Under Root on
                temp_script_file
                    .write_all("project generateunderroot 1".as_bytes())
                    .unwrap();
            }
        }
        // Generate peripheral initialization as a pair of '.c/.h' files per peripheral
        temp_script_file
            .write_all("project couplefilesbyip 1\n".as_bytes())
            .unwrap();
        temp_script_file
            .write_all("project generate\n".as_bytes())
            .unwrap();
        temp_script_file.write_all("exit".as_bytes()).unwrap();
    }
    let status = if cfg!(target_os = "windows") {
        return Err(());
    } else {
        Command::new("stm32cubemx")
            .arg("-s")
            .arg(&tmp_path)
            .stdout(Stdio::null()) // 屏蔽 stdout
            .stderr(Stdio::null()) // 屏蔽 stderr
            .arg("-q")
            .status()
    };
    remove_file(tmp_path).unwrap();
    match status {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => {
            error!("Generate failed with status: {}", status);
            Err(())
        }
        Err(e) => {
            error!("Failed to execute stm32cubemx: {}", e);
            Err(())
        }
    }
}
