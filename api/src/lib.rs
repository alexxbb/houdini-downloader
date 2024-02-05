use bytes::Bytes;
use reqwest::Client as ReqwestClient;
use reqwest::StatusCode;
use serde::{de::Error, Deserialize, Serialize};
use serde_json::json;
use std::error::Error as StdError;

const ACCESS_TOKEN_URL: &str = "https://www.sidefx.com/oauth2/application_token";
const ENDPOINT_URL: &str = "https://www.sidefx.com/api";

pub(crate) type BoxError = Box<dyn StdError + Send + Sync>;

#[derive(Debug)]
pub struct ApiError(BoxError);

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!("ApiError: {}", self.0))
    }
}

impl StdError for ApiError {}

impl ApiError {
    pub(crate) fn new<E>(source: E) -> ApiError
    where
        E: Into<BoxError>,
    {
        ApiError(source.into())
    }
}
impl From<reqwest::Error> for ApiError {
    fn from(value: reqwest::Error) -> Self {
        ApiError(Box::new(value))
    }
}

impl From<serde_json::Error> for ApiError {
    fn from(value: serde_json::Error) -> Self {
        ApiError::new(Box::new(value))
    }
}

#[non_exhaustive]
#[derive(Debug, Serialize, Deserialize, Copy, Clone)]
#[serde(rename_all = "lowercase")]
pub enum Product {
    Houdini,
    #[serde(rename = "houdini-launcher")]
    HoudiniLauncher,
    #[serde(rename = "launcher-iso")]
    LauncherIso,
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
    pub version: Option<String>,
    pub only_production: bool,
}

impl ListBuildsParms {
    pub fn new() -> Self {
        ListBuildsParms {
            product: Product::Houdini,
            platform: Platform::Linux,
            version: None,
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

enum EndPoint {
    ListBuilds(ListBuildsParms),
    Download(DownloadParms),
}

async fn get_access_token(
    client: &ReqwestClient,
    user_id: &str,
    user_secret: &str,
) -> Result<String, ApiError> {
    #[derive(Deserialize, Serialize)]
    struct Token {
        access_token: String,
        // Lifespan of the token
        expires_in: u64,
        #[serde(default)]
        // Time in seconds when the token expire
        expires_at: u64,
    }

    fn time_now() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }
    let token_file = dirs::cache_dir().map(|path| {
        path.join("houdini.downloader")
            .join("api")
            .with_extension("token")
    });

    if let Some(token_file) = &token_file {
        if let Ok(data) = std::fs::read(token_file) {
            let token: Token = serde_json::from_slice(&data)?;
            if time_now() < token.expires_at {
                return Ok(token.access_token);
            }
        }
    }

    let resp = client
        .post(ACCESS_TOKEN_URL)
        .basic_auth(user_id, Some(user_secret))
        .send()
        .await?;

    if !resp.status().is_success() {
        return match resp.status() {
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Err(ApiError::new(
                "Could not authorize, check user credentials.".to_string(),
            )),
            error_status => Err(ApiError::new(format!(
                "Request error code: {error_status:?}"
            ))),
        };
    }

    let mut token: Token = resp.json().await?;

    if let Some(token_file) = &token_file {
        let _ = std::fs::create_dir_all(token_file.parent().expect("parent must present"));
        if let Ok(file) = std::fs::File::create(token_file) {
            token.expires_at = time_now() + token.expires_in;
            if let Err(e) = serde_json::to_writer(file, &token) {
                eprintln!("Could not save token file {}", e)
            }
        }
    }

    Ok(token.access_token)
}

pub struct SesiClient {
    token: String,
    client: ReqwestClient,
}

impl SesiClient {
    pub async fn new(user_id: &str, user_secret: &str) -> Result<Self, ApiError> {
        let client = ReqwestClient::new();
        let token = get_access_token(&client, user_id, user_secret).await?;
        Ok(SesiClient { token, client })
    }

    pub async fn list_builds(
        &self,
        product: Product,
        platform: Platform,
        version: Option<impl Into<String>>,
        only_production: bool,
    ) -> Result<Vec<Build>, ApiError> {
        let body = self
            .call_api(EndPoint::ListBuilds(ListBuildsParms {
                product,
                platform,
                version: version.map(|t| t.into()),
                only_production,
            }))
            .await?;
        serde_json::from_slice(&body).map_err(|e| ApiError::new(e))
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

        serde_json::from_slice(&body).map_err(|_| ApiError::new(String::from_utf8_lossy(&body)))
    }

    async fn call_api(&self, endpoint: EndPoint) -> reqwest::Result<Bytes> {
        let (method, parms) = match endpoint {
            EndPoint::ListBuilds(parms) => (
                "download.get_daily_builds_list",
                serde_json::to_value(parms).unwrap(),
            ),
            EndPoint::Download(parms) => (
                "download.get_daily_build_download",
                serde_json::to_value(parms).unwrap(),
            ),
        };
        let parms = json!([method, [], parms]).to_string();
        self.client
            .post(ENDPOINT_URL)
            .bearer_auth(&self.token)
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

#[derive(Debug, Deserialize)]
pub struct BuildUrl {
    pub download_url: String,
    pub filename: String,
    pub hash: String,
    pub size: u64,
}
