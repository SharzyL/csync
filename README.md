# csync

csync is code/continuous sync, a CLI tool to sync changes in one directory to another in real time, written in Rust, powered by [notify](https://github.com/notify-rs/notify) crate.

csync by default synchronizes `.git` and respects `.gitignore`.

If you want to synchronize to a remote directory, use csync together with sshfs. Support for direct SSH access is planed.

This project is in its early development stage.

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
  -h, --help               Print help
  -V, --version            Print version
```
