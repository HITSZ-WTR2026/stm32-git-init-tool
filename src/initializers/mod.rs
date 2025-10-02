use crate::initializers::clion::CLion;
use crate::initializers::eide::EIDE;
use clap::{Parser, ValueEnum};
use tracing::info;

mod clion;
mod eide;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum FPUType {
    Hard,
    Soft,
}

#[derive(Parser, Debug)]
pub struct IdeInitArgs {
    /// 选择 FPU 类型
    #[arg(long, short, default_value = "hard")]
    fpu: FPUType,
}

pub trait IdeInitializer {
    fn name(&self) -> &'static str;
    fn init(&self, args: &IdeInitArgs, force: bool) -> anyhow::Result<()>;
}

pub fn all() -> Vec<Box<dyn IdeInitializer>> {
    vec![Box::new(CLion), Box::new(EIDE)]
}

pub struct IdeNone;
impl IdeInitializer for IdeNone {
    fn name(&self) -> &'static str {
        "None"
    }

    fn init(&self, _args: &IdeInitArgs, _force: bool) -> anyhow::Result<()> {
        info!("No IDE initializer selected");
        Ok(())
    }
}
