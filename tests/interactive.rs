#![cfg(unix)]
use cron_person_cli::store::Store;
use std::{
    fs,
    io::{Read, Write},
    os::{
        fd::{FromRawFd, OwnedFd},
        unix::process::CommandExt,
    },
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[test]
fn inherited_terminal_input_and_original_foreground_group_are_restored() {
    fs::create_dir_all(".tmp").unwrap();
    let temp = tempfile::tempdir_in(".tmp").unwrap();
    let dir = temp.path().canonicalize().unwrap();
    fs::write(dir.join(".cronrc"), "@daily true\n").unwrap();
    let store = Store::new(&dir.join("state.json")).unwrap();
    store.allow(&dir).unwrap();
    let quote = |s: &std::ffi::OsStr| String::from_utf8(shell_quote::Sh::quote_vec(s)).unwrap();
    let command = format!(
        "{} --store {} exec {} -- 'printf CHILD_READY; read line; printf \"CHILD_ACCEPTED:%s\\n\" \"$line\"'; printf OUTER_READY; read line; printf \"OUTER_ACCEPTED:%s\\n\" \"$line\"",
        quote(std::ffi::OsStr::new(env!("CARGO_BIN_EXE_cron-person"))),
        quote(store.path.as_os_str()),
        quote(dir.as_os_str())
    );
    let (mut master, mut slave) = (0, 0);
    assert_eq!(
        unsafe {
            libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        },
        0
    );
    let master = unsafe { OwnedFd::from_raw_fd(master) };
    let slave = unsafe { OwnedFd::from_raw_fd(slave) };
    let mut process = Command::new("/bin/sh");
    process
        .args(["-c", &command])
        .env("SHELL", "/bin/sh")
        .stdin(Stdio::from(slave.try_clone().unwrap()))
        .stdout(Stdio::from(slave.try_clone().unwrap()))
        .stderr(Stdio::from(slave));
    unsafe {
        process.pre_exec(|| {
            if libc::setsid() < 0 || libc::ioctl(0, libc::TIOCSCTTY, 0) < 0 {
                Err(std::io::Error::last_os_error())
            } else {
                Ok(())
            }
        });
    }
    let mut child = process.spawn().unwrap();
    let mut terminal = fs::File::from(master);
    use std::os::fd::AsRawFd;
    let mut output = Vec::new();
    let mut sent_child = false;
    let mut sent_outer = false;
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        let mut poll = libc::pollfd {
            fd: terminal.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        if unsafe { libc::poll(&mut poll, 1, 20) } > 0 {
            let mut bytes = [0; 4096];
            match terminal.read(&mut bytes) {
                Ok(0) => break,
                Ok(n) => output.extend_from_slice(&bytes[..n]),
                Err(e) if e.raw_os_error() == Some(libc::EIO) => break,
                Err(e) => panic!("PTY read: {e}"),
            };
        }
        let text = String::from_utf8_lossy(&output);
        if text.contains("CHILD_READY") && !sent_child {
            terminal.write_all(b"hello\n").unwrap();
            sent_child = true;
        }
        if text.contains("OUTER_READY") && !sent_outer {
            terminal.write_all(b"world\n").unwrap();
            sent_outer = true;
        }
        if text.contains("OUTER_ACCEPTED:world") {
            break;
        }
    }
    // Closing the controlling terminal hangs up any stalled test processes, including separate groups.
    drop(terminal);
    let end = Instant::now() + Duration::from_secs(2);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() > end {
            child.kill().unwrap();
            break child.wait().unwrap();
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let text = String::from_utf8_lossy(&output);
    assert!(text.contains("CHILD_ACCEPTED:hello"), "{text}");
    assert!(text.contains("OUTER_ACCEPTED:world"), "{text}");
    assert!(status.success(), "{status}: {text}");
}
