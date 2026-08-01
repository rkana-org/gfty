use std::{
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use hmac::{Hmac, Mac};
use reqwest::{
    Method, StatusCode, Url,
    blocking::{Client, Response},
    header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, DATE, HeaderMap, HeaderValue, LOCATION},
};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::credentials::Credentials;

const API_VERSION: &str = "v11";
const JSON_CONTENT_TYPE: &str = "application/json;charset=UTF-8; qs=0.09";
const JSON_ACCEPT: &str = "application/json;charset=UTF-8; qs=0.09";
const BINARY_ACCEPT: &str = "application/octet-stream";
const MAX_REDIRECTS: usize = 5;
const MAX_SHADED_VIEW_URL_LENGTH: usize = 6_000;
const TRANSLATION_TIMEOUT: Duration = Duration::from_secs(5 * 60);
// Onshape's documented isometric view: Z remains up and the front of the bin
// faces the viewer. Keep this matrix aligned with the browser designer view.
const ISOMETRIC_VIEW_MATRIX: &str = "0.612,0.612,0,0,-0.354,0.354,0.707,0,0.707,-0.707,0.707,0";
const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
static NONCE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone)]
pub struct ModelTarget {
    origin: String,
    document_id: String,
    version_id: String,
    element_id: String,
}

impl ModelTarget {
    pub fn parse(value: &str) -> Result<Self> {
        let url =
            Url::parse(value).with_context(|| format!("invalid Onshape model URL {value:?}"))?;
        if url.scheme() != "https" {
            bail!("Onshape model URL must use HTTPS");
        }
        let host = url.host_str().context("Onshape model URL has no host")?;
        if host != "onshape.com" && !host.ends_with(".onshape.com") {
            bail!("Onshape model URL host must be onshape.com or one of its subdomains");
        }
        let segments = url
            .path_segments()
            .context("Onshape model URL cannot be a base URL")?
            .collect::<Vec<_>>();
        let Some(document_index) = segments.iter().position(|segment| *segment == "documents")
        else {
            bail!(
                "Onshape model URL must contain /documents/DOCUMENT_ID/v/VERSION_ID/e/ELEMENT_ID"
            );
        };
        let tail = &segments[document_index + 1..];
        if tail.len() < 5 || tail[1] != "v" || tail[3] != "e" {
            if tail.get(1) == Some(&"w") {
                bail!("Onshape export requires an immutable version URL, not a workspace URL");
            }
            bail!(
                "Onshape model URL must contain /documents/DOCUMENT_ID/v/VERSION_ID/e/ELEMENT_ID"
            );
        }
        validate_id(tail[0], "document")?;
        validate_id(tail[2], "version")?;
        validate_id(tail[4], "element")?;
        let port = url
            .port()
            .map(|port| format!(":{port}"))
            .unwrap_or_default();
        Ok(Self {
            origin: format!("{}://{host}{port}", url.scheme()),
            document_id: tail[0].to_owned(),
            version_id: tail[2].to_owned(),
            element_id: tail[4].to_owned(),
        })
    }

    fn url(&self, path: &str) -> Result<Url> {
        Url::parse(&format!("{}{}", self.origin, path))
            .context("failed to construct Onshape API URL")
    }
}

fn validate_id(value: &str, kind: &str) -> Result<()> {
    if value.len() != 24 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        bail!("Onshape {kind} ID must contain 24 hexadecimal digits");
    }
    Ok(())
}

pub struct OnshapeClient {
    http: Client,
    credentials: Credentials,
}

impl OnshapeClient {
    pub fn new(credentials: Credentials) -> Result<Self> {
        let http = Client::builder()
            .connect_timeout(Duration::from_secs(30))
            .timeout(Duration::from_secs(2 * 60))
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(concat!("gfty/", env!("CARGO_PKG_VERSION")))
            .build()
            .context("failed to initialize HTTP client")?;
        Ok(Self { http, credentials })
    }

    pub fn export_label_step(
        &self,
        target: &ModelTarget,
        label_json: &str,
        gridfinity_json: &str,
        destination_name: &str,
    ) -> Result<Vec<u8>> {
        self.export_configured_step(
            target,
            &[
                ConfigurationValue {
                    parameter_id: "Config",
                    parameter_value: label_json,
                },
                ConfigurationValue {
                    parameter_id: "GFTYUltimateConfig",
                    parameter_value: gridfinity_json,
                },
            ],
            destination_name,
        )
    }

    pub fn export_bin_step(
        &self,
        target: &ModelTarget,
        gridfinity_json: &str,
        destination_name: &str,
    ) -> Result<Vec<u8>> {
        self.export_configured_step(
            target,
            &[ConfigurationValue {
                parameter_id: "Config",
                parameter_value: gridfinity_json,
            }],
            destination_name,
        )
    }

    pub fn render_bin_preview(
        &self,
        target: &ModelTarget,
        gridfinity_json: &str,
    ) -> Result<Vec<u8>> {
        let mut url = target.url(&format!(
            "/api/{API_VERSION}/partstudios/d/{}/v/{}/e/{}/shadedviews",
            target.document_id, target.version_id, target.element_id
        ))?;
        let configuration = format!("Config={}", form_encode(gridfinity_json));
        url.query_pairs_mut()
            .append_pair("configuration", &configuration)
            .append_pair("viewMatrix", ISOMETRIC_VIEW_MATRIX)
            .append_pair("outputWidth", "512")
            .append_pair("outputHeight", "512")
            .append_pair("pixelSize", "0")
            .append_pair("edges", "show")
            .append_pair("showAllParts", "true")
            .append_pair("useAntiAliasing", "true");
        if url.as_str().len() > MAX_SHADED_VIEW_URL_LENGTH {
            bail!(
                "configured shaded-view URL is {} characters; Onshape exposes previews only through GET and URLs above about {MAX_SHADED_VIEW_URL_LENGTH} characters are unreliable",
                url.as_str().len()
            );
        }

        eprintln!("Rendering configured PNG preview");
        let response = self
            .request(Method::GET, url, None, JSON_ACCEPT)
            .context("failed to request Onshape shaded view")?;
        let response: ShadedViewsResponse = serde_json::from_slice(&response)
            .context("Onshape returned an invalid shaded-view response")?;
        let images = response
            .images
            .into_iter()
            .flat_map(ShadedImageGroup::into_images)
            .collect::<Vec<_>>();
        if images.len() != 1 {
            bail!(
                "Onshape shaded view returned {} images; expected exactly one",
                images.len()
            );
        }
        let image = BASE64
            .decode(&images[0])
            .context("Onshape shaded view contains invalid base64 image data")?;
        if !image.starts_with(PNG_SIGNATURE) {
            bail!("Onshape shaded view is not a PNG image");
        }
        Ok(image)
    }

    fn export_configured_step(
        &self,
        target: &ModelTarget,
        parameters: &[ConfigurationValue<'_>],
        destination_name: &str,
    ) -> Result<Vec<u8>> {
        eprintln!("Encoding Onshape configuration");
        let encoded = self.encode_configuration(target, parameters)?;

        eprintln!("Starting configured STEP export");
        let mut translation = self.start_translation(target, &encoded, destination_name)?;
        let deadline = Instant::now() + TRANSLATION_TIMEOUT;
        let mut delay = Duration::from_secs(2);
        while translation.request_state == TranslationState::Active {
            if Instant::now() >= deadline {
                bail!(
                    "Onshape translation {} did not finish within {} seconds",
                    translation.id,
                    TRANSLATION_TIMEOUT.as_secs()
                );
            }
            eprintln!("Waiting for Onshape translation {}", translation.id);
            thread::sleep(delay);
            translation = self.get_translation(target, &translation.id)?;
            delay = (delay * 2).min(Duration::from_secs(20));
        }
        if translation.request_state != TranslationState::Done {
            bail!(
                "Onshape STEP translation failed: {}",
                translation
                    .failure_reason
                    .as_deref()
                    .unwrap_or("Onshape did not provide a failure reason")
            );
        }
        if translation.version_id.as_deref() != Some(&target.version_id) {
            bail!(
                "Onshape translated an unexpected version: expected {}, received {}",
                target.version_id,
                translation.version_id.as_deref().unwrap_or("none")
            );
        }
        if translation.workspace_id.is_some() {
            bail!("Onshape unexpectedly translated a mutable workspace");
        }
        let result_ids = translation.result_external_data_ids.unwrap_or_default();
        if result_ids.len() != 1 {
            bail!(
                "Onshape STEP export returned {} external files; expected exactly one grouped STEP",
                result_ids.len()
            );
        }
        eprintln!("Downloading configured STEP");
        self.download_external_data(
            target,
            translation
                .result_document_id
                .as_deref()
                .unwrap_or(&target.document_id),
            &result_ids[0],
        )
    }

    fn encode_configuration(
        &self,
        target: &ModelTarget,
        parameters: &[ConfigurationValue<'_>],
    ) -> Result<String> {
        let url = target.url(&format!(
            "/api/{API_VERSION}/elements/d/{}/e/{}/configurationencodings",
            target.document_id, target.element_id
        ))?;
        let body = serde_json::to_vec(&EncodeConfigurationRequest { parameters })
            .context("failed to serialize Onshape configuration request")?;
        let response = self
            .request(Method::POST, url, Some(body), JSON_ACCEPT)
            .context("failed to encode Onshape configuration")?;
        let response: EncodeConfigurationResponse = serde_json::from_slice(&response)
            .context("Onshape returned an invalid configuration encoding response")?;
        if response.encoded_id.is_empty() {
            bail!("Onshape returned an empty configuration encoding");
        }
        Ok(response.encoded_id)
    }

    fn start_translation(
        &self,
        target: &ModelTarget,
        configuration: &str,
        destination_name: &str,
    ) -> Result<TranslationResponse> {
        let url = target.url(&format!(
            "/api/{API_VERSION}/partstudios/d/{}/v/{}/e/{}/translations",
            target.document_id, target.version_id, target.element_id
        ))?;
        let body = serde_json::to_vec(&TranslationRequest {
            format_name: "STEP",
            store_in_document: false,
            configuration,
            grouping: true,
            destination_name,
            notify_user: false,
            step_version_string: "AP242",
            unit: "MILLIMETER",
        })
        .context("failed to serialize Onshape translation request")?;
        let response = self
            .request(Method::POST, url, Some(body), JSON_ACCEPT)
            .context("failed to start Onshape STEP translation")?;
        serde_json::from_slice(&response)
            .context("Onshape returned an invalid translation response")
    }

    fn get_translation(&self, target: &ModelTarget, id: &str) -> Result<TranslationResponse> {
        validate_id(id, "translation")?;
        let url = target.url(&format!("/api/{API_VERSION}/translations/{id}"))?;
        let response = self
            .request(Method::GET, url, None, JSON_ACCEPT)
            .context("failed to poll Onshape translation")?;
        serde_json::from_slice(&response)
            .context("Onshape returned an invalid translation status response")
    }

    fn download_external_data(
        &self,
        target: &ModelTarget,
        document_id: &str,
        external_id: &str,
    ) -> Result<Vec<u8>> {
        validate_id(document_id, "result document")?;
        validate_id(external_id, "external data")?;
        let url = target.url(&format!(
            "/api/{API_VERSION}/documents/d/{document_id}/externaldata/{external_id}"
        ))?;
        self.request(Method::GET, url, None, BINARY_ACCEPT)
            .context("failed to download Onshape STEP result")
    }

    fn request(
        &self,
        method: Method,
        mut url: Url,
        body: Option<Vec<u8>>,
        accept: &'static str,
    ) -> Result<Vec<u8>> {
        for redirect in 0..=MAX_REDIRECTS {
            let response = self.send_once(method.clone(), &url, body.clone(), accept)?;
            if response.status().is_redirection() {
                if redirect == MAX_REDIRECTS {
                    bail!("Onshape API exceeded {MAX_REDIRECTS} redirects");
                }
                let location = response
                    .headers()
                    .get(LOCATION)
                    .context("Onshape redirect omitted the Location header")?
                    .to_str()
                    .context("Onshape redirect Location is not valid text")?;
                url = url
                    .join(location)
                    .context("Onshape returned an invalid redirect URL")?;
                if url.scheme() != "https" {
                    bail!("Onshape redirected API credentials to a non-HTTPS URL");
                }
                continue;
            }
            return read_response(response);
        }
        unreachable!("redirect loop always returns or fails")
    }

    fn send_once(
        &self,
        method: Method,
        url: &Url,
        body: Option<Vec<u8>>,
        accept: &'static str,
    ) -> Result<Response> {
        let headers = self.signed_headers(&method, url, accept)?;
        let mut request = self.http.request(method, url.clone()).headers(headers);
        if let Some(body) = body {
            request = request.body(body);
        }
        request.send().context("failed to contact Onshape API")
    }

    fn signed_headers(&self, method: &Method, url: &Url, accept: &str) -> Result<HeaderMap> {
        let nonce = nonce();
        let date = httpdate::fmt_http_date(SystemTime::now());
        let canonical = format!(
            "{}\n{}\n{}\n{}\n{}\n{}\n",
            method.as_str(),
            nonce,
            date,
            JSON_CONTENT_TYPE,
            url.path(),
            url.query().unwrap_or("")
        )
        .to_ascii_lowercase();
        let mut hmac = Hmac::<Sha256>::new_from_slice(self.credentials.secret_key().as_bytes())
            .context("failed to initialize Onshape request signature")?;
        hmac.update(canonical.as_bytes());
        let signature = BASE64.encode(hmac.finalize().into_bytes());
        let authorization = format!(
            "On {}:HmacSHA256:{signature}",
            self.credentials.access_key()
        );

        let mut headers = HeaderMap::new();
        headers.insert(DATE, HeaderValue::from_str(&date)?);
        headers.insert("on-nonce", HeaderValue::from_str(&nonce)?);
        headers.insert(AUTHORIZATION, HeaderValue::from_str(&authorization)?);
        headers.insert(CONTENT_TYPE, HeaderValue::from_static(JSON_CONTENT_TYPE));
        headers.insert(ACCEPT, HeaderValue::from_str(accept)?);
        Ok(headers)
    }
}

fn read_response(response: Response) -> Result<Vec<u8>> {
    let status = response.status();
    let request_id = response
        .headers()
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let bytes = response
        .bytes()
        .context("failed to read Onshape response")?;
    if status.is_success() {
        return Ok(bytes.to_vec());
    }

    let detail = serde_json::from_slice::<ApiError>(&bytes)
        .ok()
        .and_then(|error| error.message)
        .or_else(|| {
            std::str::from_utf8(&bytes)
                .ok()
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .map(|text| text.chars().take(500).collect())
        })
        .unwrap_or_else(|| "Onshape returned no error details".to_owned());
    let request_context = request_id
        .map(|id| format!("; request ID {id}"))
        .unwrap_or_default();
    if status == StatusCode::TOO_MANY_REQUESTS {
        bail!("Onshape API rate limit exceeded: {detail}{request_context}");
    }
    bail!("Onshape API returned HTTP {status}: {detail}{request_context}")
}

fn form_encode(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(char::from(byte));
            }
            b' ' => encoded.push('+'),
            _ => {
                const HEX: &[u8; 16] = b"0123456789ABCDEF";
                encoded.push('%');
                encoded.push(char::from(HEX[usize::from(byte >> 4)]));
                encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
            }
        }
    }
    encoded
}

fn nonce() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let sequence = NONCE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("{nanos:x}{:x}{sequence:x}", std::process::id())
}

#[derive(Deserialize)]
struct ShadedViewsResponse {
    images: Vec<ShadedImageGroup>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ShadedImageGroup {
    Image(String),
    Images(Vec<String>),
}

impl ShadedImageGroup {
    fn into_images(self) -> Vec<String> {
        match self {
            Self::Image(image) => vec![image],
            Self::Images(images) => images,
        }
    }
}

#[derive(Serialize)]
struct EncodeConfigurationRequest<'a> {
    parameters: &'a [ConfigurationValue<'a>],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ConfigurationValue<'a> {
    parameter_id: &'a str,
    parameter_value: &'a str,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EncodeConfigurationResponse {
    encoded_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TranslationRequest<'a> {
    format_name: &'static str,
    store_in_document: bool,
    configuration: &'a str,
    grouping: bool,
    destination_name: &'a str,
    notify_user: bool,
    step_version_string: &'static str,
    unit: &'static str,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
enum TranslationState {
    Active,
    Done,
    Failed,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TranslationResponse {
    id: String,
    request_state: TranslationState,
    failure_reason: Option<String>,
    result_external_data_ids: Option<Vec<String>>,
    result_document_id: Option<String>,
    version_id: Option<String>,
    workspace_id: Option<String>,
}

#[derive(Deserialize)]
struct ApiError {
    message: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_immutable_part_studio_url() {
        let target = ModelTarget::parse(
            "https://cad.onshape.com/documents/089ad0a2edf08cd2cfdc9875/v/02d1ce92af09ce405aff8f7d/e/5bba513a46b691f2bf439aaa?renderMode=0",
        )
        .unwrap();
        assert_eq!(target.origin, "https://cad.onshape.com");
        assert_eq!(target.document_id, "089ad0a2edf08cd2cfdc9875");
        assert_eq!(target.version_id, "02d1ce92af09ce405aff8f7d");
        assert_eq!(target.element_id, "5bba513a46b691f2bf439aaa");
    }

    #[test]
    fn rejects_non_onshape_hosts() {
        let error = ModelTarget::parse(
            "https://example.com/documents/089ad0a2edf08cd2cfdc9875/v/02d1ce92af09ce405aff8f7d/e/5bba513a46b691f2bf439aaa",
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("host"));
    }

    #[test]
    fn rejects_workspace_targets() {
        let error = ModelTarget::parse(
            "https://cad.onshape.com/documents/089ad0a2edf08cd2cfdc9875/w/02d1ce92af09ce405aff8f7d/e/5bba513a46b691f2bf439aaa",
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("immutable version"));
    }

    #[test]
    fn nonce_is_alphanumeric_unique_and_long_enough() {
        let first = nonce();
        let second = nonce();
        assert_ne!(first, second);
        assert!(first.len() >= 16);
        assert!(first.bytes().all(|byte| byte.is_ascii_alphanumeric()));
    }

    #[test]
    fn form_encodes_configuration_values() {
        assert_eq!(
            form_encode(r#"{"label":"A B","size":[1,2]}"#),
            "%7B%22label%22%3A%22A+B%22%2C%22size%22%3A%5B1%2C2%5D%7D"
        );
    }

    #[test]
    fn parses_translation_states() {
        let response: TranslationResponse = serde_json::from_str(
            r#"{"id":"6a6bc337a92378177f5abc79","requestState":"DONE","failureReason":null,"resultExternalDataIds":[],"resultDocumentId":null,"versionId":null,"workspaceId":null}"#,
        )
        .unwrap();
        assert_eq!(response.request_state, TranslationState::Done);
    }
}
