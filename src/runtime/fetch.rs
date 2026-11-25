use std::{cell::RefCell, future::poll_fn, rc::Rc, task::Poll, time::Duration};

use boa_engine::{Context, Finalize, JsData, JsError, JsObject, JsResult, JsString, Trace};
use boa_runtime::{
    abort::JsAbortSignal,
    fetch::{Fetcher, request::JsRequest, response::JsResponse},
};

#[derive(Debug, Trace, Finalize, JsData)]
pub(super) struct HttpFetcher {
    #[unsafe_ignore_trace]
    client: reqwest::Client,
}

impl HttpFetcher {
    pub fn new() -> anyhow::Result<Self> {
        // Reuse the Ring provider used by the S3 transport; preserve any installed provider.
        let _ = rustls::crypto::ring::default_provider().install_default();
        Ok(Self {
            client: reqwest::Client::builder()
                .dns_resolver(std::sync::Arc::new(super::dns::DnsResolver::default()))
                .timeout(Duration::from_secs(60))
                .build()?,
        })
    }
}

impl Fetcher for HttpFetcher {
    async fn fetch(
        self: Rc<Self>,
        request: JsRequest,
        signal: Option<JsObject>,
        context: &RefCell<&mut Context>,
    ) -> JsResult<JsResponse> {
        let request = request.into_inner();
        let transfer = async {
            let response = self
                .client
                .request(request.method().clone(), request.uri().to_string())
                .headers(request.headers().clone())
                .body(request.into_body())
                .send()
                .await
                .map_err(JsError::from_rust)?;
            let url = JsString::from(response.url().as_str());
            let status = response.status();
            let headers = response.headers().clone();
            let body = response.bytes().await.map_err(JsError::from_rust)?;
            let mut response = http::Response::new(body.to_vec());
            *response.status_mut() = status;
            *response.headers_mut() = headers;
            Ok(JsResponse::basic(url, response))
        };
        let mut transfer = std::pin::pin!(transfer);
        poll_fn(|cx| {
            if let Some(signal) = signal
                .as_ref()
                .and_then(|s| s.downcast_ref::<JsAbortSignal>())
                && signal.is_aborted()
            {
                return Poll::Ready(Err(JsError::from_opaque(
                    signal.abort_reason(&mut context.borrow_mut()),
                )));
            }
            transfer.as_mut().poll(cx)
        })
        .await
    }
}
