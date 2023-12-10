#![allow(unused)]
#![allow(dead_code)]
use reqwest::blocking::Client;
use reqwest::{StatusCode, Url};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use serde_this_or_that::as_u64;
use std::collections::HashMap;
use std::error::Error as StdError;
use std::time::SystemTime;

const ACCESS_TOKEN_URL: &str = "https://www.sidefx.com/oauth2/application_token";
const ENDPOINT_URL: &str = "https://www.sidefx.com/api";

pub(crate) type BoxError = Box<dyn StdError + Send + Sync>;

#[derive(Debug)]
pub(crate) enum Kind {
    Request,
    Decode,
}
#[derive(Debug)]
struct Inner {
    kind: Kind,
    source: Option<BoxError>,
}

#[derive(Debug)]
pub struct ApiError {
    inner: Box<Inner>,
}

impl ApiError {
    pub(crate) fn new<E>(kind: Kind, source: Option<E>) -> ApiError
    where
        E: Into<BoxError>,
    {
        ApiError {
            inner: Box::new(Inner {
                kind,
                source: source.map(Into::into),
            }),
        }
    }
}
impl From<reqwest::Error> for ApiError {
    fn from(value: reqwest::Error) -> Self {
        ApiError::new(Kind::Request, Some(value))
    }
}

#[derive(Deserialize, Debug)]
pub struct AccessToken {
    access_token: String,
    expires_in: u32,
    #[serde(default)]
    expiry_time: u64,
}

#[non_exhaustive]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Product {
    Houdini,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Linux,
    Win64,
    Macos,
    #[serde(rename = "macosx_arm64")]
    MacosxArm64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListBuildsParms {
    pub product: Product,
    pub version: String,
    pub platform: Platform,
    pub only_production: bool,
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
    fn method_and_parms(&self) -> (&'static str, Value) {
        match self {
            EndPoint::ListBuilds(parms) => (
                "download.get_daily_builds_list",
                serde_json::to_value(parms).unwrap(),
            ),
            EndPoint::Download(parms) => (
                "download.get_daily_build_download",
                serde_json::to_value(parms).unwrap(),
            ),
        }
    }
}

pub fn call_api(client: &Client, access_token: &str, endpoint: EndPoint) -> reqwest::Result<Value> {
    let (method, parms) = endpoint.method_and_parms();
    let parms = json!([method, [], parms]).to_string();
    client
        .post(ENDPOINT_URL)
        .bearer_auth(access_token)
        .form(&[("json", parms)])
        .send()?
        .json()
}

fn get_access_token_and_expiry_time(
    client: &Client,
    user_id: &str,
    user_secret: &str,
) -> reqwest::Result<AccessToken> {
    let mut token: AccessToken = client
        .post(ACCESS_TOKEN_URL)
        .basic_auth(user_id, Some(user_secret))
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

pub struct Downloader {
    token: AccessToken,
    client: Client,
}

impl Downloader {
    pub fn new(user_id: &str, user_secret: &str) -> Result<Self, ApiError> {
        let client = Client::new();
        let token = get_access_token_and_expiry_time(&client, user_id, user_secret)?;
        Ok(Downloader { token, client })
    }

    pub fn list_builds(&self, parameters: ListBuildsParms) -> Result<Vec<Build>, ApiError> {
        let json_value = call_api(
            &self.client,
            &self.token.access_token,
            EndPoint::ListBuilds(parameters),
        )?;
        serde_json::from_value(json_value).map_err(|e| ApiError::new(Kind::Decode, Some(e)))
    }
}

#[derive(Debug, Deserialize)]
pub struct Build {
    #[serde(deserialize_with = "as_u64")]
    build: u64,
    date: String, // TODO: Use chrono
    product: Product,
    platform: String,
    release: String,
    status: String,
    version: String,
}
