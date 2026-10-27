use crate::{Error, Result, cronrc};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Allowed,
    Changed,
    Unauthorized,
    MissingCronrc,
    Orphan,
}
impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Allowed => "allowed",
            Self::Changed => "changed",
            Self::Unauthorized => "unauthorized",
            Self::MissingCronrc => "missing_cronrc",
            Self::Orphan => "orphan",
        })
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    pub dir: PathBuf,
    pub path: PathBuf,
    pub hash: String,
    pub allowed_at: DateTime<Utc>,
}
#[derive(Debug, Clone)]
pub struct Store {
    pub path: PathBuf,
}
pub fn default_path(state: Option<&Path>, home: Option<&Path>) -> Result<PathBuf> {
    let base = match state.filter(|p| p.is_absolute()) {
        Some(p) => p.to_path_buf(),
        None => home
            .ok_or_else(|| {
                Error::Invalid(
                    "home directory is unavailable; set an absolute XDG_STATE_HOME or --store"
                        .into(),
                )
            })?
            .join(".local/state"),
    };
    Ok(base.join("cron-person/allowed.json"))
}
impl Store {
    pub fn new(path: &Path) -> Result<Self> {
        let store = Self {
            path: cronrc::absolute(path)?,
        };
        store.load()?;
        Ok(store)
    }
    fn load(&self) -> Result<BTreeMap<PathBuf, Record>> {
        let bytes = match fs::read(&self.path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
            Err(e) => return Err(e.into()),
        };
        let records: Vec<Record> = serde_json::from_slice(&bytes)?;
        let mut map = BTreeMap::new();
        for record in records {
            if !record.dir.is_absolute()
                || record.path != record.dir.join(".cronrc")
                || record.hash.len() != 64
                || !record.hash.bytes().all(|b| b.is_ascii_hexdigit())
            {
                return Err(Error::Invalid(
                    "authorization store contains an invalid record".into(),
                ));
            }
            map.insert(record.dir.clone(), record);
        }
        Ok(map)
    }
    pub fn lock(&self) -> Result<File> {
        fs::create_dir_all(self.path.parent().expect("absolute parent"))?;
        let mut name = self.path.as_os_str().to_os_string();
        name.push(".lock");
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(PathBuf::from(name))?;
        file.lock()?;
        Ok(file)
    }
    fn persist(&self, records: &BTreeMap<PathBuf, Record>) -> Result<()> {
        let parent = self.path.parent().expect("absolute parent");
        let mut temp = tempfile::NamedTempFile::new_in(parent)?;
        serde_json::to_writer_pretty(&mut temp, &records.values().collect::<Vec<_>>())?;
        temp.write_all(b"\n")?;
        temp.as_file().sync_all()?;
        temp.persist(&self.path).map_err(|e| e.error)?;
        #[cfg(unix)]
        File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(Error::Committed)?;
        Ok(())
    }
    pub fn list(&self) -> Result<Vec<Record>> {
        Ok(self.load()?.into_values().collect())
    }
    pub fn allow(&self, dir: &Path) -> Result<Record> {
        let _lock = self.lock()?;
        let mut records = self.load()?;
        let file = cronrc::parse_file(&cronrc::absolute(dir)?.join(".cronrc"))?;
        let record = Record {
            dir: file.dir,
            path: file.path,
            hash: file.hash,
            allowed_at: Utc::now(),
        };
        records.insert(record.dir.clone(), record.clone());
        self.persist(&records)?;
        Ok(record)
    }
    pub fn deny(&self, dir: &Path) -> Result<()> {
        let _lock = self.lock()?;
        let mut records = self.load()?;
        records.remove(&cronrc::absolute(dir)?);
        self.persist(&records)
    }
    pub fn status(&self, dir: &Path) -> Result<(Status, Option<Record>)> {
        let dir = cronrc::absolute(dir)?;
        let record = self.load()?.remove(&dir);
        let path = dir.join(".cronrc");
        match path.metadata() {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok((
                    if record.is_some() {
                        Status::Orphan
                    } else {
                        Status::MissingCronrc
                    },
                    record,
                ));
            }
            Err(e) => return Err(e.into()),
            Ok(info) if !info.is_file() => return Ok((Status::MissingCronrc, record)),
            Ok(_) => {}
        }
        let digest = cronrc::hash(&cronrc::read(&path)?);
        Ok((
            match &record {
                None => Status::Unauthorized,
                Some(r) if r.hash == digest => Status::Allowed,
                Some(_) => Status::Changed,
            },
            record,
        ))
    }
    pub fn approved(&self, dir: &Path) -> Result<cronrc::File> {
        let _lock = self.lock()?;
        let dir = cronrc::absolute(dir)?;
        let record = self
            .load()?
            .remove(&dir)
            .ok_or_else(|| Error::Authorization {
                dir: dir.display().to_string(),
                status: Status::Unauthorized,
            })?;
        let bytes = cronrc::read(&dir.join(".cronrc"))?;
        if record.hash != cronrc::hash(&bytes) {
            return Err(Error::Authorization {
                dir: dir.display().to_string(),
                status: Status::Changed,
            });
        }
        cronrc::parse(&bytes, &record.path)
    }
    pub fn prune(&self) -> Result<Vec<PathBuf>> {
        let _lock = self.lock()?;
        let mut records = self.load()?;
        let mut removed = Vec::new();
        records.retain(|dir, _| {
            if dir.join(".cronrc").is_file() {
                true
            } else {
                removed.push(dir.clone());
                false
            }
        });
        if !removed.is_empty() {
            self.persist(&records)?;
        }
        Ok(removed)
    }
}
