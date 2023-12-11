#![allow(unused)]
#![allow(dead_code)]
use reqwest::blocking::Client;
use reqwest::{StatusCode, Url};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use serde_this_or_that::as_u64;
use std::collections::HashMap;
use std::error::Error as StdError;
use std::fmt::Display;
use std::time::SystemTime;

const ACCESS_TOKEN_URL: &str = "https://www.sidefx.com/oauth2/application_token";
const ENDPOINT_URL: &str = "https://www.sidefx.com/api";

pub(crate) type BoxError = Box<dyn StdError + Send + Sync>;

#[derive(Debug, Eq, PartialEq)]
pub enum Kind {
    AuthError,
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
    pub fn is_authorization_error(&self) -> bool {
        self.inner.kind == Kind::AuthError
    }
}

impl Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "TODO. Kind: {:?}, e: {:?} ",
            self.inner.kind, self.inner.source
        )
    }
}

impl StdError for ApiError {}

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
#[derive(Debug, Serialize, Deserialize, Copy, Clone)]
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

impl Platform {
    fn from_build_str(platform: &str) -> Self {
        if platform.starts_with("linux") {
            Platform::Linux
        } else if platform.starts_with("win64") {
            Platform::Win64
        } else if platform.starts_with("macosx_x86") {
            Platform::Macos
        } else {
            Platform::MacosxArm64
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListBuildsParms {
    pub product: Product,
    pub platform: Platform,
    pub version: String,
    pub only_production: bool,
}

impl ListBuildsParms {
    pub fn new() -> Self {
        ListBuildsParms {
            product: Product::Houdini,
            platform: Platform::Linux,
            version: "19.5".to_string(),
            only_production: true,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct DownloadParms {
    product: Product,
    platform: Platform,
    version: String,
    build: u64,
}

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

fn get_access_token_and_expiry_time(
    client: &Client,
    user_id: &str,
    user_secret: &str,
) -> Result<AccessToken, ApiError> {
    let resp = client
        .post(ACCESS_TOKEN_URL)
        .basic_auth(user_id, Some(user_secret))
        .send()?;

    if !resp.status().is_success() {
        match resp.status() {
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                return Err(ApiError::new(Kind::AuthError, None::<BoxError>));
            }
            error_status => {
                return Err(ApiError::new(
                    Kind::Request,
                    Some(format!("Request error code: {error_status:?}")),
                ))
            }
        }
    }

    let mut token: AccessToken = resp.json()?;

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
        let json_value = self.call_api(EndPoint::ListBuilds(parameters))?;
        serde_json::from_value(json_value).map_err(|e| ApiError::new(Kind::Decode, Some(e)))
    }

    pub fn get_build_url(&self, build: &Build) -> Result<BuildUrl, ApiError> {
        let parms = DownloadParms {
            product: build.product,
            platform: Platform::from_build_str(&build.platform),
            version: build.version.clone(),
            build: build.build,
        };
        let json_value = self.call_api(EndPoint::Download(parms))?;

        serde_json::from_value(json_value).map_err(|e| ApiError::new(Kind::Decode, Some(e)))
    }

    fn call_api(&self, endpoint: EndPoint) -> reqwest::Result<Value> {
        let (method, parms) = endpoint.method_and_parms();
        let parms = json!([method, [], parms]).to_string();
        self.client
            .post(ENDPOINT_URL)
            .bearer_auth(&self.token.access_token)
            .form(&[("json", parms)])
            .send()?
            .json()
    }
}

#[derive(Debug, Deserialize)]
pub struct Build {
    #[serde(deserialize_with = "as_u64")]
    pub build: u64,
    pub date: String, // TODO: Use chrono
    pub product: Product,
    pub platform: String,
    pub release: String,
    pub status: String,
    pub version: String,
}

impl Build {
    pub fn full_version(&self) -> String {
        format!("{}.{}", self.version, self.build)
    }
}

#[derive(Debug, Deserialize)]
pub struct BuildUrl {
    pub download_url: String,
    pub filename: String,
    pub hash: String,
}
