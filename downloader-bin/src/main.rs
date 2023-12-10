#![allow(unused)]
#![allow(dead_code)]
use clap::Parser;
use downloader_api::download;

#[derive(Debug, Parser)]
struct Args {
    #[arg(long)]
    user_id: String,
    #[arg(long)]
    user_key: String,
}

fn main() {
    let args: Args = Args::parse();
    let r = download(&args.user_id, &args.user_key).expect("Download");
}
