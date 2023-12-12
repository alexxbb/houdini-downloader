#![allow(unused)]
#![allow(dead_code)]
use anyhow::{anyhow, Result};
use clap::{ArgAction, CommandFactory, Parser, Subcommand, ValueEnum};
use dialoguer::Confirm;
use downloader_api::{ListBuildsParms, Platform, Product, SesiClient};
use futures_util::StreamExt;
use owo_colors::{AnsiColors, OwoColorize};
use reqwest;
use std::io::Write;
use tokio::io::AsyncWriteExt;

#[derive(Debug, Parser)]
struct App {
    #[command(subcommand)]
    commands: Commands,
    #[arg(long, global = true, env = "SESI_USER_ID")]
    user_id: Option<String>,
    #[arg(long, global = true, env = "SESI_USER_SECRET")]
    user_secret: Option<String>,
    #[arg(long, global = true, value_enum, default_value_t = ProductArg::Houdini)]
    product: ProductArg,
    #[arg(long, global = true, value_enum, default_value_t = PlatformArg::Linux)]
    platform: PlatformArg,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Get {
        /// Product version [e.g. 19.5]
        #[arg(short, long)]
        version: String,

        /// Product build number
        #[arg(short, long)]
        build: u64,
    },
    List {
        /// List only production builds.
        #[arg(short, long, default_value_t = false)]
        include_daily_builds: bool,
        /// Product version [e.g. 19.5]
        #[arg(short, long)]
        version: String,
    },
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum ProductArg {
    Houdini,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum PlatformArg {
    Linux,
    Win64,
    Macos,
    MacosxArm64,
}

impl From<ProductArg> for Product {
    fn from(arg: ProductArg) -> Self {
        match arg {
            ProductArg::Houdini => Product::Houdini,
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

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let args: App = App::parse();
    if args.user_id.is_none() || args.user_secret.is_none() {
        return Err(anyhow!("SESI_USER_ID and SESI_USER_SECRET are required"));
    }

    // None variants were checked above
    let user_id = args.user_id.as_deref().unwrap();
    let user_secret = args.user_secret.as_deref().unwrap();

    let client = SesiClient::new(user_id, user_secret).await?;

    match args.commands {
        Commands::Get { version, build } => {
            let build_info = client
                .get_download_url(args.product.into(), args.platform.into(), version, build)
                .await?;
            let filename = &build_info.filename;
            let confirmation = Confirm::new()
                .with_prompt(format!("Download {filename}?"))
                .interact_opt()?;
            match confirmation {
                None => return Ok(()),
                Some(inp) if !inp => return Ok(()),
                _ => {}
            }
            let response = reqwest::get(build_info.download_url).await?;
            let mut stream = response.bytes_stream();
            let mut file = tokio::fs::File::create("c:/Temp/houdini-install.exe").await?;
            let total = build_info.size as usize;
            let mut downloaded: usize = 0;
            while let Some(item) = stream.next().await {
                let chunk = item?;
                downloaded += chunk.len();
                file.write_all(&chunk).await?;
                println!("Downloaded {downloaded}/{total}")
            }
        }
        Commands::List {
            include_daily_builds,
            version,
        } => {
            let mut stdout = std::io::stdout().lock();
            for (i, build) in client
                .list_builds(
                    args.product.into(),
                    args.platform.into(),
                    version,
                    !include_daily_builds,
                )
                .await?
                .into_iter()
                .enumerate()
            {
                let status = if build.status == "bad" {
                    build.status.color(AnsiColors::Red)
                } else {
                    build.status.color(AnsiColors::Green)
                };
                writeln!(
                    stdout,
                    "{i:>2}. Build date: {}, version: {}, status: {}, release: {}",
                    build.date,
                    build.full_version(),
                    status,
                    build.release
                );
            }
            drop(stdout);
        }
    }

    Ok(())
}
