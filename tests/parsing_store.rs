use cron_person_cli::{
    cronrc,
    store::{Status, Store, default_path},
};
use std::{fs, path::Path};
fn temp() -> tempfile::TempDir {
    fs::create_dir_all(".tmp").unwrap();
    tempfile::tempdir_in(".tmp").unwrap()
}
#[test]
fn native_cron_names_ranges_descriptors_and_validation() {
    let dir = temp();
    let path = dir.path().join(".cronrc");
    let input=b"# heading\r\n\nVALUE=first\nVALUE = \" last  value \"\n0-59/3 0,12 * JUL WED printf 'a  b'\n@hourly true\n@daily true\n@midnight true\n@weekly true\n@monthly true\n@yearly true\n@annually true\n@reboot true\n";
    let file = cronrc::parse(input, &path).unwrap();
    assert_eq!(file.tasks.len(), 9);
    assert_eq!(file.tasks[0].line_number, 5);
    assert_eq!(file.env["VALUE"], " last  value ");
    assert_eq!(file.tasks[0].command, "printf 'a  b'");
    for text in [
        "bad",
        "* * * * *",
        "* * * * *   ",
        "@unknown true",
        "@every 1m true",
        "* 99 * * * true",
        "0 0 L * * true",
        "0 0 * * MON#2 true",
        "\0",
        "=bad",
        "@daily",
        "* * * * * \0",
    ] {
        assert!(cronrc::parse(text.as_bytes(), &path).is_err(), "{text}");
    }
    assert!(cronrc::parse(&[0xff], &path).is_err());
    fs::write(&path, input).unwrap();
    assert_eq!(cronrc::parse_file(&path).unwrap().hash, file.hash);
    fs::create_dir_all(dir.path().join("a/b")).unwrap();
    assert_eq!(
        cronrc::discover(&dir.path().join("a/b")).unwrap().unwrap(),
        cronrc::absolute(&path).unwrap()
    );
    let absolute = cronrc::absolute(&dir.path().join("a/../a/b")).unwrap();
    assert!(absolute.ends_with("a/b"));
    assert!(cronrc::parse_file(dir.path()).is_err());
    assert!(
        default_path(Some(Path::new("relative")), Some(Path::new("/home/test")))
            .unwrap()
            .starts_with("/home/test/.local/state")
    );
    assert!(
        default_path(Some(Path::new("/state")), None)
            .unwrap()
            .starts_with("/state")
    );
    assert!(default_path(None, None).is_err());
}
#[test]
fn concurrent_locked_transactions_and_failed_persist_do_not_resurrect_records() {
    let temp = temp();
    let state = temp.path().join("state/allowed.json");
    let store = Store::new(&state).unwrap();
    let mut threads = Vec::new();
    for index in 0..16 {
        let dir = temp.path().join(index.to_string());
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join(".cronrc"), "@daily true\n").unwrap();
        let store = store.clone();
        threads.push(std::thread::spawn(move || store.allow(&dir).unwrap()));
    }
    for thread in threads {
        thread.join().unwrap();
    }
    assert_eq!(store.list().unwrap().len(), 16);
    let dir = temp.path().join("0");
    store.deny(&dir).unwrap();
    assert_eq!(store.status(&dir).unwrap().0, Status::Unauthorized);
    let a = temp.path().join("1");
    fs::remove_file(a.join(".cronrc")).unwrap();
    assert_eq!(store.prune().unwrap(), vec![cronrc::absolute(&a).unwrap()]);
    assert!(store.prune().unwrap().is_empty());
    fs::remove_file(&state).unwrap();
    fs::create_dir(&state).unwrap();
    assert!(matches!(
        store.allow(&dir),
        Err(cron_person_cli::Error::Io(_))
    ));
    fs::remove_dir(&state).unwrap();
    assert!(store.list().unwrap().is_empty());
    fs::write(&state, b"bad json").unwrap();
    assert!(Store::new(&state).is_err());
    fs::write(&state,b"[{\"dir\":\"relative\",\"path\":\"wrong\",\"hash\":\"bad\",\"allowed_at\":\"2026-01-01T00:00:00Z\"}]").unwrap();
    assert!(Store::new(&state).is_err());
    fs::remove_file(&state).unwrap();
    assert_eq!(
        store.status(&temp.path().join("absent")).unwrap().0,
        Status::MissingCronrc
    );
    fs::create_dir(temp.path().join(".cronrc")).unwrap();
    assert_eq!(store.status(temp.path()).unwrap().0, Status::MissingCronrc);
}
#[test]
fn malformed_markers_are_rejected_and_cron_arguments_are_opaque() {
    use cron_person_cli::crontab::{self, END, START};
    for text in [
        format!("{END}\n"),
        format!("{START}\n{START}\n{END}\n"),
        format!("{START}\n{END}\n{START}\n{END}\n"),
    ] {
        assert!(crontab::merge(&text, None).is_err());
    }
    assert_eq!(
        crontab::merge("no trailing newline", Some("block\n")).unwrap(),
        "no trailing newline\nblock\n"
    );
    for text in ["a\nb", "a\rb", "a\0b"] {
        assert!(crontab::quote(text).is_err());
    }
    let quoted = crontab::quote("").unwrap();
    let output = std::process::Command::new("/bin/sh")
        .args(["-c", &format!("printf '<%s>' {quoted}")])
        .output()
        .unwrap();
    assert_eq!(output.stdout, b"<>");
}
