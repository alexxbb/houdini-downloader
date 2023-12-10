#![allow(unused)]
#![allow(dead_code)]
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::time::SystemTime;

const ACCESS_TOKEN_URL: &str = "https://www.sidefx.com/oauth2/application_token";
const ENDPOINT_URL: &str = "https://www.sidefx.com/api";

#[derive(Deserialize, Debug)]
pub struct AccessToken {
    access_token: String,
    expires_in: u32,
    #[serde(default)]
    expiry_time: u64,
}

pub fn get_access_token_and_expiry_time(
    client_id: &str,
    client_secret: &str,
) -> reqwest::Result<AccessToken> {
    let client = reqwest::blocking::Client::new();

    let mut token: AccessToken = client
        .post(ACCESS_TOKEN_URL)
        .basic_auth(client_id, Some(client_secret))
        .send()?
        .json()?;
    token.expiry_time = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        - 2
        + token.expires_in as u64;
    Ok(token)
}

#[non_exhaustive]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Product {
    Houdini,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Platform {
    Linux,
    Win64,
    Macos,
    #[serde(rename = "macosx_arm64")]
    MacosxArm64,
}

#[derive(Debug, Serialize, Deserialize)]
struct ListBuildsParms {
    product: Product,
    version: String,
    platform: Platform,
    only_production: bool,
}

impl ListBuildsParms {
    pub fn new() -> Self {
        ListBuildsParms {
            product: Product::Houdini,
            version: "19.5".to_string(),
            platform: Platform::Linux,
            only_production: true,
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct DownloadParms {}

#[derive(Debug)]
enum EndPoint {
    ListBuilds(ListBuildsParms),
    Download(DownloadParms),
}

impl EndPoint {
    fn method(&self) -> &'static str {
        match self {
            EndPoint::ListBuilds(_) => "download.get_daily_builds_list",
            EndPoint::Download(_) => "download.get_daily_build_download",
        }
    }
    fn parameters(&self) -> serde_json::Result<Value> {
        match self {
            EndPoint::ListBuilds(parms) => serde_json::to_value(parms),
            EndPoint::Download(parms) => serde_json::to_value(parms),
        }
    }
}

pub fn call_api(access_token: AccessToken, endpoint: EndPoint) -> reqwest::Result<Value> {
    let client = reqwest::blocking::Client::new();
    let parms = json!([endpoint.method(), [], endpoint.parameters().expect("TODO")]).to_string();
    let resp = client
        .post(ENDPOINT_URL)
        .bearer_auth(&access_token.access_token)
        .form(&[("json", parms)])
        .send()?;

    if !resp.status().is_success() {
        if let Ok(b) = resp.bytes() {
            let s = String::from_utf8_lossy(&b);
            eprintln!("Body: {}", s);
        }
        return Ok(Value::String("TODO".to_string()));
    }
    resp.json()
}

pub fn download(client_id: &str, client_secret: &str) -> reqwest::Result<()> {
    let token = get_access_token_and_expiry_time(client_id, client_secret)?;
    let val = call_api(token, EndPoint::ListBuilds(ListBuildsParms::new()))?;
    let dict: Vec<HashMap<String, Value>> = serde_json::from_value(val).unwrap();
    dbg!(&dict[0]);
    Ok(())
}

/*
post_data = 'json: ["download.get_daily_build_download", [], {"product": "houdini", "version": "19.5", "build": "production", "platform": "linux"}]'
response = {'download_url': 'https://d199n7he4uszw5.cloudfront.net/download/download-build/108884/cdn/?Expires=1702184271&Signature=NQVZ7m7D4hDk3yWt-UEzgJirwLZNaH~7-TPb5wLWauhspvW4mX6vwtzLXl9GdFKHwQ9o~j4h4d8ah-MulJF6cGmRJYo5uXEmNy1rdpJQSTr4-XrexaB3th69WuKkpg~2LWxxBd6Z2EU5IdAbncVismQMcNGj~H~nnX7JRVAX4dDnvHg~uO-dTePz~6OtPEetUFJNO84bmF3rnyuSCFNPTEIlMv707sMu999j6mPXlEpN1f4DJjhI2JnB3Q8w-PyIOxPePqCoQSWvo5H29lMyXdhCFvvF-u8wSfOKmKRGFYLg0S~V9l8twT0rzJ2kqMOEuvmO8ujmTu~epHNbq2Fgkw__&Key-Pair-Id=APKAITRNKY64UW6MKIWQ', 'filename': 'houdini-19.5.805-linux_x86_64_gcc9.3.tar.gz', 'hash': '224a0a15b34dd74b453ba8714c223e21', 'size': 2341358674, 'date': '2023/11/21', 'status': 'good', 'releases_list': 'gold', 'pdb_download_url': 'https://d199n7he4uszw5.cloudfront.net/download/download-build/108884/pdb/cdn/?Expires=1702184272&Signature=SXQF31JmSqoa7EsvRTcamvQl69ABGjcBDV6r~FJGSquE4PFPVXv~xp~w0yPoqBZgHVGZsN5XqZSQ~ZwfCdcq8VZGy-HagqcvC0~TRcwPEa6L129dXSMnTIMz-u5xya2i11Rh43ZUhPKSKq0J6ugXiW-jgnyCaKRnwdbuNlWCwZlgTcbk3BPTNj8U158~3fVG6K23IPQerimRhfMCiV9TvQjjpKWqp2lfRvQElbEcgKS~LVNniLpwMCUCZPovpDZ3Mr5~x0N6uUh7V4dcMSPhw3p1nQqYUick0Zrwo0kNWTQTFUeA1~JFR89XwY-uEr0z2CdmpaaCn~r5tO-9h3t8yA__&Key-Pair-Id=APKAITRNKY64UW6MKIWQ'}
 */
