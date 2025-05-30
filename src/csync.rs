use crate::cache::TTLCache;
use anyhow::{anyhow, bail, Context};
use filetime::FileTime;
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use ignore::WalkBuilder;
use notify::event::{ModifyKind, RenameMode};
use notify::{Event, EventKind};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tracing::{debug, error, info, instrument, trace, warn};

type Result<T> = anyhow::Result<T>;

pub(crate) struct Csync {
    source_dir: PathBuf,
    target_dir: PathBuf,
    no_delete: bool,

    gitignore: Gitignore,
    moved_from_cache: TTLCache<usize, PathBuf>,
    untrackable_moved_from_cache: TTLCache<PathBuf, ()>,
    ephemeral_cache: TTLCache<PathBuf, ()>,
}

impl Csync {
    pub fn new(
        source_dir: &Path,
        target_dir: &Path,
        ignore_patterns: &Vec<String>,
        use_gitignore: bool,
        no_delete: bool,
    ) -> Result<Self> {
        // For various editors, when the editor writes a file, it performs the following steps:
        //     1. MOVE file to file~
        //     2. CREATE file
        //     3. MODIFY file
        //     4. ATTRIB file
        //     5. DELETE file~
        //
        // To avoid unnecessary sync operation,
        // - When detecting a MOVED_FROM event, we add the relpath to moved_from_cache
        // - When detecting a MOVED_TO event of file `xxx~`, and `xxx` to moved_from_cache,
        //   Add `xxx` to ephemeral_file_cache and remove `xxx` from moved_from_cache
        //   Deletion of `xxx` is not synced
        // - When items in moved_from_cache expires, sync deletion
        // - When detecting a CREATE, MODIFY event of file `xxx` and `xxx` in ephemeral_file_cache, ignore the event
        // - When detecting a ATTRIB event of file `xxx` and `xxx` in ephemeral_file_cache,
        //   sync file and remove from ephemeral_file_cache
        // - When items in ephemeral_file_cache expires, sync `xxx` and `xxx~`
        Ok(Self {
            source_dir: source_dir.to_path_buf(),
            target_dir: target_dir.to_path_buf(),
            gitignore: Self::new_gitignore(source_dir, use_gitignore, ignore_patterns)?,
            no_delete,
            moved_from_cache: TTLCache::new(Duration::from_millis(100)),
            untrackable_moved_from_cache: TTLCache::new(Duration::from_millis(100)),
            ephemeral_cache: TTLCache::new(Duration::from_millis(200)),
        })
    }

    #[instrument(skip_all)]
    pub fn initial_sync(&self, fast: bool) -> Result<()> {
        info!(
            "Starting initial sync from {} to {}",
            self.source_dir.display(),
            self.target_dir.display()
        );

        let start_time = Instant::now();
        let mut synced_files = 0;
        for entry in WalkBuilder::new(&self.source_dir).build() {
            let entry = entry?;
            let src_path = entry.path();
            let rel_path = match src_path.strip_prefix(&self.source_dir) {
                Ok(p) => p,
                Err(_) => continue, // Skip entries not in source dir
            };

            // Skip the root directory itself
            if rel_path.to_str() == Some("") {
                continue;
            }

            // Check ignore patterns
            // TODO: the ignore mechanism in initial sync is slightly different from later syncs
            // because initial sync will respect recursive .gitignore
            if self.should_ignore(rel_path) {
                continue;
            }

            let dest_path = self.target_dir.join(rel_path);

            trace!("Syncing {src_path:?}");
            self.sync_file(src_path, dest_path.as_path(), fast)?;
            synced_files += 1;
        }

        info!(
            "Initial sync completed in {:.2?}. Synced {synced_files} files",
            start_time.elapsed(),
        );

        Ok(())
    }

    pub fn check_cache(&mut self) -> Result<()> {
        self.moved_from_cache
            .expire(|_, src_rel_path| -> Result<()> {
                let dest_path = self.target_dir.join(&src_rel_path);
                info!("Delete {:?} for expired moved_from cache", src_rel_path);
                let src_path = self.source_dir.join(&src_rel_path);
                if !src_path.exists() && dest_path.exists() && !self.no_delete {
                    self.delete_file(&dest_path)
                        .context("Failed to delete on checking moved_from_cache")?
                }
                Ok(())
            })?;

        self.untrackable_moved_from_cache
            .expire(|src_rel_path, _| {
                let dest_path = self.target_dir.join(&src_rel_path);
                let src_path = self.source_dir.join(&src_rel_path);
                if !src_path.exists() && dest_path.exists() && !self.no_delete {
                    info!("Delete {:?} for expired moved_from cache", src_rel_path);
                    self.delete_file(&dest_path)
                        .context("Failed to delete on checking untrackable_moved_from_cache")?
                }
                Ok(())
            })?;

        self.ephemeral_cache.expire(|src_rel_path, _| {
            let dest_path = self.target_dir.join(&src_rel_path);
            let src_path = self.source_dir.join(&src_rel_path);
            if self.source_dir.join(&src_path).exists() {
                info!("Delete {:?} for expired moved_from cache", src_rel_path);
                self.sync_file(&self.source_dir.join(&src_path), &dest_path, false)
                    .context("Failed to sync on checking ephemeral_cache")?;
            } else if dest_path.exists() {
                self.delete_file(&dest_path)
                    .context("Failed to delete on checking ephemeral_cache")?;
            }
            Ok(())
        })?;

        Ok(())
    }

    pub fn handle_event(&mut self, event: &Event) {
        if let Err(e) = match event.kind {
            EventKind::Create(_) => self.handle_create(event),
            EventKind::Remove(_) => self.handle_remove(event),
            EventKind::Modify(modify) => match modify {
                ModifyKind::Data(_) => self.handle_modify(event, false),
                ModifyKind::Metadata(_) => self.handle_modify(event, true),
                ModifyKind::Name(rename) => match rename {
                    RenameMode::To => self.handle_moved_to(event),
                    RenameMode::From => self.handle_moved_from(event),
                    RenameMode::Both => self.handle_moved_both(event),
                    _ => {
                        debug!("unknown rename event: {event:?}");
                        Ok(())
                    }
                },
                _ => {
                    debug!("unknown modify event: {event:?}");
                    Ok(())
                }
            },
            EventKind::Access(_) => Ok(()),
            _ => {
                debug!("unknown event {event:?}");
                Ok(())
            }
        } {
            error!("error on handling {event:?}: {e:?}");
        }
    }

    fn handler_common<'a>(
        &mut self,
        event: &'a Event,
    ) -> Result<Option<(&'a Path, &'a Path, PathBuf)>> {
        // TODO: handle multiple paths
        let src_path = event.paths.first().ok_or(anyhow!("no path in event"))?;

        let Ok(rel_path) = src_path.strip_prefix(&self.source_dir) else {
            warn!("Create event outside source dir: {src_path:?}");
            return Ok(None);
        };

        if self.should_ignore(rel_path) {
            debug!("Ignored {rel_path:?}");
            return Ok(None);
        }
        let dest_path = self.target_dir.join(rel_path);
        Ok(Some((src_path, rel_path, dest_path)))
    }

    #[instrument(skip_all)]
    fn handle_create(&mut self, event: &Event) -> Result<()> {
        let (src_path, rel_path, dest_path) = match self.handler_common(event)? {
            Some(x) => x,
            None => return Ok(()),
        };

        // handling temporary files that disappears quickly
        std::thread::sleep(Duration::from_millis(100));

        if self.ephemeral_cache.get(&rel_path.to_path_buf()).is_some() {
            debug!("Skipping ephemeral file creation of {rel_path:?}");
            return Ok(());
        }

        if src_path.exists() {
            info!("Syncing new file: {rel_path:?}");
            self.sync_file(src_path, &dest_path, false)
                .context("Failed to sync")
        } else {
            debug!("Detected CREATE event for {rel_path:?} but it is missing");
            Ok(())
        }
    }

    #[instrument(skip_all)]
    fn handle_remove(&mut self, event: &Event) -> Result<()> {
        if self.no_delete {
            return Ok(());
        }

        let (_, rel_path, dest_path) = match self.handler_common(event)? {
            Some(x) => x,
            None => return Ok(()),
        };

        if dest_path.exists() {
            info!("Syncing deletion: {rel_path:?}");
            self.delete_file(&dest_path)
        } else {
            debug!("Detected REMOVE event for {rel_path:?} but it is missing");
            Ok(())
        }
    }

    #[instrument(skip_all)]
    // TODO: handle metadata only modify
    fn handle_modify(&mut self, event: &Event, metadata_only: bool) -> Result<()> {
        let (src_path, rel_path, dest_path) = match self.handler_common(event)? {
            Some(x) => x,
            None => return Ok(()),
        };

        // skip directory modify
        if src_path.is_dir() {
            debug!("Ignoring modify event: {src_path:?}");
            return Ok(());
        }

        let in_ephermeral_cache = self.ephemeral_cache.get(&rel_path.to_path_buf()).is_some();
        if in_ephermeral_cache {
            if metadata_only {
                // handle ATTRIB
                debug!(
                    "Detected ATTRIB event on {rel_path:?}, remove it from ephemeral_cache and continue syncing"
                );
                self.ephemeral_cache.del(&rel_path.to_path_buf());
            } else {
                // handle MODIFY
                debug!("Skipping ephemeral file modification of {rel_path:?}");
                return Ok(());
            }
        }

        if src_path.exists() {
            // if in_ephermeral_cache, we need to sync file content on ATTRIB
            // because we skipped sync content on MODIFY
            if metadata_only && dest_path.exists() && !in_ephermeral_cache {
                info!("Syncing metadata: {rel_path:?}");
                self.sync_metadata(src_path, &dest_path)
            } else {
                info!("Syncing file: {rel_path:?}");
                self.sync_file(src_path, &dest_path, false)
            }
        } else {
            debug!("Detected MODIFY event for {rel_path:?} but it is missing");
            Ok(())
        }
    }

    #[instrument(skip_all)]
    fn handle_moved_from(&mut self, event: &Event) -> Result<()> {
        let (_, rel_path, dest_path) = match self.handler_common(event)? {
            Some(x) => x,
            None => return Ok(()),
        };

        if dest_path.exists() {
            match event.attrs.tracker() {
                Some(cookie) => {
                    debug!(
                        cookie = cookie,
                        "Detected MOVED_FROM event for {rel_path:?}, add in to moved_from_cache"
                    );
                    self.moved_from_cache.set(cookie, rel_path.to_path_buf());
                }
                None => {
                    debug!(
                        "Detected MOVED_FROM event for {rel_path:?}, add in to untrackable_moved_from_cache"
                    );
                    self.untrackable_moved_from_cache
                        .set(rel_path.to_path_buf(), ());
                }
            }
        }
        Ok(())
    }

    #[instrument(skip_all)]
    fn handle_moved_to(&mut self, event: &Event) -> Result<()> {
        let (src_path, rel_path, dest_path) = match self.handler_common(event)? {
            Some(x) => x,
            None => return Ok(()),
        };

        fn equals_to_with_tilde(p1: &Path, p2: &Path) -> bool {
            p1.file_name()
                .and_then(|n| n.to_str())
                .map(|s| p1.with_file_name(format!("{s}~")) == p2)
                .unwrap_or(false)
        }

        if let Some(cookie) = event.attrs.tracker() {
            // if we know where it is from
            if let Some(from_rel_path) = self.moved_from_cache.get(&cookie) {
                // if some_file is moved to some_file~, put it in ephemeral_cache
                if equals_to_with_tilde(&from_rel_path, rel_path) {
                    debug!("Detected MOVED_TO event for {rel_path:?}, add to ephemeral_cache");
                    self.ephemeral_cache.set(from_rel_path, ())
                } else {
                    // else sync the move
                    let from_rel_path_in_dest = self.target_dir.join(&from_rel_path);
                    if src_path.exists() {
                        info!(
                            cookie = cookie,
                            "Detected MOVED_TO event of {rel_path:?} from {from_rel_path:?}, syncing the move",
                        );
                        self.delete_file(&from_rel_path_in_dest)?;
                        self.sync_file(src_path, &dest_path, false)?;
                    }
                }
            }
            self.moved_from_cache.del(&cookie);
        } else if src_path.exists() {
            // we do not know where it is from
            info!("Detected MOVED_TO event for {rel_path:?}, syncing");
            self.sync_file(src_path, &dest_path, false)?;
        } else {
            debug!("Detected MOVED_TO event for {rel_path:?} but it is missing")
        }

        Ok(())
    }

    #[instrument(skip_all)]
    fn handle_moved_both(&mut self, event: &Event) -> Result<()> {
        let src_from_path = event.paths.first().ok_or(anyhow!("no path in event"))?;
        let src_to_path = event
            .paths
            .get(1)
            .ok_or(anyhow!("no second path in event"))?;

        let Ok(rel_from_path) = src_from_path.strip_prefix(&self.source_dir) else {
            warn!("MOVED_FROM event outside source dir: {src_from_path:?}");
            return Ok(());
        };

        let Ok(rel_to_path) = src_to_path.strip_prefix(&self.source_dir) else {
            warn!("MOVED_TO event outside source dir: {src_to_path:?}");
            return Ok(());
        };

        if self.should_ignore(rel_from_path) {
            debug!("Ignored MOVED_FROM event: {rel_from_path:?} (to {rel_to_path:?})");
        } else {
            let dest_from_path = self.target_dir.join(rel_from_path);
            if !src_from_path.exists() && dest_from_path.exists() {
                info!(
                    "Syncing deletion for MOVED_FROM event of {rel_from_path:?} (to {rel_to_path:?})"
                );
                self.delete_file(&dest_from_path)?;
            } else {
                debug!("Detected MOVED_FROM event for {rel_from_path:?} no need to sync deletion");
            }
        }

        if self.should_ignore(rel_to_path) {
            debug!("Ignored MOVED_TO event: {rel_to_path:?} (from {rel_from_path:?})");
        } else {
            let dest_to_path = self.target_dir.join(rel_to_path);
            if src_to_path.exists() {
                info!(
                    "Syncing file for MOVED_TO event of {rel_to_path:?} (from {rel_from_path:?})"
                );
                self.sync_file(src_to_path, &dest_to_path, false)?;
            } else {
                debug!(
                    "Detected MOVED_TO event for {rel_to_path:?} (from {rel_from_path:?}) but it is missing"
                );
            }
        }

        Ok(())
    }

    fn new_gitignore(
        source_dir: &Path,
        use_gitignore: bool,
        ignore_patterns: &Vec<String>,
    ) -> Result<Gitignore> {
        let mut builder = GitignoreBuilder::new(&source_dir);
        for pattern in ignore_patterns {
            builder
                .add_line(None, &pattern)
                .context("Failed to add ignore pattern")?;
        }
        builder.add_line(None, ".git/**/*.lock")?;
        builder.add_line(None, ".git/objects/*/tmp_obj_*")?;
        builder.add_line(None, ".git/COMMIT_EDITMSG")?;

        if use_gitignore {
            Self::load_gitignore(&source_dir, &mut builder)
                .context("Failed to load git ignore file")?;
        }

        builder.build().context("Failed to build GitignoreBuilder")
    }

    fn load_gitignore(dir: &Path, builder: &mut GitignoreBuilder) -> Result<()> {
        let gitignore = dir.join(".gitignore");
        if gitignore.exists() {
            if let Some(err) = builder.add(&gitignore) {
                Err(anyhow!(err))?
            }
            debug!("Added ignore file: {gitignore:?}");
        }

        let git_exclude = dir.join(".git").join("info").join("exclude");
        if git_exclude.exists() {
            if let Some(err) = builder.add(&git_exclude) {
                Err(anyhow!(err))?
            }
            debug!("Added ignore file: {git_exclude:?}");
        }

        if let Some(parent) = dir.parent() {
            Self::load_gitignore(parent, builder)?;
        }
        Ok(())
    }

    fn should_ignore(&self, path: &Path) -> bool {
        self.gitignore
            .matched_path_or_any_parents(path, false)
            .is_ignore()
    }

    fn sync_metadata(&self, src: &Path, dest: &Path) -> Result<()> {
        let src_metadata =
            std::fs::metadata(src).context(format!("Failed to get metadata for src {src:?}"))?;
        let dest_metadata =
            std::fs::metadata(dest).context(format!("Failed to get metadata for src {dest:?}"))?;

        if let Ok(src_mtime) = src_metadata.modified() {
            if let Ok(dest_mtime) = src_metadata.modified() {
                if src_mtime != dest_mtime {
                    debug!("syncing mtime of {src:?}");
                    filetime::set_file_mtime(dest, FileTime::from_system_time(src_mtime))
                        .context("Failed to set mtime")?
                }
            }
        };

        if src_metadata.permissions() != dest_metadata.permissions() {
            debug!("syncing permissions of {src:?}");
            std::fs::set_permissions(dest, src_metadata.permissions())
                .context("Failed to set permissions")?;
        }
        Ok(())
    }

    fn sync_file(&self, src: &Path, dest: &Path, fast_check: bool) -> Result<()> {
        if src.is_dir() {
            std::fs::create_dir_all(dest)?;
        } else {
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)
                    .context(format!("Failed to create parent directory {parent:?}",))?
            }

            if fast_check {
                if files_equal_fast(src, dest)? {
                    debug!("Skipping file sync by metadata comparison: {src:?}");
                }
            }

            if dest.exists() {
                self.sync_metadata(src, dest)?;
            }

            if let Err(e) = std::fs::copy(src, dest) {
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    if files_equal(src, dest).unwrap_or(false) {
                        debug!("Skipping permission denied but unchanged file {src:?}");
                    } else {
                        bail!("Permission denied for changed file {dest:?}")
                    }
                } else {
                    Err(e).context(format!("Failed to copy {src:?} to {dest:?}"))?
                }
            };

            // sync action will be logged outside, no repeated log
        }
        Ok(())
    }

    #[instrument(skip(self))]
    fn delete_file(&self, dest: &Path) -> Result<()> {
        if !dest.exists() {
            return Ok(());
        }

        if dest.is_dir() {
            std::fs::remove_dir_all(dest)
        } else {
            std::fs::remove_file(dest)
        }
        .context(format!("Failed to remove {}", dest.display()))
        // delete action will be logged outside, no repeated log
    }
}

fn files_equal(a: &Path, b: &Path) -> Result<bool> {
    let mut fa = std::fs::File::open(a)?;
    let mut fb = std::fs::File::open(b)?;
    let mut buf_a = [0; 4096];
    let mut buf_b = [0; 4096];

    loop {
        let read_a = fa.read(&mut buf_a)?;
        let read_b = fb.read(&mut buf_b)?;

        if read_a != read_b {
            return Ok(false);
        }
        if read_a == 0 {
            return Ok(true);
        }
        if buf_a[..read_a] != buf_b[..read_b] {
            return Ok(false);
        }
    }
}

fn files_equal_fast(a: &Path, b: &Path) -> Result<bool> {
    let a_meta = a.metadata()?;
    let b_meta = b.metadata()?;

    Ok(a_meta.len() == b_meta.len()
        && FileTime::from_last_modification_time(&a_meta)
            == FileTime::from_last_modification_time(&b_meta))
}
