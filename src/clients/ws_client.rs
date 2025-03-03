use futures::SinkExt;
use yawc::{frame::FrameView, CompressionLevel, Options, WebSocket};
use tokio_rustls::{
    rustls::{self, pki_types::TrustAnchor}, TlsConnector
};
use std::sync::Arc;


pub struct WebsocketClient<'a> {
    pub client: WebSocket,
    pub url: &'a str
}

impl<'a> WebsocketClient<'a> {
    pub async  fn new(url : &'a str) -> anyhow::Result<Self> {
        let client = WebSocket::connect_with_options(
            url.parse().unwrap(),
            Some(WebsocketClient::tls_connector()),
            Options::default().with_compression_level(CompressionLevel::fast()),
        )
        .await?;
        Ok(Self {url, client})
    }

    pub async fn send(&mut self, msg: String) -> anyhow::Result<(), yawc::WebSocketError>{
        Ok(self.client.send(FrameView::text(msg)).await?)
    }

    pub async fn close(&mut self) -> anyhow::Result<()> {
        self.client.close().await?;
        Ok(())
    }

    pub async fn ping(&mut self, data: &'static str) -> anyhow::Result<()> {
        self.client.send(FrameView::ping(data)).await?;
        Ok(())
    }

    fn tls_connector() -> TlsConnector {
        let mut root_cert_store = rustls::RootCertStore::empty();
        root_cert_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().map(|ta| TrustAnchor {
            subject: ta.subject.clone(),
            subject_public_key_info: ta.subject_public_key_info.clone(),
            name_constraints: ta.name_constraints.clone(),
        }));
    
        TlsConnector::from(Arc::new(
            rustls::ClientConfig::builder()
                .with_root_certificates(root_cert_store)
                .with_no_client_auth(),
        ))
    }
}