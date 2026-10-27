#![cfg(unix)]
use cron_person_cli::{
    CancellationToken, crontab, hook, runner,
    store::{Status, Store},
};
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

fn fixture() -> (tempfile::TempDir, Store, PathBuf) {
    fs::create_dir_all(".tmp").unwrap();
    let temp = tempfile::tempdir_in(".tmp").unwrap();
    let dir = cron_person_cli::cronrc::absolute(&temp.path().join("dir space ' quote %")).unwrap();
    fs::create_dir(&dir).unwrap();
    fs::write(dir.join(".cronrc"), "VALUE='two  spaces'\n@daily true\n").unwrap();
    let store = Store::new(&temp.path().join("state custom/allowed.json")).unwrap();
    (temp, store, dir)
}
fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_cron-person")
}
fn call(store: &Store, args: &[&str]) -> std::process::Output {
    Command::new(binary())
        .arg("--store")
        .arg(&store.path)
        .args(args)
        .env("AGENT", "1")
        .env("SHELL", "/bin/sh")
        .output()
        .unwrap()
}
// Cronie's documented command-side percent processing, before /bin/sh.
fn cron_command(input: &str) -> String {
    let mut out = String::new();
    let mut escaped = false;
    for ch in input.chars() {
        if escaped {
            if ch == '%' {
                out.pop();
            }
            out.push(ch);
            escaped = false;
        } else if ch == '\\' {
            out.push(ch);
            escaped = true;
        } else if ch == '%' {
            panic!("unescaped cron percent");
        } else {
            out.push(ch);
        }
    }
    out
}
#[test]
fn actual_generated_entries_preserve_opaque_commands_and_revoke_every_shell_effect() {
    let (_temp, store, dir) = fixture();
    let command = "printf '%s' 'x  y\\%$HOME' | cat > result; printf '%s' \"$VALUE\" > env-result";
    fs::write(
        dir.join(".cronrc"),
        format!("VALUE='two  spaces'\n@daily {command}\n"),
    )
    .unwrap();
    store.allow(&dir).unwrap();
    let (block, count) = crontab::generate(&store, Path::new(binary()), &mut Vec::new()).unwrap();
    assert_eq!(count, 1);
    let line = block.lines().find(|l| l.starts_with("@daily ")).unwrap();
    let shell = cron_command(line.strip_prefix("@daily ").unwrap());
    let output = Command::new("/bin/sh")
        .args(["-c", &shell])
        .env("SHELL", "/bin/sh")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read(dir.join("result")).unwrap(), b"x  y\\%$HOME");
    assert_eq!(fs::read(dir.join("env-result")).unwrap(), b"two  spaces");
    store.deny(&dir).unwrap();
    fs::remove_file(dir.join("result")).unwrap();
    fs::remove_file(dir.join("env-result")).unwrap();
    let output = Command::new("/bin/sh")
        .args(["-c", &shell])
        .env("AGENT", "0")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(!dir.join("result").exists());
    assert!(!dir.join("env-result").exists());
    assert_eq!(
        String::from_utf8_lossy(&output.stderr)
            .matches("ERR:")
            .count(),
        0
    ); // human execution reports once
    assert_eq!(
        String::from_utf8_lossy(&output.stderr)
            .matches("[ERROR]")
            .count(),
        1
    );
}
#[tokio::test]
async fn runner_actual_environment_child_failure_and_cancelled_descendants() {
    let (_temp, store, dir) = fixture();
    store.allow(&dir).unwrap();
    let output = call(
        &store,
        &[
            "exec",
            dir.to_str().unwrap(),
            "--",
            "printf '%s|%s|%s' \"$VALUE\" \"$CRON_PERSON_DIR\" \"$CRON_PERSON_CRONRC\"",
        ],
    );
    assert!(output.status.success());
    let value = String::from_utf8(output.stdout).unwrap();
    assert!(value.starts_with("two  spaces|"));
    assert!(value.contains(dir.to_str().unwrap()));
    let output = call(&store, &["task", "run", dir.to_str().unwrap(), "exit 7"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("exit status: 7"));
    let token = CancellationToken::new();
    token.cancel();
    assert!(matches!(
        runner::run(&store, &dir, "true", &BTreeMap::new(), &token).await,
        Err(cron_person_cli::Error::Cancelled)
    ));
    assert!(
        runner::run(
            &store,
            &dir,
            "  ",
            &BTreeMap::new(),
            &CancellationToken::new()
        )
        .await
        .is_err()
    );
    let token = CancellationToken::new();
    let stop = token.clone();
    let marker = dir.join("child.pid");
    let ready = marker.clone();
    let canceller = tokio::spawn(async move {
        for _ in 0..200 {
            if ready.exists() {
                stop.cancel();
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("child not ready")
    });
    let result = runner::run(
        &store,
        &dir,
        "sleep 20 & echo $! > child.pid; wait",
        &BTreeMap::new(),
        &token,
    )
    .await;
    canceller.await.unwrap();
    assert!(matches!(result, Err(cron_person_cli::Error::Cancelled)));
    let pid: i32 = fs::read_to_string(marker).unwrap().trim().parse().unwrap();
    // A killed orphan may await init reaping; it must never remain running.
    let stat = fs::read_to_string(format!("/proc/{pid}/stat"));
    if let Ok(stat) = stat {
        assert_eq!(stat.split(") ").nth(1).unwrap().chars().next(), Some('Z'));
    }
    fs::write(dir.join(".cronrc"), "SHELL=/does/not/exist\n@daily true\n").unwrap();
    store.allow(&dir).unwrap();
    assert!(matches!(
        runner::run(
            &store,
            &dir,
            "true",
            &BTreeMap::new(),
            &CancellationToken::new()
        )
        .await,
        Err(cron_person_cli::Error::Io(_))
    ));
}
fn provider(temp: &Path) -> crontab::System {
    let program = temp.join("provider");
    fs::write(&program,"#!/bin/sh\ncd -- \"$(dirname -- \"$0\")\" || exit 9\nif [ -f failure ]; then echo denied >&2; exit 3; fi\ncase \"$1\" in\n-l) if [ -f data ]; then cat data; else echo 'no crontab for fixture' >&2; exit 1; fi;;\n-) cat > data;;\nesac\n").unwrap();
    fs::set_permissions(&program, fs::Permissions::from_mode(0o700)).unwrap();
    crontab::System {
        program: "/bin/sh".into(),
        arguments: vec![program.into_os_string()],
    }
}
#[tokio::test]
async fn native_crontab_provider_installs_uninstalls_and_surfaces_errors() {
    fs::create_dir_all(".tmp").unwrap();
    let temp = tempfile::tempdir_in(".tmp").unwrap();
    let system = provider(temp.path());
    let token = CancellationToken::new();
    assert!(system.read(&token).await.unwrap().is_empty());
    let personal = "MAILTO=user@example.org\n@weekly echo personal\n";
    fs::write(temp.path().join("data"), personal).unwrap();
    let block = format!("{}\n@daily true\n{}\n", crontab::START, crontab::END);
    system.update(Some(&block), &token).await.unwrap();
    assert_eq!(
        system.read(&token).await.unwrap(),
        format!("{personal}{block}")
    );
    system.update(Some(&block), &token).await.unwrap();
    system.update(None, &token).await.unwrap();
    assert_eq!(system.read(&token).await.unwrap(), personal);
    system.update(None, &token).await.unwrap();
    fs::write(temp.path().join("failure"), "").unwrap();
    assert!(matches!(
        system.read(&token).await,
        Err(cron_person_cli::Error::Provider {
            operation: "read",
            ..
        })
    ));
    let token = CancellationToken::new();
    token.cancel();
    assert!(matches!(
        system.read(&token).await,
        Err(cron_person_cli::Error::Cancelled)
    ));
}

#[tokio::test]
async fn provider_conflict_write_failure_invalid_text_and_cancellation_are_real() {
    use cron_person_cli::Error;
    fs::create_dir_all(".tmp").unwrap();
    let temp = tempfile::tempdir_in(".tmp").unwrap();
    let script = temp.path().canonicalize().unwrap().join("fixture.sh");
    fs::write(&script,"case \"$1\" in -l) printf personal;; -) cat >/dev/null; echo rejected >&2; exit 8;; esac\n").unwrap();
    let system = crontab::System {
        program: "/bin/sh".into(),
        arguments: vec![script.clone().into_os_string()],
    };
    let token = CancellationToken::new();
    assert!(matches!(
        system.update(Some("block\n"), &token).await,
        Err(Error::Provider {
            operation: "write",
            ..
        })
    ));
    let count = script.with_file_name("count");
    let quoted = String::from_utf8(shell_quote::Sh::quote_vec(count.as_os_str())).unwrap();
    fs::write(
        &script,
        format!("if [ -f {quoted} ]; then echo changed; else touch {quoted}; echo personal; fi\n"),
    )
    .unwrap();
    assert!(matches!(
        system.update(Some("block\n"), &token).await,
        Err(Error::Conflict)
    ));
    fs::write(&script, "printf '\\377'\n").unwrap();
    assert!(matches!(system.read(&token).await, Err(Error::Invalid(_))));
    fs::write(&script, "sleep 20 & echo ready > \"$0.ready\"; wait\n").unwrap();
    let stop = token.clone();
    let ready = script.with_extension("sh.ready");
    let canceller = tokio::spawn(async move {
        for _ in 0..200 {
            if ready.exists() {
                stop.cancel();
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("provider not ready")
    });
    assert!(matches!(system.read(&token).await, Err(Error::Cancelled)));
    canceller.await.unwrap();
}
#[test]
fn hooks_execute_warnings_preserve_status_traps_and_register_once() {
    let (_temp, store, dir) = fixture();
    let bash = hook::generate("bash", Path::new(binary()), &store.path).unwrap();
    for initial in [
        "PROMPT_COMMAND=(not_cron_person_hook)",
        "PROMPT_COMMAND='echo previous'",
    ] {
        let output = Command::new("/bin/bash")
            .args([
                "-c",
                &format!("{initial}; {bash}\n{bash}\ndeclare -p PROMPT_COMMAND"),
            ])
            .env("PATH", "/nonexistent")
            .output()
            .unwrap();
        assert!(output.status.success());
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(text.contains("_cron_person_hook"));
        if initial.contains("not_") {
            assert!(text.contains("[0]=\"_cron_person_hook\""), "{text}");
            assert!(text.contains("not_cron_person_hook"));
        } else {
            assert_eq!(text.matches("_cron_person_hook").count(), 1);
            assert!(text.contains("echo previous"));
        }
    }
    for shell in ["bash", "zsh"] {
        let script = hook::generate(shell, Path::new(binary()), &store.path).unwrap();
        let registration = if shell == "bash" {
            "PROMPT_COMMAND=(existing_prompt);"
        } else {
            "precmd_functions=(existing_prompt); chpwd_functions=(existing_prompt);"
        };
        let examine = if shell == "bash" {
            "declare -p PROMPT_COMMAND"
        } else {
            "typeset -p precmd_functions chpwd_functions"
        };
        let test = format!(
            "{registration}\ntrap 'echo retained' INT\n{script}\n{script}\nfalse; _cron_person_hook; result=$?; kill -INT $$; {examine}; exit $result"
        );
        let output = Command::new(shell)
            .args(["-c", &test])
            .current_dir(&dir)
            .env("AGENT", "1")
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(1),
            "{shell}:{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stderr).contains("WARN: unauthorized"));
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(text.contains("retained"));
        assert!(text.contains("existing_prompt"));
        let occurrences = text.matches("_cron_person_hook").count();
        assert_eq!(occurrences, if shell == "bash" { 1 } else { 2 }, "{text}");
    }
    store.allow(&dir).unwrap();
    let mut warnings = Vec::new();
    hook::export(&store, &dir, false, &mut warnings).unwrap();
    assert!(warnings.is_empty());
    fs::write(dir.join(".cronrc"), "@daily false\n").unwrap();
    hook::export(&store, &dir, false, &mut warnings).unwrap();
    assert!(String::from_utf8(warnings).unwrap().contains("modified"));
    assert!(hook::generate("unknown", Path::new(binary()), &store.path).is_err());
    assert_eq!(store.status(&dir).unwrap().0, Status::Changed);
}
