use crate::{Error, Result};
use croner::parser::{CronParser, Seconds, Year};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::OpenOptions,
    io::Read,
    path::{Component, Path, PathBuf},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub line_number: usize,
    pub schedule: String,
    pub command: String,
    pub raw: String,
}
#[derive(Debug, Clone)]
pub struct File {
    pub path: PathBuf,
    pub dir: PathBuf,
    pub hash: String,
    pub env: BTreeMap<String, String>,
    pub tasks: Vec<Task>,
}
pub fn absolute(path: &Path) -> Result<PathBuf> {
    let mut clean = PathBuf::new();
    for component in std::path::absolute(path)?.components() {
        match component {
            Component::ParentDir => {
                clean.pop();
            }
            Component::CurDir => {}
            other => clean.push(other.as_os_str()),
        }
    }
    Ok(clean)
}
pub fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub fn read(path: &Path) -> Result<Vec<u8>> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    let mut file = options.open(path)?;
    if !file.metadata()?.is_file() {
        return Err(Error::Invalid(format!(
            "{} must be a regular file",
            path.display()
        )));
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(bytes)
}
pub fn parse_file(path: &Path) -> Result<File> {
    parse(&read(path)?, path)
}
pub fn parse(bytes: &[u8], path: &Path) -> Result<File> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| Error::Invalid(".cronrc must contain UTF-8 text".into()))?;
    if text.contains('\0') {
        return Err(Error::Invalid(".cronrc contains a NUL byte".into()));
    }
    let path = absolute(path)?;
    let mut file = File {
        dir: path.parent().expect("absolute file parent").into(),
        path,
        hash: hash(bytes),
        env: BTreeMap::new(),
        tasks: Vec::new(),
    };
    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            if !key.is_empty()
                && key.bytes().enumerate().all(|(i, ch)| {
                    ch == b'_' || ch.is_ascii_alphabetic() || (i > 0 && ch.is_ascii_digit())
                })
            {
                let value = value.trim();
                let value = if value.len() >= 2
                    && ((value.starts_with('"') && value.ends_with('"'))
                        || (value.starts_with('\'') && value.ends_with('\'')))
                {
                    &value[1..value.len() - 1]
                } else {
                    value
                };
                file.env.insert(key.into(), value.into());
                continue;
            }
        }
        let task = task(line, index + 1, raw)
            .map_err(|e| Error::Invalid(format!("line {}: {e}", index + 1)))?;
        file.tasks.push(task);
    }
    Ok(file)
}
fn task(line: &str, line_number: usize, raw: &str) -> Result<Task> {
    let count = if line.starts_with('@') { 1 } else { 5 };
    let mut rest = line;
    let mut fields = Vec::new();
    for _ in 0..count {
        let end = rest
            .find(char::is_whitespace)
            .ok_or_else(|| Error::Invalid("expected schedule followed by a command".into()))?;
        fields.push(&rest[..end]);
        rest = rest[end..].trim_start();
    }
    if rest.is_empty() {
        return Err(Error::Invalid("missing command".into()));
    }
    let schedule = fields.join(" ");
    if count == 1 {
        if !matches!(
            schedule.as_str(),
            "@hourly"
                | "@daily"
                | "@midnight"
                | "@weekly"
                | "@monthly"
                | "@yearly"
                | "@annually"
                | "@reboot"
        ) {
            return Err(Error::Invalid(format!(
                "unsupported system-cron descriptor {schedule}; use a five-field schedule"
            )));
        }
    } else {
        let invalid_names = fields.iter().enumerate().any(|(index, field)| {
            field
                .split(|c: char| !c.is_ascii_alphabetic())
                .filter(|s| !s.is_empty())
                .any(|name| {
                    let name = name.to_ascii_uppercase();
                    match index {
                        3 => !matches!(
                            name.as_str(),
                            "JAN"
                                | "FEB"
                                | "MAR"
                                | "APR"
                                | "MAY"
                                | "JUN"
                                | "JUL"
                                | "AUG"
                                | "SEP"
                                | "OCT"
                                | "NOV"
                                | "DEC"
                        ),
                        4 => !matches!(
                            name.as_str(),
                            "SUN" | "MON" | "TUE" | "WED" | "THU" | "FRI" | "SAT"
                        ),
                        _ => true,
                    }
                })
        });
        if !schedule
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b" *,-/".contains(&b))
            || invalid_names
        {
            return Err(Error::Invalid(
                "unsupported system-cron schedule extension".into(),
            ));
        }
        CronParser::builder()
            .seconds(Seconds::Disallowed)
            .year(Year::Disallowed)
            .build()
            .parse(&schedule)
            .map_err(|e| Error::Invalid(format!("invalid schedule {schedule}: {e}")))?;
    }
    Ok(Task {
        line_number,
        schedule,
        command: rest.into(),
        raw: raw.into(),
    })
}
pub fn discover(start: &Path) -> Result<Option<PathBuf>> {
    let mut current = absolute(start)?;
    loop {
        let candidate = current.join(".cronrc");
        match candidate.metadata() {
            Ok(info) if info.is_file() => return Ok(Some(candidate)),
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        if !current.pop() {
            return Ok(None);
        }
    }
}
