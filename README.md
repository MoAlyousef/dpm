# dpm

A simplistic declarative package manager for linux, which only manages packages. No dotfile management!
Configurable using a $HOME/.dpm.toml file.

## Usage
```bash
Usage: dpm [OPTIONS] <COMMAND>

Commands:
  switch    Switch to the new configuration
  list      List dpm generations
  rollback  Rollsback to a previous generation
  update    Update package list
  upgrade   Upgrade packages
  help      Print this message or the help of the given subcommand(s)

Options:
  -d, --dry-run  
  -h, --help     Print help
  -V, --version  Print version
```

example $HOME/.dpm.toml:
```toml
packages = [
        "curl",
        "wget",
        ]
```

## Building
Requires a fairly recent Rust version:
```bash
git clone https://github.com/MoAlyousef/dpm
cd dpm
cargo build --release
```

Copy the resulting binary into a PATH directory.