#![allow(unused)]
#![allow(dead_code)]
use anyhow::{anyhow, bail, Context, Result};
use clap::{ArgAction, CommandFactory, Parser, Subcommand, ValueEnum};
use dialoguer::{theme::ColorfulTheme, Confirm};
use downloader_api::{ListBuildsParms, Platform, Product, SesiClient};
use futures_util::StreamExt;
use indicatif::ProgressStyle;
use md5::{Digest, Md5};
use owo_colors::{AnsiColors, OwoColorize};
use reqwest;
use std::io::Write;
use std::path::PathBuf;
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
    ctrlc::set_handler(move || {
        std::process::exit(0);
    });
    let args: App = App::parse();
    if args.user_id.is_none() || args.user_secret.is_none() {
        bail!("SESI_USER_ID and SESI_USER_SECRET are required");
    }

    // None variants were checked above
    let user_id = args.user_id.as_deref().unwrap();
    let user_secret = args.user_secret.as_deref().unwrap();

    let client = SesiClient::new(user_id, user_secret).await?;

    match args.commands {
        Commands::Get {
            version,
            build,
            output_dir,
            auto_confirm,
            overwrite,
        } => {
            let build_info = client
                .get_download_url(args.product.into(), args.platform.into(), version, build)
                .await?;
            let filename = &build_info.filename;
            let output = output_dir.join(filename);
            if !overwrite && output.exists() {
                bail!("File already downloaded: {}", output.to_string_lossy());
            }
            if !auto_confirm {
                let confirmation = Confirm::with_theme(&ColorfulTheme::default())
                    .with_prompt(format!("Download {filename}?"))
                    .interact_opt()?;
                match confirmation {
                    None => return Ok(()),
                    Some(inp) if !inp => return Ok(()),
                    _ => {}
                }
            }
            let response = reqwest::get(build_info.download_url)
                .await
                .context("Could not send GET download request")?;
            let mut stream = response.bytes_stream();
            let pb = indicatif::ProgressBar::new(build_info.size);
            pb.set_style(
                ProgressStyle::default_bar()
                    .template(
                        "{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] \
                            {bytes}/{total_bytes} ({binary_bytes_per_sec}, {eta})",
                    )?
                    .progress_chars("#>-"),
            );
            pb.set_message(format!("Downloading {}", filename));
            let mut file = tokio::fs::File::create(&output)
                .await
                .context("Could not create file to save")?;
            let mut hash = Md5::new();
            while let Some(chunk) = stream.next().await {
                if let Ok(bytes) = chunk {
                    file.write_all(&bytes)
                        .await
                        .context("Error writing to file")?;
                    hash.update(&bytes);
                    pb.inc(bytes.len() as u64);
                }
            }
            pb.finish_with_message(format!("Downloaded: {}", output.to_string_lossy()));
            let downloaded_bytes_hash = hex::encode(&hash.finalize()[..]);
            if (downloaded_bytes_hash != build_info.hash) {
                eprintln!(
                    "{}",
                    "[warning]: Downloaded file hash is different from the build hash"
                        .color(AnsiColors::Red)
                )
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
