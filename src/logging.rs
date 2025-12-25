use std::sync::Once;

use tracing_subscriber::{EnvFilter, fmt};

static INIT: Once = Once::new();

/// Initialize a global tracing subscriber for application runtime.
pub fn init() {
    INIT.call_once(|| setup(false));
}

/// Initialize tracing with a test-friendly writer so log output is visible during `cargo test`.
#[cfg(test)]
pub fn init_for_tests() {
    INIT.call_once(|| setup(true));
}

/// Configure the tracing subscriber, optionally using the test writer.
fn setup(for_tests: bool) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("trace"));
    let builder = fmt::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_thread_names(true)
        .with_level(true);

    if for_tests {
        builder.with_test_writer().init();
    } else {
        builder.init();
    }
}
