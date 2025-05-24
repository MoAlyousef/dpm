use anyhow::Context;
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Debug, Deserialize, Serialize)]
struct Dpm {
    update: String,
    upgrade: String,
    install: String,
    uninstall: String,
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
    /// List dpm generations
    List,
    /// Rollsback to a previous generation
    Rollback { generation: Option<String> },
    /// Update package list
    Update,
    /// Upgrade packages
    Upgrade,
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
    Ok(paths.into_iter().rev().collect())
}

fn get_gen_file(dir: impl AsRef<Path>, idx: usize) -> Option<(PathBuf, u32)> {
    let paths = generation_files(dir.as_ref()).ok()?;
    let f = paths.get(idx);
    if let Some(f) = f {
        let n = extract_gen(&f);
        if n == -1 {
            None
        } else {
            Some((f.path(), n as u32))
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

fn resolve_changes(
    install_cmd: &str,
    uninstall_cmd: &str,
    added: &[String],
    removed: &[String],
    dry_run: bool,
) -> anyhow::Result<()> {
    if added.is_empty() && removed.is_empty() {
        println!("Nothing to resolve!");
        return Ok(());
    }
    if dry_run {
        if !removed.is_empty() {
            println!("{uninstall_cmd} {}", removed.join(" "));
        }
        if !added.is_empty() {
            println!("{install_cmd} {}", added.join(" "));
        }
    } else {
        if !removed.is_empty() {
            let cmd_n_args: Vec<_> = uninstall_cmd.split_whitespace().collect();
            let mut cmd = Command::new(cmd_n_args[0]);
            cmd.args(&cmd_n_args[1..]);
            cmd.args(removed);
            cmd.spawn()?.wait()?;
        }
        if !added.is_empty() {
            let cmd_n_args: Vec<_> = install_cmd.split_whitespace().collect();
            let mut cmd = Command::new(cmd_n_args[0]);
            cmd.args(&cmd_n_args[1..]);
            cmd.args(added);
            cmd.spawn()?.wait()?;
        }
    }
    Ok(())
}

fn main() -> anyhow::Result<()> {
    // maybe we wanna work as root?
    let home = PathBuf::from(env::var("HOME").unwrap_or_default());
    let dpm_toml = fs::read_to_string(home.join(".dpm.toml"))
        .expect("No .dpm.toml file found");
    let cache0 = env::var("XDG_CACHE_HOME").unwrap_or_default();
    let cache = if cache0.is_empty() {
        home.join(".cache/dpm")
    } else {
        PathBuf::from(cache0)
    };
    if dpm_toml.is_empty() {
        eprintln!("Empty .dpm.toml\nterminating!");
        return Ok(());
    }
    if !cache.exists() {
        fs::create_dir(&cache)?;
    }
    let dpm: Dpm = toml::from_str(&dpm_toml)?;
    let latest_gen = get_gen_file(&cache, 0);
    let (latest_gen, n) = if let Some(f) = latest_gen {
        (toml::from_str(&fs::read_to_string(f.0)?)?, f.1)
    } else {
        let gen0 = cache.join("generation_0.toml");
        let mut dpm0 = dpm.clone();
        dpm0.packages.clear();
        fs::write(&gen0, toml::to_string(&dpm0)?.as_bytes())?;
        // assuming the above worked!
        (dpm0, 0)
    };

    let args = Args::parse();
    match &args.command {
        Commands::Switch => {
            let (added, removed) = diff_unique(&latest_gen.packages, &dpm.packages);
            resolve_changes(&dpm.install, &dpm.uninstall, &added, &removed, args.dry_run)?;
            if (!removed.is_empty() || !added.is_empty()) && !args.dry_run {
                fs::write(
                    cache.join(format!("generation_{}.toml", n + 1)),
                    dpm_toml.as_bytes(),
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
                    p.path()
                        .file_stem()
                        .context("Failed to get stem")?
                        .to_str()
                        .context("Failed to convert file name to str")?,
                    time.date_naive(),
                    time.time()
                );
            }
        }
        Commands::Rollback { generation } => {
            let new_gen: Dpm = if let Some(generation) = generation {
                toml::from_str(&fs::read_to_string(
                    cache.join(format!("{generation}.toml")),
                )?)?
            } else {
                toml::from_str(&fs::read_to_string(
                    get_gen_file(&cache, 1).context("Failed to get last generation file")?.0,
                )?)?
            };
            let (added, removed) = diff_unique(&latest_gen.packages, &new_gen.packages);
            resolve_changes(&dpm.install, &dpm.uninstall, &added, &removed, args.dry_run)?;
            fs::write(home.join(".dpm.toml"), toml::to_string(&new_gen)?.as_bytes())?;
        }
        Commands::Update => {
            if args.dry_run {
                println!("{}", dpm.update);
            } else {
                let cmd_n_args: Vec<_> = dpm.update.split_whitespace().collect();
                let mut dpm_gc = Command::new(cmd_n_args[0]);
                dpm_gc.args(&cmd_n_args[1..]);
                dpm_gc.spawn()?.wait()?;
            }
        }
        Commands::Upgrade => {
            if args.dry_run {
                println!("{}", dpm.upgrade);
            } else {
                let cmd_n_args: Vec<_> = dpm.upgrade.split_whitespace().collect();
                let mut dpm_gc = Command::new(cmd_n_args[0]);
                dpm_gc.args(&cmd_n_args[1..]);
                dpm_gc.spawn()?.wait()?;
            }
        }
    }
    Ok(())
}
