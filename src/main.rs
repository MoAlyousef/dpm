use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Deserialize, Serialize)]
struct Nix {
    packages: Vec<String>,
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    dry_run: bool,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Switch to the new configuration
    Switch,
    /// List nixme generations
    List,
    /// Rollsback to a previous generation
    Rollback { generation: String },
    /// Runs nix-collect-garbage
    Gc,
}

fn extract_gen(s: &fs::DirEntry) -> i32 {
    s.file_name()
        .to_string_lossy()
        .trim_start_matches("generation_")
        .trim_end_matches(".toml")
        .parse::<i32>()
        .unwrap_or(-1)
}

fn generation_files(dir: impl AsRef<Path>) -> anyhow::Result<Vec<fs::DirEntry>> {
    let mut paths: Vec<_> = fs::read_dir(dir.as_ref())?.filter_map(Result::ok).collect();
    paths.sort_by_key(extract_gen);
    Ok(paths)
}

fn latest_gen_file(dir: impl AsRef<Path>) -> Option<(PathBuf, u32)> {
    let mut paths = generation_files(dir.as_ref()).ok()?;
    let last = paths.pop();
    if let Some(last) = last {
        let n = extract_gen(&last);
        if n == -1 {
            None
        } else {
            Some((last.path(), n as u32))
        }
    } else {
        None
    }
}

fn diff_unique(old: &[String], new: &[String]) -> (Vec<String>, Vec<String>) {
    let old_set: HashSet<_> = old.iter().cloned().collect();
    let new_set: HashSet<_> = new.iter().cloned().collect();

    let added = new_set.difference(&old_set).cloned().collect();
    let removed = old_set.difference(&new_set).cloned().collect();

    (added, removed)
}

fn resolve_changes(added: &[String], removed: &[String], dry_run: bool) -> anyhow::Result<()> {
    if added.is_empty() && removed.is_empty() {
        println!("Nothing to resolve!");
        return Ok(());
    }
    if dry_run {
        if !removed.is_empty() {
            println!("nix-env -e {}", removed.join(" "));
        }
        if !added.is_empty() {
            println!("nix-env -iA nixpkgs.{}", added.join(" nixpkgs."));
        }
    } else {
        if !removed.is_empty() {
            let mut nix_env = Command::new("nix-env");
            nix_env.arg("-e");
            nix_env.args(removed);
            nix_env.spawn()?.wait()?;
        }
        if !added.is_empty() {
            let mut nix_env = Command::new("nix-env");
            nix_env.arg("-iA");
            for p in added {
                nix_env.arg(format!("nixpkgs.{}", p));
            }
            nix_env.spawn()?.wait()?;
        }
    }
    Ok(())
}

fn main() -> anyhow::Result<()> {
    // maybe we wanna work as root?
    let home = PathBuf::from(env::var("HOME").unwrap_or_default());
    let nix0 = fs::read_to_string(home.join(".nix-packages.toml"))
        .expect("No .nix-packages.toml file found");
    let cache0 = env::var("XDG_CACHE_HOME").unwrap_or_default();
    let cache = if cache0.is_empty() {
        home.join(".cache/nixme")
    } else {
        PathBuf::from(cache0)
    };
    if nix0.is_empty() {
        eprintln!("Empty .nix-packages.toml\nterminating!");
        return Ok(());
    }
    if !cache.exists() {
        fs::create_dir(&cache)?;
    }
    let nix: Nix = toml::from_str(&nix0)?;
    let latest_gen = latest_gen_file(&cache);
    let (latest_gen, n) = if let Some(f) = latest_gen {
        (fs::read_to_string(f.0)?, f.1)
    } else {
        let gen0 = cache.join("generation_0.toml");
        let s = "packages = []";
        fs::write(&gen0, s.as_bytes())?;
        // assuming the above worked!
        (s.to_owned(), 0)
    };
    let latest_gen: Nix = toml::from_str(&latest_gen)?;

    let args = Args::parse();
    match &args.command {
        Commands::Switch => {
            let (added, removed) = diff_unique(&latest_gen.packages, &nix.packages);
            resolve_changes(&added, &removed, args.dry_run)?;
            if (!removed.is_empty() || !added.is_empty()) && !args.dry_run {
                fs::write(
                    cache.join(format!("generation_{}.toml", n + 1)),
                    nix0.as_bytes(),
                )?;
            }
        }
        Commands::List => {
            let paths = generation_files(&cache)?;
            for path in paths {
                let p = &path;
                let time = chrono::DateTime::<chrono::Local>::from(p.metadata()?.created()?);
                println!(
                    "{}\t\t{}\t\t{}",
                    p.path().file_stem().unwrap().to_str().unwrap(),
                    time.date_naive(),
                    time.time()
                );
            }
        }
        Commands::Rollback { generation } => {
            let new_gen: Nix = toml::from_str(&fs::read_to_string(
                cache.join(format!("{generation}.toml")),
            )?)?;
            let (added, removed) = diff_unique(&latest_gen.packages, &new_gen.packages);
            resolve_changes(&added, &removed, args.dry_run)?;
        }
        Commands::Gc => {
            if args.dry_run {
                println!("nix-collect-garbage");
            } else {
                let mut nix_gc = Command::new("nix-collect-garbage");
                nix_gc.spawn()?.wait()?;
            }
        }
    }

    Ok(())
}
