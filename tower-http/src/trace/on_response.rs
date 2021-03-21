use super::DEFAULT_MESSAGE_LEVEL;
use crate::LatencyUnit;
use http::Response;
use std::time::Duration;
use tracing::Level;

pub trait OnResponse<B> {
    fn on_response(self, response: &Response<B>, latency: Duration);
}

impl<B> OnResponse<B> for () {
    #[inline]
    fn on_response(self, _: &Response<B>, _: Duration) {}
}

impl<B, F> OnResponse<B> for F
where
    F: FnOnce(&Response<B>, Duration),
{
    fn on_response(self, response: &Response<B>, latency: Duration) {
        self(response, latency)
    }
}

#[derive(Clone, Debug)]
pub struct DefaultOnResponse {
    level: Level,
    latency_unit: LatencyUnit,
}

impl Default for DefaultOnResponse {
    fn default() -> Self {
        Self {
            level: DEFAULT_MESSAGE_LEVEL,
            latency_unit: LatencyUnit::Millis,
        }
    }
}

impl DefaultOnResponse {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn level(mut self, level: Level) -> Self {
        self.level = level;
        self
    }

    pub fn latency_unit(mut self, latency_unit: LatencyUnit) -> Self {
        self.latency_unit = latency_unit;
        self
    }
}

// Repeating this pattern match for each case is tedious. So we do it with a quick and
// dirty macro.
//
// Tracing requires all these parts to be declared statically. You cannot easily build
// events dynamically.
#[allow(unused_macros)]
macro_rules! log_pattern_match {
    (
        $this:expr, $latency:expr, $status:expr, [$($level:ident),*]
    ) => {
        match ($this.level, $this.latency_unit) {
            $(
                (Level::$level, LatencyUnit::Millis) => {
                    tracing::event!(
                        Level::$level,
                        latency = format_args!("{} ms", $latency.as_millis()),
                        status = $status,
                        "finished processing request"
                    );
                }
                (Level::$level, LatencyUnit::Nanos) => {
                    tracing::event!(
                        Level::$level,
                        latency = format_args!("{} ns", $latency.as_nanos()),
                        status = $status,
                        "finished processing request"
                    );
                }
            )*
        }
    };
}

impl<B> OnResponse<B> for DefaultOnResponse {
    fn on_response(self, response: &Response<B>, latency: Duration) {
        let status = status(response);
        log_pattern_match!(self, latency, status, [ERROR, WARN, INFO, DEBUG, TRACE]);
    }
}

fn status<B>(res: &Response<B>) -> i32 {
    let is_grpc = res
        .headers()
        .get(http::header::CONTENT_TYPE)
        .map_or(false, |value| value == "application/grpc");

    if is_grpc {
        if let Some(Err(status)) = crate::classify::classify_grpc_metadata(res.headers()) {
            status
        } else {
            // 0 is success in gRPC
            0
        }
    } else {
        res.status().as_u16().into()
    }
}
