use anyhow::Context;
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Debug, Deserialize, Serialize)]
struct Dpmm {
    managers: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct Dpm {
    name: Option<String>,
    update: String,
    upgrade: String,
    install: String,
    uninstall: String,
    packages: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct Generation {
    managers: Vec<Dpm>,
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
    /// List dpmm generations
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
        let n = extract_gen(f);
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
    let home = PathBuf::from(env::var("HOME").context("No HOME directory set")?);
    let config = if let Ok(p) = env::var("XDG_CONFIG_HOME") {
        PathBuf::from(p).join("dpmm")
    } else {
        home.join(".config/dpmm")
    };
    let dpmm_toml = fs::read_to_string(config.join("dpmm.toml"))?;
    let cache = if let Ok(p) = env::var("XDG_CACHE_HOME") {
        PathBuf::from(p).join("dpmm")
    } else {
        home.join(".cache/dpmm")
    };
    if dpmm_toml.is_empty() {
        eprintln!("Empty dpmm.toml\nterminating!");
        return Ok(());
    }
    if !cache.exists() {
        fs::create_dir(&cache)?;
    }
    let dpmm: Dpmm = toml::from_str(&dpmm_toml)?;
    let mut managers: Vec<Dpm> = vec![];
    for manager in dpmm.managers {
        let fname = format!("{manager}.toml");
        let mut toml: Dpm = toml::from_str(&fs::read_to_string(config.join(&fname))?)?;
        toml.name = Some(manager);
        managers.push(toml);
    }
    let latest_gen = get_gen_file(&cache, 0);
    let (latest_gen, n) = if let Some(f) = latest_gen {
        (toml::from_str(&fs::read_to_string(f.0)?)?, f.1)
    } else {
        let gen0 = cache.join("generation_0.toml");
        let mut managers0 = managers.clone();
        for manager in &mut managers0 {
            manager.packages.clear();
        }
        let managers0 = Generation {
            managers: managers0,
        };
        fs::write(&gen0, toml::to_string(&managers0)?.as_bytes())?;
        // assuming the above worked!
        (managers0, 0)
    };

    let current_gen = Generation { managers };

    let args = Args::parse();
    match &args.command {
        Commands::Switch => {
            let mut changed = false;
            for m in &current_gen.managers {
                let mname = m.name.clone().unwrap().clone();
                // ignore removed managers
                if let Some(corresp) = latest_gen
                    .managers
                    .iter()
                    .find(|manager| manager.name == Some(mname.clone()))
                {
                    let (added, removed) = diff_unique(&corresp.packages, &m.packages);
                    resolve_changes(&m.install, &m.uninstall, &added, &removed, args.dry_run)?;
                    changed = !removed.is_empty() || !added.is_empty();
                }
            }
            if changed && !args.dry_run {
                fs::write(
                    cache.join(format!("generation_{}.toml", n + 1)),
                    toml::to_string(&current_gen)?,
                )?;
            }
        }
        Commands::Rollback { generation } => {
            let new_gen_file: String = if let Some(generation) = generation {
                fs::read_to_string(cache.join(format!("{generation}.toml")))?
            } else {
                fs::read_to_string(
                    get_gen_file(&cache, 1)
                        .context("Failed to get last generation file")?
                        .0,
                )?
            };
            let new_gen: Generation = toml::from_str(&new_gen_file)?;
            for m in &new_gen.managers {
                let mname = m.name.clone().unwrap().clone();
                // ignore removed managers
                if let Some(corresp) = latest_gen
                    .managers
                    .iter()
                    .find(|manager| manager.name == Some(mname.clone()))
                {
                    let (added, removed) = diff_unique(&corresp.packages, &m.packages);
                    resolve_changes(&m.install, &m.uninstall, &added, &removed, args.dry_run)?;
                }
                fs::write(
                    config.join(format!("{mname}.toml")),
                    toml::to_string::<Dpm>(m)?.as_bytes(),
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
        Commands::Update => {
            if args.dry_run {
                for d in current_gen.managers {
                    println!("{}", d.update);
                }
            } else {
                for d in current_gen.managers {
                    let cmd_n_args: Vec<_> = d.update.split_whitespace().collect();
                    let mut d = Command::new(cmd_n_args[0]);
                    d.args(&cmd_n_args[1..]);
                    d.spawn()?.wait()?;
                }
            }
        }
        Commands::Upgrade => {
            if args.dry_run {
                for d in current_gen.managers {
                    println!("{}", d.upgrade);
                }
            } else {
                for d in current_gen.managers {
                    let cmd_n_args: Vec<_> = d.upgrade.split_whitespace().collect();
                    let mut d = Command::new(cmd_n_args[0]);
                    d.args(&cmd_n_args[1..]);
                    d.spawn()?.wait()?;
                }
            }
        }
    }
    Ok(())
}
