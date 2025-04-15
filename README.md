# csync: Real-Time Directory Synchronization Tool

**csync** (code/continuous sync) is a CLI tool written in Rust that synchronizes changes between directories in real time. It's powered by the [notify](https://github.com/notify-rs/notify) crate.

To synchronize with remote directories, use **csync** in conjunction with sshfs. Direct SSH access support is planned for future releases.

**csync** is constrained by the capabilities of the notify crate. For detailed information about these limitations, please refer to [the notify documentation](https://docs.rs/notify/latest/notify/index.html#known-problems).

This project is currently in early development. The following is a list of planed improvements:

- Parallelize initial synchronization process
- Potentially implement async Rust architecture
- Implement debouncing for large file write operations
- Add PollWatcher implementation for environments where necessary
- Develop direct SSH access functionality
- Add comprehensive test suite
- Optimize git index access to reduce the cost of large file copies

## Build

```console
$ cargo build
```

Or if you are using Nix
```console
$ nix build
```

## Usage

```console
Usage: csync [OPTIONS] <SOURCE_DIR> <TARGET_DIR>
Arguments:
  <SOURCE_DIR>  
  <TARGET_DIR>  
Options:
  -i, --ignore <IGNORE>    Globs to ignore, can be specified multiple times
      --no-git-ignore      Do not respect .gitignore and .git/info/exclude
      --no-delete          Do not sync deletion
      --fast-initial-sync  Use fast metadata-only comparison on initial sync
      --debug              Enable debug logging
      --trace              Enable trace logging (and debug logging)
      --listen-only        A debug mode that only prints events and not syncing things
  -h, --help               Print help
  -V, --version            Print version
```
