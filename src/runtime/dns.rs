use std::{
    future::Future,
    net::SocketAddr,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll},
};

use hickory_resolver::{TokioResolver, config::LookupIpStrategy};
use hyper_util::client::legacy::connect::dns::Name;
use tower_service::Service;

type Error = Box<dyn std::error::Error + Send + Sync>;
type Addresses = std::vec::IntoIter<SocketAddr>;

#[derive(Clone, Default)]
pub(super) struct DnsResolver {
    resolver: Arc<Mutex<Option<TokioResolver>>>,
}

impl DnsResolver {
    async fn lookup(self, name: String) -> Result<Addresses, Error> {
        // Initialize inside Tokio, and never fall back to a public DNS server.
        let resolver = {
            let mut state = self
                .resolver
                .lock()
                .map_err(|_| std::io::Error::other("DNS resolver initialization lock poisoned"))?;
            if state.is_none() {
                let mut builder = TokioResolver::builder_tokio()?;
                builder.options_mut().ip_strategy = LookupIpStrategy::Ipv4AndIpv6;
                *state = Some(builder.build()?);
            }
            state.as_ref().unwrap().clone()
        };
        Ok(resolver
            .lookup_ip(name)
            .await?
            .iter()
            .map(|ip| SocketAddr::new(ip, 0))
            .collect::<Vec<_>>()
            .into_iter())
    }
}

impl Service<Name> for DnsResolver {
    type Response = Addresses;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Addresses, Error>> + Send>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, name: Name) -> Self::Future {
        Box::pin(self.clone().lookup(name.as_str().to_owned()))
    }
}

impl reqwest::dns::Resolve for DnsResolver {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let resolver = self.clone();
        Box::pin(async move {
            let addresses = resolver.lookup(name.as_str().to_owned()).await?;
            Ok(Box::new(addresses) as reqwest::dns::Addrs)
        })
    }
}
