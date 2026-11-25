use std::time::{Duration, SystemTime};

use aws_credential_types::Credentials;
use aws_sigv4::{
    http_request::{
        PayloadChecksumKind, PercentEncodingMode, SignableBody, SignableRequest, SignatureLocation,
        SigningInstructions, SigningSettings, UriPathNormalizationMode, sign,
    },
    sign::v4,
};
use bytes::Bytes;
use http::{Request, Uri};
use http_body_util::{BodyExt, Full};
use hyper_rustls::{HttpsConnector, HttpsConnectorBuilder};
use hyper_util::{
    client::legacy::{Client, connect::HttpConnector},
    rt::TokioExecutor,
};
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
use serde::Deserialize;

use crate::runtime::dns::DnsResolver;

const KEY_ENCODE: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'~')
    .remove(b'/');

type HttpClient = Client<HttpsConnector<HttpConnector<DnsResolver>>, Full<Bytes>>;

pub(super) struct Config {
    pub access_key: String,
    pub secret_key: String,
    pub session_token: Option<String>,
    pub bucket: String,
    pub region: String,
    pub endpoint: String,
    pub timeout_ms: u64,
}

pub(super) struct Transport {
    config: Config,
    client: HttpClient,
}

/// Only controlled error messages and selected service identifiers cross into JS.
#[derive(Debug)]
pub(super) struct Error {
    pub message: &'static str,
    pub status: Option<u16>,
    pub code: Option<String>,
    pub request_id: Option<String>,
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl Error {
    pub(super) fn new(message: &'static str) -> Self {
        Self {
            message,
            status: None,
            code: None,
            request_id: None,
            source: None,
        }
    }

    fn with_source(mut self, source: impl std::error::Error + Send + Sync + 'static) -> Self {
        self.source = Some(Box::new(source));
        self
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.message)
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source.as_deref().map(|source| source as _)
    }
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ServiceError {
    code: Option<String>,
    request_id: Option<String>,
}

impl Transport {
    pub fn new(mut config: Config) -> Result<Self, &'static str> {
        if config.access_key.is_empty() || config.secret_key.is_empty() {
            return Err("S3 credentials must not be empty");
        }
        if !config
            .access_key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
        {
            return Err(
                "accessKeyId must contain only ASCII letters, digits, hyphens, or underscores",
            );
        }
        if config.session_token.as_ref().is_some_and(|token| {
            token.is_empty() || !token.bytes().all(|b| (33..=126).contains(&b))
        }) {
            return Err("sessionToken must contain nonempty printable ASCII without spaces");
        }
        if config.bucket.is_empty()
            || !config
                .bucket
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"-._".contains(&c))
            || matches!(config.bucket.as_str(), "." | "..")
        {
            return Err("bucket must be a nonempty bucket name, not a path");
        }
        if config.region.is_empty()
            || !config
                .region
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-')
        {
            return Err("region must contain only ASCII letters, digits, or hyphens");
        }
        let uri: Uri = config.endpoint.parse().map_err(|_| "invalid S3 endpoint")?;
        if !matches!(uri.scheme_str(), Some("http" | "https"))
            || uri.host().is_none()
            || uri.authority().is_some_and(|a| a.as_str().contains('@'))
            || uri.query().is_some()
            || config.endpoint.contains('#')
            || !matches!(uri.path(), "" | "/")
        {
            return Err(
                "endpoint must be an HTTP(S) origin without credentials, path, query, or fragment",
            );
        }
        config.endpoint = format!(
            "{}://{}",
            uri.scheme_str().unwrap(),
            uri.authority().unwrap()
        );
        let mut http = HttpConnector::new_with_resolver(DnsResolver::default());
        http.enforce_http(false);
        let connector = HttpsConnectorBuilder::new()
            .with_webpki_roots()
            .https_or_http()
            .enable_http1()
            .wrap_connector(http);
        let client = Client::builder(TokioExecutor::new())
            .retry_canceled_requests(false)
            .build(connector);
        Ok(Self { config, client })
    }

    fn url(&self, key: &str) -> String {
        format!(
            "{}/{}/{}",
            self.config.endpoint,
            self.config.bucket,
            utf8_percent_encode(key, KEY_ENCODE)
        )
    }

    fn sign(
        &self,
        request: SignableRequest<'_>,
        mut settings: SigningSettings,
    ) -> Result<SigningInstructions, Error> {
        let identity = Credentials::new(
            &self.config.access_key,
            &self.config.secret_key,
            self.config.session_token.clone(),
            None,
            "snek",
        )
        .into();
        settings.percent_encoding_mode = PercentEncodingMode::Single;
        settings.uri_path_normalization_mode = UriPathNormalizationMode::Disabled;
        let params = v4::SigningParams::builder()
            .identity(&identity)
            .region(&self.config.region)
            .name("s3")
            .time(SystemTime::now())
            .settings(settings)
            .build()
            .map_err(|error| Error::new("could not configure S3 signing").with_source(error))?
            .into();
        let (instructions, _) = sign(request, &params)
            .map_err(|error| Error::new("could not sign S3 request").with_source(error))?
            .into_parts();
        Ok(instructions)
    }

    fn sign_request(&self, mut request: Request<Bytes>) -> Result<Request<Full<Bytes>>, Error> {
        let url = request.uri().to_string();
        let headers = request
            .headers()
            .iter()
            .map(|(name, value)| {
                value
                    .to_str()
                    .map(|value| (name.as_str(), value))
                    .map_err(|error| Error::new("invalid S3 request headers").with_source(error))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let signable = SignableRequest::new(
            request.method().as_str(),
            &url,
            headers.into_iter(),
            SignableBody::Bytes(request.body()),
        )
        .map_err(|error| Error::new("could not prepare S3 signing").with_source(error))?;
        let mut settings = SigningSettings::default();
        settings.payload_checksum_kind = PayloadChecksumKind::XAmzSha256;
        self.sign(signable, settings)?
            .apply_to_request_http1x(&mut request);
        Ok(request.map(Full::new))
    }

    pub fn presign(&self, key: &str, method: &str, expires: u64) -> Result<String, Error> {
        if !matches!(method, "GET" | "PUT" | "HEAD" | "DELETE") || !(1..=604800).contains(&expires)
        {
            return Err(Error::new("invalid presign method or expiration"));
        }
        let url = self.url(key);
        let signable = SignableRequest::new(
            method,
            &url,
            std::iter::empty(),
            SignableBody::UnsignedPayload,
        )
        .map_err(|error| Error::new("could not prepare S3 signing").with_source(error))?;
        let mut settings = SigningSettings::default();
        settings.signature_location = SignatureLocation::QueryParams;
        settings.expires_in = Some(Duration::from_secs(expires));
        let instructions = self.sign(signable, settings)?;
        // Use the signer's query encoding without constructing an HTTP body.
        let mut request = Request::builder()
            .method(method)
            .uri(&url)
            .body(())
            .map_err(|error| Error::new("invalid S3 request headers").with_source(error))?;
        instructions.apply_to_request_http1x(&mut request);
        Ok(request.uri().to_string())
    }

    pub async fn request(
        &self,
        key: &str,
        method: &str,
        body: Bytes,
        headers: Vec<(String, String)>,
        timeout_ms: Option<u64>,
    ) -> Result<(http::HeaderMap, Bytes), Error> {
        let mut request = Request::builder().method(method).uri(self.url(key));
        for (name, value) in headers {
            request = request.header(name, value);
        }
        let request = request
            .body(body)
            .map_err(|error| Error::new("invalid S3 request headers").with_source(error))?;
        let request = self.sign_request(request)?;
        let mut observed_status = None;
        let mut observed_id = None;
        let result = tokio::time::timeout(
            Duration::from_millis(timeout_ms.unwrap_or(self.config.timeout_ms)),
            async {
                let response =
                    self.client.request(request).await.map_err(|error| {
                        Error::new("S3 network request failed").with_source(error)
                    })?;
                let status = response.status();
                let headers = response.headers().clone();
                observed_status = Some(status.as_u16());
                observed_id = headers
                    .get("x-amz-request-id")
                    .and_then(|v| v.to_str().ok())
                    .map(str::to_owned);
                let mut body = response.into_body();
                let mut bytes = Vec::new();
                while let Some(frame) = body.frame().await {
                    let frame = frame.map_err(|error| {
                        Error::new("could not read S3 response").with_source(error)
                    })?;
                    if let Ok(data) = frame.into_data() {
                        if !status.is_success() {
                            let remaining = 65536usize.saturating_sub(bytes.len());
                            bytes.extend_from_slice(&data[..data.len().min(remaining)]);
                            if bytes.len() == 65536 {
                                break;
                            }
                        } else {
                            bytes.extend_from_slice(&data);
                        }
                    }
                }
                if !status.is_success() {
                    let details: ServiceError =
                        quick_xml::de::from_reader(bytes.as_slice()).unwrap_or_default();
                    return Err(Error {
                        message: "S3 request failed",
                        status: Some(status.as_u16()),
                        code: details.code,
                        request_id: observed_id.clone().or(details.request_id),
                        source: None,
                    });
                }
                Ok((headers, Bytes::from(bytes)))
            },
        )
        .await;
        let result = result.unwrap_or_else(|error| {
            let mut failure = Error::new("S3 request timed out").with_source(error);
            failure.code = Some("TimeoutError".into());
            Err(failure)
        });
        result.map_err(|mut error| {
            error.status = error.status.or(observed_status);
            error.request_id = error.request_id.or(observed_id);
            error
        })
    }
}
