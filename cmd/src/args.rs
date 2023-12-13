use clap::{Parser, Subcommand, ValueEnum};
use houdini_downloader_api::{Platform, Product};
use std::ops::Not;
use std::path::PathBuf;

/// Utility for downloading SideFX Houdini installers.
#[derive(Debug, Parser)]
#[clap(disable_help_subcommand = true)]
#[command(author, version)]
pub struct Args {
    #[command(subcommand)]
    pub commands: Commands,
    #[arg(long, global = true, env = "SESI_USER_ID", hide_env_values = true)]
    pub user_id: Option<String>,
    #[arg(long, global = true, env = "SESI_USER_SECRET", hide_env_values = true)]
    pub user_secret: Option<String>,
    #[arg(long, global = true, value_enum, default_value_t = ProductArg::Houdini)]
    pub product: ProductArg,
    #[arg(long, global = true, value_enum, default_value_t = PlatformArg::default())]
    pub platform: PlatformArg,
}

impl Args {
    pub fn parse_() -> Self {
        Args::parse()
    }
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Download a particular build.
    Get {
        /// Product version [e.g. 19.5]
        #[arg(short, long)]
        version: String,

        /// Product build number.
        #[arg(short, long)]
        build: u64,

        /// Directory to save the downloaded file.
        #[arg(short, long)]
        output_dir: PathBuf,

        /// Skip download confirmation question.
        #[arg(short, long)]
        auto_confirm: bool,

        /// Overwrite if file exists in the output directory.
        #[arg(long)]
        overwrite: bool,
    },
    /// List available builds.
    List {
        /// By default, only production builds are listed.
        #[arg(short, long, default_value_t = false)]
        include_daily_builds: bool,
        /// Product version [e.g. 19.5]
        #[arg(short, long)]
        version: String,
    },
}

impl Commands {
    pub fn is_version_valid(&self) -> bool {
        // Verify if version is major.minor
        let version = match self {
            Commands::Get { version, .. } => version,
            Commands::List { version, .. } => version,
        };
        version.ends_with('.').not() && version.split('.').count() == 2
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
pub enum ProductArg {
    Houdini,
    HoudiniLauncher,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
pub enum PlatformArg {
    Linux,
    Win64,
    Macos,
    MacosxArm64,
}

impl Default for PlatformArg {
    fn default() -> Self {
        if cfg!(target_os = "windows") {
            PlatformArg::Win64
        } else if cfg!(target_os = "linux") {
            PlatformArg::Linux
        } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
            PlatformArg::Macos
        } else if cfg!(all(target_os = "macos", target_os = "aarch64")) {
            PlatformArg::MacosxArm64
        } else {
            panic!("Unsupported platform");
        }
    }
}

impl From<ProductArg> for Product {
    fn from(arg: ProductArg) -> Self {
        match arg {
            ProductArg::Houdini => Product::Houdini,
            ProductArg::HoudiniLauncher => Product::HoudiniLauncher,
        }
    }
}

impl From<PlatformArg> for Platform {
    fn from(arg: PlatformArg) -> Self {
        match arg {
            PlatformArg::Linux => Platform::Linux,
            PlatformArg::Win64 => Platform::Win64,
            PlatformArg::Macos => Platform::Macos,
            PlatformArg::MacosxArm64 => Platform::MacosxArm64,
        }
    }
}
