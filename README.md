# dpmm

A simplistic crossplatform declarative package manager manager.

## Usage
```bash
Usage: dpmm [OPTIONS] <COMMAND>

Commands:
  switch    Switch to the new configuration
  list      List dpmm generations
  rollback  Rollsback to a previous generation
  update    Update package list
  upgrade   Upgrade packages
  help      Print this message or the help of the given subcommand(s)

Options:
  -d, --dry-run  
  -h, --help     Print help
  -V, --version  Print version
```

example $HOME/config/dpmm/dpmm.toml:
```toml
managers = ["apt", "brew"]
```

$HOME/.config/dpmm/apt.toml:
```toml
install = "sudo apt-get install -y $"
uninstall = "sudo apt-get purge -y $"
packages = [
  "jq",
  "vim"
]
```

$HOME/.config/dpmm/brew.toml:
```toml
install = "brew install $"
uninstall = "brew uninstall $"
packages = [
  "gcc@14"
]
```

## Building
Requires a fairly recent Rust version:
```bash
git clone https://github.com/MoAlyousef/dpmm
cd dpmm
cargo build --release
```

Copy the resulting binary into a PATH directory.

## Info

The Dpmm format:
```toml
# These identify the managers in your config directory, so apt for example maps to apt.toml, brew maps to brew.toml.
# These are also handled sequentially
managers = ["apt", "brew"]
```
The Dpm format:
```toml
# OPTIONAL: the file's stem is used to identify the manager
name = "apt"
# OPTIONAL
update = "sudo apt-get update"
# OPTIONAL
upgrade = "sudo apt-get upgrade -y"

install = "sudo apt-get install -y $"
uninstall = "sudo apt-get purge -y $"

# OPTIONAL, specifies whether the install/uninstall commands can have multiple package names.
# The default is true
supports_multi_args = true

packages = [
  "jq",
  "vim"
]
```

The dpmm update and upgrade commands, require the name of the manager, or an explicit `all` argument. This is to avoid breaking updates/upgrades.