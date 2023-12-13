use bytes::Bytes;
use reqwest::Client as HttpClient;
use reqwest::StatusCode;
use serde::{de::Error, Deserialize, Serialize};
use serde_json::{json, Value};
use std::error::Error as StdError;

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

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match &self.inner.source {
            None => format!(
                "Error Kind: {:?}. No error source available",
                self.inner.kind
            ),
            Some(err) => err.to_string(),
        };
        f.write_str(&msg)
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

impl From<serde_json::Error> for ApiError {
    fn from(value: serde_json::Error) -> Self {
        ApiError::new(Kind::Decode, Some(value))
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
    #[serde(rename = "houdini-launcher")]
    HoudiniLauncher,
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
    pub platform: Platform,
    // TODO: Support version list
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

async fn get_access_token_and_expiry_time(
    client: &HttpClient,
    user_id: &str,
    user_secret: &str,
) -> Result<AccessToken, ApiError> {
    let resp = client
        .post(ACCESS_TOKEN_URL)
        .basic_auth(user_id, Some(user_secret))
        .send()
        .await?;

    if !resp.status().is_success() {
        return match resp.status() {
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Err(ApiError::new(
                Kind::AuthError,
                Some("Could not authorize, check user credentials.".to_string()),
            )),
            error_status => Err(ApiError::new(
                Kind::Request,
                Some(format!("Request error code: {error_status:?}")),
            )),
        };
    }

    let mut token: AccessToken = resp.json().await?;

    token.expiry_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        - 2
        + token.expires_in as u64;
    Ok(token)
}

pub struct SesiClient {
    token: AccessToken,
    client: HttpClient,
}

impl SesiClient {
    pub async fn new(user_id: &str, user_secret: &str) -> Result<Self, ApiError> {
        let client = HttpClient::new();
        let token = get_access_token_and_expiry_time(&client, user_id, user_secret).await?;
        Ok(SesiClient { token, client })
    }

    pub async fn list_builds(
        &self,
        product: Product,
        platform: Platform,
        version: impl Into<String>,
        only_production: bool,
    ) -> Result<Vec<Build>, ApiError> {
        let body = self
            .call_api(EndPoint::ListBuilds(ListBuildsParms {
                product,
                platform,
                version: version.into(),
                only_production,
            }))
            .await?;
        serde_json::from_slice(&body).map_err(|e| ApiError::new(Kind::Decode, Some(e)))
    }

    pub async fn get_build_url(
        &self,
        product: Product,
        platform: Platform,
        version: impl Into<String>,
        build: u64,
    ) -> Result<BuildUrl, ApiError> {
        let parms = DownloadParms {
            product,
            platform,
            version: version.into(),
            build,
        };
        let body = self.call_api(EndPoint::Download(parms)).await?;

        serde_json::from_slice(&body)
            .map_err(|_| ApiError::new(Kind::Decode, Some(String::from_utf8_lossy(&body))))
    }

    async fn call_api(&self, endpoint: EndPoint) -> reqwest::Result<Bytes> {
        let (method, parms) = endpoint.method_and_parms();
        let parms = json!([method, [], parms]).to_string();
        self.client
            .post(ENDPOINT_URL)
            .bearer_auth(&self.token.access_token)
            .form(&[("json", parms)])
            .send()
            .await?
            .bytes()
            .await
    }
}

#[derive(Debug, Deserialize)]
pub struct Build {
    #[serde(deserialize_with = "parse_build_number")]
    pub build: u64,
    pub date: String, // TODO: Use chrono
    pub product: Product,
    pub platform: String,
    pub release: String,
    pub status: String,
    pub version: String,
}

fn parse_build_number<'de, D: serde::Deserializer<'de>>(des: D) -> Result<u64, D::Error> {
    let str_val = String::deserialize(des)?;
    str_val
        .parse()
        .map_err(|_| Error::custom("build is not a number"))
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
    pub size: u64,
}
