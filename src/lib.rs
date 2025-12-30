mod args;
mod cache;
mod csync;

pub use args::CsyncArgs;
pub use csync::Csync;

use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

pub fn setup_logger(debug: bool, trace: bool) {
    let default_level = if trace {
        "trace"
    } else if debug {
        "debug"
    } else {
        "info"
    };

    let env_filter = EnvFilter::from_default_env()
        .add_directive(default_level.parse().unwrap())
        .add_directive("ignore=off".parse().unwrap())
        .add_directive("globset=off".parse().unwrap());

    let mut fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(false)
        .with_level(true);

    if trace {
        fmt_layer = fmt_layer.with_target(true)
    }

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .init();
}
