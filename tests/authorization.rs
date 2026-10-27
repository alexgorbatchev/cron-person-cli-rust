use cron_person_cli::{
    cronrc, crontab,
    store::{Status, Store},
};
use std::fs;

#[test]
fn approved_bytes_commands_and_fresh_transactions() {
    fs::create_dir_all(".tmp").unwrap();
    let temp = tempfile::tempdir_in(".tmp").unwrap();
    let a = temp.path().join("a");
    let b = temp.path().join("b");
    fs::create_dir(&a).unwrap();
    fs::create_dir(&b).unwrap();
    let bytes = b"VALUE = 'two  spaces'\n0 1 * JAN 7 printf 'x  y' | cat > result\n";
    fs::write(a.join(".cronrc"), bytes).unwrap();
    fs::write(b.join(".cronrc"), "@reboot true\n").unwrap();
    let file = cronrc::parse(bytes, &a.join(".cronrc")).unwrap();
    assert_eq!(file.env["VALUE"], "two  spaces");
    assert_eq!(file.tasks[0].command, "printf 'x  y' | cat > result");
    assert_ne!(
        file.hash,
        cronrc::parse(&bytes[..bytes.len() - 1], &a.join(".cronrc"))
            .unwrap()
            .hash
    );
    let path = temp.path().join("state/allowed.json");
    let one = Store::new(&path).unwrap();
    let stale = Store::new(&path).unwrap();
    assert_eq!(one.status(&a).unwrap().0, Status::Unauthorized);
    one.allow(&a).unwrap();
    assert_eq!(stale.approved(&a).unwrap().env["VALUE"], "two  spaces");
    one.deny(&a).unwrap();
    stale.allow(&b).unwrap();
    assert!(stale.approved(&a).is_err());
    assert_eq!(one.list().unwrap().len(), 1);
    one.allow(&a).unwrap();
    fs::write(a.join(".cronrc"), "* * * * * touch forbidden\n").unwrap();
    assert_eq!(one.status(&a).unwrap().0, Status::Changed);
    assert!(one.approved(&a).is_err());
    fs::remove_file(a.join(".cronrc")).unwrap();
    assert_eq!(one.status(&a).unwrap().0, Status::Orphan);
}

#[test]
fn managed_markers_preserve_personal_bytes() {
    let personal = format!(
        "# personal text {} suffix\n@daily echo mine\n",
        crontab::START
    );
    let block = format!("{}\n@daily test\n{}\n", crontab::START, crontab::END);
    let merged = crontab::merge(&personal, Some(&block)).unwrap();
    assert!(merged.starts_with(&personal));
    assert_eq!(crontab::merge(&merged, Some(&block)).unwrap(), merged);
    assert_eq!(crontab::merge(&merged, None).unwrap(), personal);
    assert!(crontab::merge(&format!("{}\n", crontab::START), None).is_err());
}
