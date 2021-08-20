use actix_web::{http::HeaderName, HttpRequest};
use actix_web_location::{
    providers::{FallbackProvider, MaxMindProvider},
    Error, Location, LocationConfig, Provider,
};
use async_trait::async_trait;
use cadence::StatsdClient;

use crate::{error::HandlerError, settings::Settings};

/// Provides the location from a configurable client specified header for
/// testing purposes.
pub struct TestHeaderProvider {
    test_header: HeaderName,
}

impl TestHeaderProvider {
    pub fn new(test_header: &str) -> Self {
        Self {
            test_header: HeaderName::from_lowercase(test_header.to_lowercase().as_ref())
                .expect("Invalid location_test_header"),
        }
    }
}

#[async_trait(?Send)]
impl Provider for TestHeaderProvider {
    fn name(&self) -> &str {
        "test_header"
    }

    fn expect_city(&self) -> bool {
        false
    }

    async fn get_location(&self, req: &HttpRequest) -> Result<Option<Location>, Error> {
        if let Some(header) = req.headers().get(&self.test_header) {
            let mut builder = Location::build().provider(self.name().to_owned());
            let mut parts = header.to_str().unwrap_or_default().split(',');

            if let Some(country) = parts.next() {
                let country = country.trim();
                if !country.is_empty() {
                    builder = builder.country(country.to_owned())
                }
            }

            if let Some(subdivision) = parts.next() {
                let mut subdivision = subdivision.trim();
                // Expect a "Unicode CLDR subdivision ID, such as USCA or CAON"
                // (modeled from Google Load Balancer's
                // client_region_subdivision)
                if subdivision.len() > 2 {
                    subdivision = &subdivision[2..];
                }
                if !subdivision.is_empty() {
                    builder = builder.region(subdivision.to_owned());
                }
            }

            if let Some(dma) = parts.next() {
                let dma = dma.trim().parse().unwrap_or(0);
                // Non-exact validation (there's only 210 DMA regions) but
                // close enough for testing
                if (500..=900).contains(&dma) {
                    builder = builder.dma(dma);
                }
            }

            let location = builder.finish().map_err(|_| {
                Error::Provider(HandlerError::internal("Couldn't build Location").into())
            })?;
            Ok(Some(location))
        } else {
            Ok(None)
        }
    }
}

pub fn location_config_from_settings(
    settings: &Settings,
    metrics: &StatsdClient,
) -> LocationConfig {
    let mut location_config = LocationConfig::default().with_metrics(metrics.clone());
    if let Some(ref test_header) = settings.location_test_header {
        location_config = location_config.with_provider(TestHeaderProvider::new(test_header));
    }
    if let Some(ref path) = settings.maxminddb_loc {
        location_config = location_config
            .with_provider(MaxMindProvider::from_path(path).expect("Could not read mmdb file"));
    }
    location_config.with_provider(FallbackProvider::new(
        Location::build().country(settings.fallback_country.clone()),
    ))
}

#[cfg(test)]
pub mod test {
    use super::TestHeaderProvider;

    use actix_web::test::TestRequest;
    use actix_web_location::{Location, Provider};

    #[actix_rt::test]
    async fn from_test_header() {
        let test_header = "x-test-location";
        let provider = TestHeaderProvider::new(test_header);

        let request = TestRequest::default()
            .header(test_header, "US, USCA, 862")
            .to_http_request();
        let location = provider
            .get_location(&request)
            .await
            .expect("could not get location")
            .expect("location was none");

        let expected = Location::build()
            .provider("test_header".to_owned())
            .country("US".to_owned())
            .region("CA".to_owned())
            .dma(862)
            .finish()
            .expect("Couldn't build Location");
        assert_eq!(location, expected);
    }

    #[actix_rt::test]
    async fn no_test_header() {
        let test_header = "x-test-location";
        let provider = TestHeaderProvider::new(test_header);

        let request = TestRequest::default().to_http_request();
        let location = provider
            .get_location(&request)
            .await
            .expect("could not get location");
        assert_eq!(location, None);
    }
}
