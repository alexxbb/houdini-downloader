mod args;

use crate::args::{Args, Commands};
use anyhow::{bail, Context, Result};
use dialoguer::{theme::ColorfulTheme, Confirm};
use futures_util::StreamExt;
use houdini_downloader_api::SesiClient;
use indicatif::ProgressStyle;
use md5::{Digest, Md5};
use owo_colors::{AnsiColors, OwoColorize};
use std::io::Write;
use tokio::io::{AsyncWriteExt, BufWriter};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let args: Args = Args::parse_();

    if args.user_id.is_none() || args.user_secret.is_none() {
        bail!("SESI_USER_ID and SESI_USER_SECRET are required");
    }

    if !args.commands.is_version_valid() {
        bail!("Version number must be major.minor [e.g 19.5]")
    }

    // None variants were checked above
    let user_id = args.user_id.as_deref().unwrap();
    let user_secret = args.user_secret.as_deref().unwrap();

    ctrlc::set_handler(move || {
        println!("Killed with CTRL-C");
        std::process::exit(0);
    })
    .context("Error setting up CTRL-C handler")?;

    let client = SesiClient::new(user_id, user_secret)
        .await
        .context("Error encountered while trying to authorize with SideFX")?;

    match args.commands {
        Commands::Get {
            version,
            build,
            output_dir,
            silent,
            overwrite,
        } => {
            let build_info = client
                .get_build_url(args.product.into(), args.platform.into(), version, build)
                .await
                .context("Error encountered while trying to get build info")?;
            let filename = &build_info.filename;
            let output = output_dir.join(filename);
            if !overwrite && output.exists() {
                eprintln!("File already downloaded: {}", output.to_string_lossy());
                return Ok(());
            }
            if !silent {
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
            let downloading_started_msg = format!("Downloading {}", filename);
            let bar = if !silent {
                let bar = indicatif::ProgressBar::new(build_info.size);
                bar.set_style(
                    ProgressStyle::default_bar()
                        .template(
                            "{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] \
                            {bytes}/{total_bytes} ({binary_bytes_per_sec}, {eta})",
                        )?
                        .progress_chars("#>-"),
                );
                bar.set_message(downloading_started_msg);
                Some(bar)
            } else {
                println!("{}", downloading_started_msg);
                None
            };
            let file = tokio::fs::File::create(&output)
                .await
                .context("Could not create file to save")?;
            let mut file_buf = BufWriter::new(file);
            let mut stream = response.bytes_stream();
            let mut hash = Md5::new();
            while let Some(chunk) = stream.next().await {
                if let Ok(bytes) = chunk {
                    file_buf
                        .write_all(&bytes)
                        .await
                        .context("Error writing to output file")?;
                    hash.update(&bytes);
                    if let Some(ref bar) = bar {
                        bar.inc(bytes.len() as u64);
                    }
                }
            }
            if let Some(bar) = bar {
                bar.finish_with_message(format!("Downloaded: {}", output.to_string_lossy()));
            }
            let downloaded_bytes_hash = hex::encode(&hash.finalize());
            println!("Build md5 checksum: {}", &downloaded_bytes_hash.green());
            if downloaded_bytes_hash != build_info.hash {
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
                .await
                .context("Error encountered when trying to list available builds")?
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
                    "{i:>2}. Date: {}, Platform: {}, Version: {}, Status: {}, Release: {}",
                    build.date,
                    build.platform,
                    build.full_version(),
                    status,
                    build.release
                )?;
            }
        }
    }

    Ok(())
}
