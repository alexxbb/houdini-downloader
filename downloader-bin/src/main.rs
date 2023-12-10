#![allow(unused)]
#![allow(dead_code)]
use clap::Parser;
use downloader_api::{Downloader, ListBuildsParms, Platform, Product};

#[derive(Debug, Parser)]
struct Args {
    #[arg(long)]
    user_id: String,
    #[arg(long)]
    user_key: String,
}

fn main() {
    let args: Args = Args::parse();
    let downloader = Downloader::new(&args.user_id, &args.user_key).expect("Downloader");
    let builds = downloader.list_builds(ListBuildsParms {
        product: Product::Houdini,
        version: "19.5".to_string(),
        platform: Platform::Linux,
        only_production: true,
    });
    for b in builds {
        dbg!(b);
    }
}
