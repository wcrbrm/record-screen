use atty::Stream;
use tracing_error::ErrorLayer;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

pub fn start(defaults: &str) {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or(EnvFilter::new(defaults));
    let is_terminal = atty::is(Stream::Stdout);
    let subscriber = tracing_subscriber::fmt::fmt()
        .with_env_filter(env_filter)
        .with_ansi(is_terminal)
        .with_span_events(fmt::format::FmtSpan::CLOSE) // enable durations
        .finish();
    _ = subscriber.with(ErrorLayer::default()).try_init();
}
