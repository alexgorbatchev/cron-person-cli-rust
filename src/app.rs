use crate::{
    CancellationToken, Error, Result, cli, cronrc, crontab, hook, runner,
    store::{Status, Store, default_path},
};
use cli_command_kit::{Mode, clap::ArgMatches};
use std::{
    collections::BTreeMap,
    io::Write,
    path::{Path, PathBuf},
};
pub async fn execute(
    matches: &ArgMatches,
    mode: Mode,
    mut stdout: &mut dyn Write,
    stderr: &mut dyn Write,
    cancel: &CancellationToken,
) -> Result<()> {
    if matches.get_flag("version") {
        stdout.write_all(concat!(env!("CARGO_PKG_VERSION"), "\n").as_bytes())?;
        return Ok(());
    }
    if cli_command_kit::write_skill(matches, &cli::help_options(), &mut stdout)? {
        return Ok(());
    }
    let mut leaf = matches;
    let mut path = Vec::new();
    while let Some((name, args)) = leaf.subcommand() {
        path.push(name);
        leaf = args;
    }
    if path.first() == Some(&"completion") && path.len() == 2 {
        return crate::completion::write(path[1], leaf.get_flag("no-descriptions"), stdout);
    }
    let mut tree = cli::command(mode);
    let mut selected = &mut tree;
    for name in &path {
        selected = selected.find_subcommand_mut(name).expect("parsed command");
    }
    if path.is_empty()
        || (selected.has_subcommands() && path != ["hook"])
        || (path == ["hook"] && leaf.get_one::<String>("shell").is_none())
    {
        selected.write_help(&mut stdout)?;
        writeln!(stdout)?;
        return Ok(());
    }
    if cancel.is_cancelled() {
        return Err(Error::Cancelled);
    }
    let configured = matches.get_one::<PathBuf>("store").expect("store default");
    let state = std::env::var_os("XDG_STATE_HOME").map(PathBuf::from);
    let store_path = if configured.as_os_str().is_empty() {
        default_path(state.as_deref(), dirs::home_dir().as_deref())?
    } else {
        configured.clone()
    };
    let store = Store {
        path: cronrc::absolute(&store_path)?,
    };
    let agent = mode == Mode::Agent;
    if path == ["hook", "export"] {
        return hook::export(&store, &std::env::current_dir()?, agent, stderr);
    }
    if path == ["hook"] {
        stdout.write_all(
            hook::generate(
                leaf.get_one::<String>("shell").expect("shell"),
                &std::env::current_exe()?,
                &store.path,
            )?
            .as_bytes(),
        )?;
        return Ok(());
    }
    if path == ["exec"] || path == ["task", "run"] {
        let dir = leaf.get_one::<PathBuf>("dir").expect("dir");
        let command = leaf
            .get_many::<String>("command")
            .expect("command")
            .cloned()
            .collect::<Vec<_>>()
            .join(" ");
        return runner::run(&store, dir, &command, &BTreeMap::new(), cancel).await;
    }
    let mut out = Output { agent, out: stdout };
    if path.first() == Some(&"crontab") {
        // Both writers use the same per-user lock, regardless of --store.
        let global = Store {
            path: default_path(state.as_deref(), dirs::home_dir().as_deref())?
                .with_file_name("crontab-sync"),
        };
        let _lock = if path[1] != "show" {
            Some(global.lock()?)
        } else {
            None
        };
        if path[1] == "uninstall" {
            crontab::System::default().update(None, cancel).await?;
            return out.status(
                "crontab",
                "uninstalled",
                "Removed cron-person managed block",
            );
        }
        let (block, count) = crontab::generate(&store, &std::env::current_exe()?, stderr)?;
        if path[1] == "show" {
            out.out.write_all(block.as_bytes())?;
            return Ok(());
        }
        crontab::System::default()
            .update(Some(&block), cancel)
            .await?;
        if agent {
            out.fields(&[
                ("synced_directories", count.to_string()),
                ("status", "success".into()),
            ])
        } else {
            writeln!(
                out.out,
                "[OK] Synchronized {count} authorized directory task(s)"
            )
            .map_err(Into::into)
        }
    } else if path == ["status"] || path == ["task", "list"] || path == ["dir", "list"] {
        listing(&store, &path, &mut out, stderr)
    } else {
        let dir = leaf
            .get_one::<PathBuf>("path")
            .cloned()
            .unwrap_or(std::env::current_dir()?);
        let dir = cronrc::absolute(&dir)?;
        match *path.last().expect("domain command") {
            "allow" => {
                let rec = store.allow(&dir)?;
                if agent {
                    out.fields(&[
                        ("dir", display(&dir)),
                        ("status", "allowed".into()),
                        ("hash", rec.hash),
                    ])
                } else {
                    writeln!(
                        out.out,
                        "[OK] Authorized .cronrc in {}\n  Hash: {}",
                        display(&dir),
                        rec.hash
                    )
                    .map_err(Into::into)
                }
            }
            "deny" => {
                store.deny(&dir)?;
                if agent {
                    out.fields(&[("dir", display(&dir)), ("status", "denied".into())])
                } else {
                    writeln!(out.out, "[OK] Revoked authorization for {}", display(&dir))
                        .map_err(Into::into)
                }
            }
            "check" => {
                let file = cronrc::parse_file(&dir.join(".cronrc"))?;
                if agent {
                    out.fields(&[
                        ("path", display(&file.path)),
                        ("tasks_count", file.tasks.len().to_string()),
                        ("hash", file.hash),
                    ])
                } else {
                    writeln!(
                        out.out,
                        "[OK] Syntax valid: {}\n  Found {} task(s), {} environment variable(s)",
                        display(&file.path),
                        file.tasks.len(),
                        file.env.len()
                    )?;
                    for (index, task) in file.tasks.iter().enumerate() {
                        writeln!(
                            out.out,
                            "  #{} (line {}) {} => {}",
                            index + 1,
                            task.line_number,
                            task.schedule,
                            cli_command_kit::display_message(&task.command)
                        )?;
                    }
                    Ok(())
                }
            }
            "status" => {
                let (status, record) = store.status(&dir)?;
                if agent {
                    out.fields(&[("dir", display(&dir)), ("status", status.to_string())])?;
                    if let Some(r) = record {
                        out.fields(&[("hash", r.hash)])?;
                    }
                } else {
                    writeln!(out.out, "Directory: {}\nStatus: {status}", display(&dir))?;
                    if let Some(r) = record {
                        writeln!(
                            out.out,
                            "Hash: {}\nAllowed: {}",
                            r.hash,
                            r.allowed_at.to_rfc3339()
                        )?;
                    }
                    if let Ok(f) = cronrc::parse_file(&dir.join(".cronrc")) {
                        writeln!(out.out, "Tasks: {}", f.tasks.len())?;
                    }
                }
                Ok(())
            }
            _ => unreachable!("parsed directory action"),
        }
    }
}
fn display(path: &Path) -> String {
    cli_command_kit::display_path(path)
}
struct Output<'a> {
    agent: bool,
    out: &'a mut dyn Write,
}
impl Output<'_> {
    fn fields(&mut self, fields: &[(&str, String)]) -> Result<()> {
        for (key, value) in fields {
            writeln!(self.out, "{key}: {value}")?;
        }
        Ok(())
    }
    fn status(&mut self, key: &str, value: &str, human: &str) -> Result<()> {
        if self.agent {
            self.fields(&[(key, value.into())])
        } else {
            writeln!(self.out, "[OK] {human}").map_err(Into::into)
        }
    }
}
fn listing(
    store: &Store,
    path: &[&str],
    out: &mut Output<'_>,
    stderr: &mut dyn Write,
) -> Result<()> {
    let records = store.list()?;
    let mut allowed = 0;
    let mut tasks = Vec::new();
    for record in &records {
        let (status, _) = store.status(&record.dir)?;
        if path == ["dir", "list"] {
            if out.agent {
                writeln!(
                    out.out,
                    "dir: {}\tstatus: {status}\thash: {}",
                    display(&record.dir),
                    record.hash
                )?;
            } else {
                writeln!(
                    out.out,
                    "{} {} ({status})\n  Hash: {}",
                    if status == Status::Allowed {
                        "[OK]"
                    } else {
                        "[WARN]"
                    },
                    display(&record.dir),
                    record.hash
                )?;
            }
        }
        match store.approved(&record.dir) {
            Ok(file) => {
                allowed += 1;
                for task in file.tasks {
                    tasks.push((record.dir.clone(), task));
                }
            }
            Err(error) if path != ["dir", "list"] => writeln!(
                stderr,
                "WARN: {}",
                cli_command_kit::display_message(&error.to_string())
            )?,
            Err(_) => {}
        }
    }
    if path == ["dir", "list"] {
        if records.is_empty() && !out.agent {
            writeln!(out.out, "No tracked directories.")?;
        }
        return Ok(());
    }
    if path == ["status"] {
        if out.agent {
            out.fields(&[
                ("version", env!("CARGO_PKG_VERSION").into()),
                ("tracked_dirs", records.len().to_string()),
                ("allowed_dirs", allowed.to_string()),
                ("total_tasks", tasks.len().to_string()),
            ])?;
        } else {
            writeln!(
                out.out,
                "[OK] cron-person system status: active\n  Version: {}\n  Tracked Directories: {}\n  Allowed Directories: {allowed}\n  Active Tasks: {}",
                env!("CARGO_PKG_VERSION"),
                records.len(),
                tasks.len()
            )?;
        }
    } else {
        if !out.agent {
            writeln!(
                out.out,
                "{}",
                if tasks.is_empty() {
                    "No active tasks.".into()
                } else {
                    format!("Total tasks: {}", tasks.len())
                }
            )?;
        }
        for (dir, task) in tasks {
            let command = cli_command_kit::display_message(&task.command);
            if out.agent {
                writeln!(
                    out.out,
                    "dir: {}\tschedule: {}\tline: {}\tcmd: {command}",
                    display(&dir),
                    task.schedule,
                    task.line_number
                )?;
            } else {
                writeln!(
                    out.out,
                    "Directory: {}\n  [{}] (line {}) {command}",
                    display(&dir),
                    task.schedule,
                    task.line_number
                )?;
            }
        }
    }
    Ok(())
}
