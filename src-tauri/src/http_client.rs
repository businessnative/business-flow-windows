use async_trait::async_trait;
use bnflow_core::{map_error, BuiltRequest, FlowError, HttpClient};
use std::sync::OnceLock;
use std::time::Duration;

#[derive(Clone)]
pub struct Net;

#[async_trait]
impl HttpClient for Net {
    async fn send(&self, req: BuiltRequest) -> Result<Vec<u8>, FlowError> {
        let method = reqwest::Method::from_bytes(req.method.as_bytes()).unwrap_or(reqwest::Method::POST);
        let mut builder = client().request(method, &req.url);
        for (k, v) in &req.headers {
            builder = builder.header(k, v);
        }
        if !req.body.is_empty() {
            builder = builder.body(req.body);
        }
        match builder.send().await {
            Ok(resp) => {
                let status = resp.status().as_u16();
                let bytes = resp.bytes().await.map_err(|_| FlowError::Network)?.to_vec();
                if (200..300).contains(&status) { Ok(bytes) } else { Err(map_error(status, &bytes)) }
            }
            Err(err) => {
                if err.is_timeout() { Err(FlowError::Timeout) } else { Err(FlowError::Network) }
            }
        }
    }
}

fn client() -> reqwest::Client {
    static CELL: OnceLock<reqwest::Client> = OnceLock::new();
    CELL.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(45))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new())
    })
    .clone()
}
