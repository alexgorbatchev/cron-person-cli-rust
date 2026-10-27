use crate::{CancellationToken, Error, Result, store::Store};
use std::{collections::BTreeMap, ffi::OsString, path::Path, process::Stdio};

pub async fn run(
    store: &Store,
    dir: &Path,
    command: &str,
    extra: &BTreeMap<OsString, OsString>,
    cancel: &CancellationToken,
) -> Result<()> {
    if command.trim().is_empty() || command.contains('\0') {
        return Err(Error::Invalid(
            "command must be nonempty and contain no NUL".into(),
        ));
    }
    if cancel.is_cancelled() {
        return Err(Error::Cancelled);
    }
    // This single owned snapshot supplies both the approval digest and execution environment.
    let file = store.approved(dir)?;
    let shell = file
        .env
        .get("SHELL")
        .filter(|s| !s.is_empty())
        .map(OsString::from)
        .or_else(|| std::env::var_os("SHELL").filter(|s| !s.is_empty()))
        .unwrap_or_else(|| "/bin/sh".into());
    let mut process = tokio::process::Command::new(shell);
    process
        .arg("-c")
        .arg(command)
        .current_dir(&file.dir)
        .envs(&file.env)
        .envs(extra)
        .env("CRON_PERSON_DIR", &file.dir)
        .env("CRON_PERSON_CRONRC", &file.path)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);
    #[cfg(unix)]
    process.process_group(0);
    #[cfg(unix)]
    let mut foreground = Foreground::capture();
    let mut child = process.spawn()?;
    let pid = child.id();
    #[cfg(unix)]
    if let Some(pid) = pid
        && let Err(error) = foreground.handoff(pid as libc::pid_t)
    {
        unsafe {
            libc::kill(-(pid as libc::pid_t), libc::SIGKILL);
        }
        let _ = child.kill().await;
        let _ = child.wait().await;
        return Err(error.into());
    }
    let status = tokio::select! {
        status=child.wait()=>status?,
        _=cancel.cancelled()=>{
            #[cfg(unix)] if let Some(pid)=pid {
                // The child owns a new process group; signal all shell descendants before reaping.
                unsafe {libc::kill(-(pid as i32),libc::SIGKILL);}
            }
            let _=child.kill().await;let _=child.wait().await;
            #[cfg(unix)] foreground.restore()?;
            return Err(Error::Cancelled);
        }
    };
    #[cfg(unix)]
    foreground.restore()?;
    if status.success() {
        Ok(())
    } else {
        Err(Error::Child(status))
    }
}

#[cfg(unix)]
struct Foreground(Option<libc::pid_t>);
#[cfg(unix)]
impl Foreground {
    fn capture() -> Self {
        // A background invocation must never steal the caller's terminal.
        let group = unsafe { libc::getpgrp() };
        Self((unsafe { libc::tcgetpgrp(0) } == group).then_some(group))
    }
    fn handoff(&self, group: libc::pid_t) -> std::io::Result<()> {
        if self.0.is_some() {
            set_foreground(group)?;
            // The shell may already have attempted a read and received SIGTTIN.
            unsafe {
                libc::kill(-group, libc::SIGCONT);
            }
        }
        Ok(())
    }
    fn restore(&mut self) -> std::io::Result<()> {
        if let Some(group) = self.0 {
            set_foreground(group)?;
            self.0 = None;
        }
        Ok(())
    }
}
#[cfg(unix)]
impl Drop for Foreground {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}
#[cfg(unix)]
fn set_foreground(group: libc::pid_t) -> std::io::Result<()> {
    // POSIX sends SIGTTOU to a background caller of tcsetpgrp. Block it only
    // on this thread during the synchronous syscall, before crossing any await.
    unsafe {
        let mut block: libc::sigset_t = std::mem::zeroed();
        let mut previous: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&mut block);
        libc::sigaddset(&mut block, libc::SIGTTOU);
        let error = libc::pthread_sigmask(libc::SIG_BLOCK, &block, &mut previous);
        if error != 0 {
            return Err(std::io::Error::from_raw_os_error(error));
        }
        let result = if libc::tcsetpgrp(0, group) == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        };
        let error = libc::pthread_sigmask(libc::SIG_SETMASK, &previous, std::ptr::null_mut());
        result?;
        if error != 0 {
            return Err(std::io::Error::from_raw_os_error(error));
        }
        Ok(())
    }
}
