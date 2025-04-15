mod cache;
mod csync;

use crate::csync::Csync;
use anyhow::Context;
use clap::Parser;
use notify::{RecursiveMode, Watcher, recommended_watcher};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

type Result<T> = anyhow::Result<T>;

#[derive(Parser)]
#[command(version, about)]
struct Args {
    source_dir: PathBuf,
    target_dir: PathBuf,

    /// Globs to ignore, can be specified multiple times
    #[arg(short, long, action = clap::ArgAction::Append)]
    ignore: Vec<String>,

    /// Do not respect .gitignore and .git/info/exclude
    #[arg(long)]
    no_git_ignore: bool,

    /// Do not sync deletion
    #[arg(long, default_value_t = false)]
    no_delete: bool,

    /// Use fast metadata-only comparison on initial sync
    #[arg(long)]
    fast_initial_sync: bool,

    /// Enable debug logging
    #[arg(long)]
    debug: bool,

    /// Enable trace logging (and debug logging)
    #[arg(long)]
    trace: bool,

    /// A debug mode that only prints event and do nothing
    #[arg(long)]
    listen_only: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    setup_logger(args.debug, args.trace);

    if !args.target_dir.exists() {
        info!("Creating non-existent target_dir {:?}", args.target_dir);
        std::fs::create_dir(&args.target_dir).context("Failed to create target directory")?;
    } else if !args.target_dir.is_dir() {
        error!("target_dir {:?} is not directory", args.target_dir);
    }

    let source_dir = args
        .source_dir
        .canonicalize()
        .context(format!("Failed to canonicalize {:?}", args.source_dir))?;
    let target_dir = args
        .target_dir
        .canonicalize()
        .context(format!("Failed to canonicalize {:?}", args.target_dir))?;

    if args.listen_only {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut watcher = recommended_watcher(tx)?;
        watcher
            .watch(&source_dir, RecursiveMode::Recursive)
            .context(format!("Failed to watch {source_dir:?}"))?;

        for event in rx {
            if let Ok(event) = event {
                if !matches!(event.kind, notify::EventKind::Access(_)) {
                    info!("{:?}", event);
                }
            }
        }

        return Ok(());
    }

    let csync = Arc::new(Mutex::new(Csync::new(
        &source_dir,
        &target_dir,
        &args.ignore,
        !args.no_git_ignore,
        args.no_delete,
    )?));

    csync
        .lock()
        .unwrap()
        .initial_sync(args.fast_initial_sync)
        .context(format!("Failed to perform initial sync {source_dir:?}"))?;

    let handler_clone = csync.clone();
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_millis(100));
            if let Err(e) = handler_clone.lock().unwrap().check_cache() {
                error!("Error on check_cache loop: {e:?}");
            }
        }
    });

    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher = recommended_watcher(tx)?;
    watcher
        .watch(&source_dir, RecursiveMode::Recursive)
        .context(format!("Failed to watch {source_dir:?}"))?;

    for event in rx {
        if let Ok(event) = event {
            csync.lock().unwrap().handle_event(&event);
        }
    }

    Ok(())
}

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
