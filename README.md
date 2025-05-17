# nixme

A simplistic nix manager engine, which only manages packages. No dotfile management!
Configurable using a .nix-packages.toml file.

## Usage
```bash
Usage: nixme [OPTIONS] <COMMAND>

Commands:
  switch    Switch to the new configuration
  list      List nixme generations
  rollback  Rollsback to a previous generation
  gc        Runs nix-collect-garbage
  help      Print this message or the help of the given subcommand(s)

Options:
  -d, --dry-run
  -h, --help     Print help
  -V, --version  Print version
```

example $HOME/.nix-packages.toml:
```toml
packages = [
        "curl",
        "wget",
        ]
```

## Building
Requires a fairly recent Rust version:
```bash
git clone https://github.com/MoAlyousef/nixme
cd nixme
cargo build --release
```

Copy the resulting binary into a PATH directory.