use crate::{CancellationToken, Error, Result, store::Store};
use std::{ffi::OsString, io::Write, path::Path, process::Stdio};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
pub const START: &str = "# >>> cron-person managed block (DO NOT EDIT DIRECTLY) >>>";
pub const END: &str = "# <<< cron-person managed block <<<";

/// Quote one opaque argument, protecting every percent from cron's pre-shell parser.
pub fn quote(argument: &str) -> Result<String> {
    if argument.contains(['\n', '\r', '\0']) {
        return Err(Error::Invalid(
            "cron arguments cannot contain newline, carriage return or NUL".into(),
        ));
    }
    let mut out = Vec::new();
    for (i, part) in argument.split('%').enumerate() {
        if i > 0 {
            out.extend(b"'\\%'");
        }
        out.extend(shell_quote::Sh::quote_vec(part));
    }
    Ok(String::from_utf8(out).expect("UTF-8 input quoting"))
}
fn text(path: &Path) -> Result<&str> {
    path.to_str()
        .ok_or_else(|| Error::Invalid("crontab and JSON store paths must be UTF-8".into()))
}
pub fn generate(
    store: &Store,
    executable: &Path,
    diagnostics: &mut dyn Write,
) -> Result<(String, usize)> {
    let mut block = format!("{START}\n");
    let mut count = 0;
    for record in store.list()? {
        let file = match store.approved(&record.dir) {
            Ok(f) => f,
            Err(e) => {
                writeln!(
                    diagnostics,
                    "WARN: {}",
                    cli_command_kit::display_message(&e.to_string())
                )?;
                continue;
            }
        };
        count += 1;
        for task in &file.tasks {
            block.push_str(&format!(
                "{} {} --store {} exec {} -- {}\n",
                task.schedule,
                quote(text(executable)?)?,
                quote(text(&store.path)?)?,
                quote(text(&file.dir)?)?,
                quote(&task.command)?
            ));
        }
    }
    block.push_str(END);
    block.push('\n');
    Ok((block, count))
}
pub fn merge(existing: &str, block: Option<&str>) -> Result<String> {
    let mut outside = String::new();
    let mut inside = false;
    let mut seen = false;
    for line in existing.split_inclusive('\n') {
        let marker = line.trim_end_matches(['\n', '\r']);
        if marker == START {
            if inside || seen {
                return Err(Error::Invalid(
                    "duplicate or nested cron-person managed block".into(),
                ));
            }
            inside = true;
            seen = true;
        } else if marker == END {
            if !inside {
                return Err(Error::Invalid("unpaired cron-person end marker".into()));
            }
            inside = false;
        } else if !inside {
            outside.push_str(line);
        }
    }
    if inside {
        return Err(Error::Invalid(
            "unterminated cron-person managed block".into(),
        ));
    }
    if let Some(block) = block {
        if !outside.is_empty() && !outside.ends_with('\n') {
            outside.push('\n');
        }
        outside.push_str(block);
    }
    Ok(outside)
}
#[derive(Debug, Clone)]
pub struct System {
    pub program: OsString,
    pub arguments: Vec<OsString>,
}
impl Default for System {
    fn default() -> Self {
        Self {
            program: "crontab".into(),
            arguments: Vec::new(),
        }
    }
}
impl System {
    async fn command(
        &self,
        args: &[&str],
        input: Option<&str>,
        cancel: &CancellationToken,
    ) -> Result<std::process::Output> {
        if cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let mut command = tokio::process::Command::new(&self.program);
        command
            .args(&self.arguments)
            .args(args)
            .env("LC_ALL", "C")
            .stdin(if input.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        #[cfg(unix)]
        command.process_group(0);
        let mut child = command.spawn()?;
        let pid = child.id();
        let stdin = child.stdin.take();
        let mut stdout = child.stdout.take().expect("piped stdout");
        let mut stderr = child.stderr.take().expect("piped stderr");
        let writer = async {
            if let (Some(mut pipe), Some(text)) = (stdin, input) {
                pipe.write_all(text.as_bytes()).await?;
            }
            Ok::<_, std::io::Error>(())
        };
        let operation = async {
            let mut out = Vec::new();
            let mut err = Vec::new();
            let (write, read_out, read_err, status) = tokio::join!(
                writer,
                stdout.read_to_end(&mut out),
                stderr.read_to_end(&mut err),
                child.wait()
            );
            write?;
            read_out?;
            read_err?;
            Ok::<_, Error>(std::process::Output {
                status: status?,
                stdout: out,
                stderr: err,
            })
        };
        tokio::select! {r=operation=>r,_=cancel.cancelled()=>{
            #[cfg(unix)] if let Some(pid)=pid {unsafe{libc::kill(-(pid as i32),libc::SIGKILL);}}
            let _=child.kill().await;let _=child.wait().await;Err(Error::Cancelled)
        }}
    }
    pub async fn read(&self, cancel: &CancellationToken) -> Result<String> {
        let output = self.command(&["-l"], None, cancel).await?;
        if !output.status.success() {
            let message = String::from_utf8_lossy(&output.stderr);
            if output.status.code() == Some(1) && message.trim().starts_with("no crontab for ") {
                return Ok(String::new());
            }
            return Err(Error::Provider {
                operation: "read",
                message: message.trim().into(),
            });
        }
        String::from_utf8(output.stdout)
            .map_err(|_| Error::Invalid("system crontab must contain UTF-8 text".into()))
    }
    pub async fn update(&self, block: Option<&str>, cancel: &CancellationToken) -> Result<()> {
        let current = self.read(cancel).await?;
        let updated = merge(&current, block)?;
        if updated == current {
            return Ok(());
        }
        if self.read(cancel).await? != current {
            return Err(Error::Conflict);
        }
        let output = self.command(&["-"], Some(&updated), cancel).await?;
        if !output.status.success() {
            return Err(Error::Provider {
                operation: "write",
                message: String::from_utf8_lossy(&output.stderr).trim().into(),
            });
        }
        Ok(())
    }
}
