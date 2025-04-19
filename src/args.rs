use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(version, about)]
pub(crate) struct CsyncArgs {
    pub source_dir: PathBuf,
    pub target_dir: PathBuf,

    /// Globs to ignore, can be specified multiple times
    #[arg(short, long, action = clap::ArgAction::Append)]
    pub ignore: Vec<String>,

    /// Do not respect .gitignore and .git/info/exclude
    #[arg(long)]
    pub no_git_ignore: bool,

    /// Do not sync deletion
    #[arg(long, default_value_t = false)]
    pub no_delete: bool,

    /// Use fast metadata-only comparison on initial sync
    #[arg(long)]
    pub fast_initial_sync: bool,

    /// Enable debug logging
    #[arg(long)]
    pub debug: bool,

    /// Enable trace logging (and debug logging)
    #[arg(long)]
    pub trace: bool,

    /// A debug mode that only prints events and not syncing things
    #[arg(long)]
    pub listen_only: bool,
}
