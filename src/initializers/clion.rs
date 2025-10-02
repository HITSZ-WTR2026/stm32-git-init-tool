use crate::initializers::{FPUType, IdeInitArgs, IdeInitializer};
use crate::patches::{apply_patch, Patch};
use crate::stm32cubemx::{generate_code, Toolchain};
use anyhow::anyhow;
use std::path::Path;
use tracing::{error, info, warn};

pub struct CLion;

impl IdeInitializer for CLion {
    fn name(&self) -> &'static str {
        "CLion (toolchain: STM32CubeIDE)"
    }

    fn init(&self, args: &IdeInitArgs, _force: bool) -> anyhow::Result<()> {
        info!("Initializing CLion project...");

        if !Path::new("CMakeLists_template.txt").exists() {
            error!("CMakeLists_template.txt is not exists, initialization failed");
            return Err(anyhow!(
                "CMakeLists_template.txt is not exists, initialization failed"
            ));
        }

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
        match args.fpu {
            FPUType::Hard => apply_patch(&Patch::UncommentBlock {
                file: "CMakeLists_template.txt".to_string(),
                marker: "#Uncomment for hardware floating point".to_string(),
            }),
            FPUType::Soft => apply_patch(&Patch::UncommentBlock {
                file: "CMakeLists_template.txt".to_string(),
                marker: "#Uncomment for software floating point".to_string(),
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
}
