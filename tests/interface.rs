use cli_command_kit::Mode;
use cron_person_cli::{CancellationToken, cli, execute};
use std::{
    fs,
    io::{self, Write},
    path::Path,
    process::Command,
};
fn temp() -> tempfile::TempDir {
    fs::create_dir_all(".tmp").unwrap();
    tempfile::tempdir_in(".tmp").unwrap()
}
async fn invoke(store: &Path, args: &[&str], mode: Mode) -> (String, String) {
    let mut argv = vec![
        "cron-person".into(),
        "--store".into(),
        store.as_os_str().to_os_string(),
    ];
    argv.extend(args.iter().map(Into::into));
    let matches = cli::command(mode).try_get_matches_from(argv).unwrap();
    let mut out = Vec::new();
    let mut err = Vec::new();
    execute(
        &matches,
        mode,
        &mut out,
        &mut err,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    (
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
    )
}
#[tokio::test]
async fn every_directory_command_both_modes_and_changed_diagnostics() {
    let temp = temp();
    let dir = temp.path().canonicalize().unwrap();
    let store = dir.join("state.json");
    let path = dir.to_str().unwrap();
    fs::write(
        dir.join(".cronrc"),
        "VALUE=one\n@daily echo two\n@hourly printf 'a  b'\n",
    )
    .unwrap();
    for mode in [Mode::Human, Mode::Agent] {
        for args in [
            vec!["dir", "check", path],
            vec!["dir", "status", path],
            vec!["dir", "list"],
            vec!["task", "list"],
            vec!["status"],
        ] {
            let (out, _) = invoke(&store, &args, mode).await;
            if mode == Mode::Human || !matches!(args.as_slice(), ["dir", "list"] | ["task", "list"])
            {
                assert!(!out.is_empty());
            }
        }
        let (out, _) = invoke(&store, &["dir", "allow", path], mode).await;
        assert!(out.contains(if mode == Mode::Agent {
            "status: allowed"
        } else {
            "Authorized"
        }));
        for args in [
            vec!["dir", "status", path],
            vec!["dir", "list"],
            vec!["task", "list"],
            vec!["status"],
            vec!["crontab", "show"],
        ] {
            let (out, err) = invoke(&store, &args, mode).await;
            assert!(!out.is_empty());
            assert!(err.is_empty());
        }
        let (out, _) = invoke(&store, &["deny", path], mode).await;
        assert!(out.contains(if mode == Mode::Agent {
            "status: denied"
        } else {
            "Revoked"
        }));
        invoke(&store, &["allow", path], mode).await;
        fs::write(dir.join(".cronrc"), "@daily changed\n").unwrap();
        for args in [
            vec!["dir", "list"],
            vec!["status"],
            vec!["task", "list"],
            vec!["crontab", "show"],
        ] {
            let (out, err) = invoke(&store, &args, mode).await;
            if args == ["dir", "list"] {
                assert!(out.contains("changed"));
            } else {
                assert!(err.contains("authorization refused"));
            }
        }
        invoke(&store, &["dir", "deny", path], mode).await;
        fs::write(
            dir.join(".cronrc"),
            "VALUE=one\n@daily echo two\n@hourly printf 'a  b'\n",
        )
        .unwrap();
    }
    for args in [
        vec![],
        vec!["dir"],
        vec!["task"],
        vec!["crontab"],
        vec!["hook"],
    ] {
        let (out, _) = invoke(&store, &args, Mode::Agent).await;
        assert!(out.starts_with("ALERT:"));
    }
    for shell in ["bash", "zsh", "fish"] {
        let (out, _) = invoke(&store, &["hook", shell], Mode::Human).await;
        assert!(out.contains("hook export"));
    }
}
struct Broken;
impl Write for Broken {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed fixture"))
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
#[tokio::test]
async fn metadata_completion_output_errors_and_cancellation() {
    let temp = temp();
    let store = temp.path().join("state.json");
    for mode in [Mode::Agent, Mode::Human] {
        assert_eq!(invoke(&store, &["--version"], mode).await.0, "0.1.0\n");
        assert_eq!(invoke(&store, &["skill"], mode).await.0, cli::SKILL_POINTER);
        for shell in ["bash", "zsh", "fish", "powershell"] {
            for suffix in [
                vec!["completion", shell],
                vec!["completion", shell, "--no-descriptions"],
            ] {
                let out = invoke(&store, &suffix, mode).await.0;
                assert!(out.contains("cron-person"));
                let matches = cli::command(mode)
                    .try_get_matches_from(std::iter::once("cron-person").chain(suffix.into_iter()))
                    .unwrap();
                assert!(
                    execute(
                        &matches,
                        mode,
                        &mut Broken,
                        &mut Vec::new(),
                        &CancellationToken::new()
                    )
                    .await
                    .is_err()
                );
            }
        }
        for args in [vec!["skill"], vec!["--version"], vec!["dir"]] {
            let matches = cli::command(mode)
                .try_get_matches_from(std::iter::once("cron-person").chain(args.into_iter()))
                .unwrap();
            assert!(
                execute(
                    &matches,
                    mode,
                    &mut Broken,
                    &mut Vec::new(),
                    &CancellationToken::new()
                )
                .await
                .is_err()
            );
        }
    }
    let matches = cli::command(Mode::Agent)
        .try_get_matches_from(["cron-person", "status"])
        .unwrap();
    let cancel = CancellationToken::new();
    cancel.cancel();
    let error = execute(
        &matches,
        Mode::Agent,
        &mut Vec::new(),
        &mut Vec::new(),
        &cancel,
    )
    .await
    .unwrap_err();
    assert_eq!(cron_person_cli::exit_status(&error), 130);
    assert_eq!(
        cron_person_cli::exit_status(&cron_person_cli::Error::Committed(io::Error::other(
            "durability"
        ))),
        6
    );
}

#[tokio::test]
async fn syntax_checks_and_hook_generation_are_independent_of_corrupt_state() {
    let temp = temp();
    let dir = temp.path().canonicalize().unwrap();
    let store = dir.join("state.json");
    fs::write(&store, "malformed state").unwrap();
    fs::write(dir.join(".cronrc"), "@daily true\n").unwrap();
    for args in [
        vec!["hook", "bash"],
        vec!["dir", "check", dir.to_str().unwrap()],
    ] {
        let (out, _) = invoke(&store, &args, Mode::Agent).await;
        assert!(!out.is_empty());
    }
}
#[test]
fn native_entrypoint_reports_once_validates_offline_and_preserves_raw_metadata() {
    let temp = temp();
    let dir = temp.path().canonicalize().unwrap();
    let mut base = Command::new(env!("CARGO_BIN_EXE_cron-person"));
    base.env("HOME", &dir)
        .env("XDG_STATE_HOME", "relative-invalid")
        .env("AGENT", "1");
    for args in [
        vec!["skill", "extra"],
        vec!["exec", "only-dir"],
        vec!["hook", "invalid"],
        vec!["dir", "deny", "a", "b"],
        vec!["hook", "export", "invalid"],
    ] {
        let output = base.args(args).output().unwrap();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).starts_with("error:"));
        base = Command::new(env!("CARGO_BIN_EXE_cron-person"));
    }
    for agent in ["0", "1", "true", "yes"] {
        let output = Command::new(env!("CARGO_BIN_EXE_cron-person"))
            .env("AGENT", agent)
            .arg("--version")
            .output()
            .unwrap();
        assert_eq!(output.stdout, b"0.1.0\n");
        assert!(output.stderr.is_empty());
    }
    let output = Command::new(env!("CARGO_BIN_EXE_cron-person"))
        .env("AGENT", "1")
        .args(["--store"])
        .arg(dir.join("missing.json"))
        .args(["dir", "allow"])
        .arg(&dir)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let error = String::from_utf8(output.stderr).unwrap();
    assert_eq!(error.matches("ERR:").count(), 1);
    assert!(!error.contains("Usage:"));
    fs::write(dir.join(".cronrc"), "@daily true\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_cron-person"))
        .env("HOME", &dir)
        .env("XDG_STATE_HOME", "relative-invalid")
        .env("AGENT", "1")
        .current_dir(&dir)
        .args(["dir", "allow"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("dir: ~\n")
    );
    assert!(dir.join(".local/state/cron-person/allowed.json").exists());
    let output = Command::new(env!("CARGO_BIN_EXE_cron-person"))
        .env("AGENT", "0")
        .arg("--help")
        .output()
        .unwrap();
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("├─") && text.contains("╰─"));
}

#[cfg(unix)]
#[test]
fn native_sync_and_uninstall_only_touch_fixture_crontab() {
    use std::os::unix::fs::PermissionsExt;
    let temp = temp();
    let root = temp.path().canonicalize().unwrap();
    let directory = root.join("project");
    fs::create_dir(&directory).unwrap();
    fs::write(directory.join(".cronrc"), "@daily echo fixture\n").unwrap();
    let bin = root.join("bin");
    fs::create_dir(&bin).unwrap();
    let provider = bin.join("crontab");
    fs::write(&provider,"#!/bin/sh\ncase \"$1\" in -l) cat \"$FIXTURE_CRONTAB\";; -) cat > \"$FIXTURE_CRONTAB\";; esac\n").unwrap();
    fs::set_permissions(&provider, fs::Permissions::from_mode(0o700)).unwrap();
    let data = root.join("personal.crontab");
    let personal = "@weekly echo unrelated\n";
    fs::write(&data, personal).unwrap();
    let store = root.join("custom-state.json");
    let path = format!("{}:/usr/bin:/bin", bin.display());
    let run = |args: &[&str], agent: &str| {
        Command::new(env!("CARGO_BIN_EXE_cron-person"))
            .env("HOME", &root)
            .env("XDG_STATE_HOME", root.join("state"))
            .env("PATH", &path)
            .env("AGENT", agent)
            .env("FIXTURE_CRONTAB", &data)
            .arg("--store")
            .arg(&store)
            .args(args)
            .output()
            .unwrap()
    };
    assert!(
        run(&["dir", "allow", directory.to_str().unwrap()], "1")
            .status
            .success()
    );
    for mode in ["0", "1"] {
        let output = run(&["crontab", "sync"], mode);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let text = fs::read_to_string(&data).unwrap();
        assert!(text.starts_with(personal));
        assert!(text.contains("--store"));
        assert!(text.contains("exec"));
        let output = run(&["crontab", "uninstall"], mode);
        assert!(output.status.success());
        assert_eq!(fs::read_to_string(&data).unwrap(), personal);
    }
    let output = run(&["hook", "export", "bash"], "1");
    assert!(output.status.success());
}
