#![allow(unused)]
#![allow(dead_code)]
use anyhow::Result;
use clap::Parser;
use downloader_api::{Downloader, ListBuildsParms, Platform, Product};

#[derive(Debug, Parser)]
struct Args {
    #[arg(long)]
    user_id: String,
    #[arg(long)]
    user_key: String,
}

fn main() -> Result<()> {
    let args: Args = Args::parse();
    let downloader = Downloader::new(&args.user_id, &args.user_key).expect("Downloader");
    let builds = downloader.list_builds(ListBuildsParms {
        product: Product::Houdini,
        version: "19.5".to_string(),
        platform: Platform::Linux,
        only_production: true,
    })?;
    let res = downloader.get_build_url(&builds[0])?;
    dbg!(res.hash);
    // for b in builds {
    //     dbg!(b);
    // }
    Ok(())
}
