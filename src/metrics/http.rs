//! Contains HTTP metrics related code, notably [`HttpMetrics`].

use hyper::Method;
use lazy_static::lazy_static;
use prometheus_client::encoding::LabelValueEncoder;
use prometheus_client::encoding::{EncodeLabelSet, EncodeLabelValue};
use prometheus_client::metrics::counter::Counter;
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::gauge::Gauge;
use prometheus_client::registry::{Registry, Unit};
use std::fmt::{Display, Formatter, Write};
use std::time::Duration;

lazy_static! {
    // Create a sample counter metric family utilizing the above custom label
    // type, representing the number of HTTP requests received.
    static ref TRACK_ENDPOINT: Family<Labels, Counter> = Family::default();
    static ref TRACK_DURATION: Family<Labels, Counter<f64>> = Family::default();
    static ref TRACK_IN_FLIGHT: Family<InFlightLabels, Gauge> = Family::default();
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
struct Labels {
    // Use your own enum types to represent label values.
    method: HttpMethod,
    // Or just a plain string.
    path: String,
    /// The HTTP status code.
    status: u16,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
struct InFlightLabels {
    path: String,
}

/// The HTTP method to track.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum HttpMethod {
    OPTIONS,
    GET,
    POST,
    PUT,
    DELETE,
    HEAD,
    PATCH,
    UNHANDLED(Method),
}

impl EncodeLabelValue for HttpMethod {
    fn encode(&self, encoder: &mut LabelValueEncoder) -> Result<(), std::fmt::Error> {
        encoder.write_str(self.to_string().as_str())
    }
}

impl Display for HttpMethod {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            HttpMethod::OPTIONS => write!(f, "OPTIONS"),
            HttpMethod::GET => write!(f, "GET"),
            HttpMethod::POST => write!(f, "POST"),
            HttpMethod::PUT => write!(f, "PUT"),
            HttpMethod::DELETE => write!(f, "DELETE"),
            HttpMethod::HEAD => write!(f, "HEAD"),
            HttpMethod::PATCH => write!(f, "PATCH"),
            HttpMethod::UNHANDLED(other) => write!(f, "{other}"),
        }
    }
}

impl From<&Method> for HttpMethod {
    fn from(value: &Method) -> Self {
        match value {
            &Method::GET => Self::GET,
            &Method::OPTIONS => Self::OPTIONS,
            &Method::POST => Self::POST,
            &Method::PUT => Self::PUT,
            &Method::DELETE => Self::DELETE,
            &Method::HEAD => Self::HEAD,
            &Method::PATCH => Self::PATCH,
            other => Self::UNHANDLED(other.clone()),
        }
    }
}

impl From<Method> for HttpMethod {
    fn from(value: Method) -> Self {
        HttpMethod::from(&value)
    }
}

/// Register the `http_requests` metric family with the registry.
pub(crate) fn register_http_requests(registry: &mut Registry) {
    registry.register(
        // With the metric name.
        "http_requests",
        // And the metric help text.
        "Number of HTTP requests received",
        TRACK_ENDPOINT.clone(),
    );

    registry.register_with_unit(
        "http_duration",
        "Duration of HTTP requests executed",
        Unit::Seconds,
        TRACK_DURATION.clone(),
    );

    registry.register(
        "http_requests_in_flight",
        "Number of requests that are currently in flight",
        TRACK_IN_FLIGHT.clone(),
    );
}

/// HTTP call metrics. Can be cheaply cloned.
/// Used by [`HttpCallMetrics`](crate::services::HttpCallMetrics).
#[derive(Default)]
pub struct HttpMetrics;

impl HttpMetrics {
    /// Tracks one call to the specified HTTP path and method.
    pub fn track<P: AsRef<str>, M: Into<HttpMethod>>(
        path: P,
        method: M,
        status: u16,
        elapsed: Duration,
    ) {
        let method = method.into();
        TRACK_ENDPOINT
            .get_or_create(&Labels {
                method: method.clone(),
                path: path.as_ref().to_string(),
                status,
            })
            .inc();

        TRACK_DURATION
            .get_or_create(&Labels {
                method,
                path: path.as_ref().to_string(),
                status,
            })
            .inc_by(elapsed.as_secs_f64());
    }

    pub fn inc_in_flight<P: AsRef<str>>(path: P) {
        TRACK_IN_FLIGHT
            .get_or_create(&InFlightLabels {
                path: path.as_ref().to_string(),
            })
            .inc();
    }

    pub fn dec_in_flight<P: AsRef<str>>(path: P) {
        TRACK_IN_FLIGHT
            .get_or_create(&InFlightLabels {
                path: path.as_ref().to_string(),
            })
            .dec();
    }
}
