//! Bounded, opt-in diagnostic logs. Never call this writer from an audio callback.
use std::{
    error::Error,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard},
    time::{SystemTime, UNIX_EPOCH},
};
use tracing_subscriber::{EnvFilter, fmt::MakeWriter, fmt::writer::MakeWriterExt};

const MAX_FILES: usize = 4;
const MAX_BYTES: u64 = 2 * 1024 * 1024;
const PREFIX: &str = "musicterrain-v1-";

/// Initialize one subscriber. `None` writes only to stderr and creates no files.
/// Explicit file output keeps at most four module-owned logs of 2 MiB each.
/// Session IDs use wall time only for diagnostics, never for generation seeds.
pub fn init(log_dir: Option<&Path>) -> Result<(), Box<dyn Error>> {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("musicterrain=info"));
    let session = session_id()?;
    if let Some(dir) = log_dir {
        let writer = SharedWriter(Arc::new(Mutex::new(RotatingLog::open(
            dir,
            session.clone(),
            MAX_BYTES,
            MAX_FILES,
        )?)));
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_ansi(false)
            .with_target(false)
            .with_writer(io::stderr.and(writer))
            .try_init()
            .map_err(|_| io::Error::other("diagnostics subscriber already initialized"))?;
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_ansi(false)
            .with_target(false)
            .with_writer(io::stderr)
            .try_init()
            .map_err(|_| io::Error::other("diagnostics subscriber already initialized"))?;
    }
    tracing::info!(session_id = %session, file_logging = log_dir.is_some(), "session started");
    Ok(())
}

fn session_id() -> io::Result<String> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| io::Error::other("system clock is earlier than the Unix epoch"))?
        .as_nanos();
    Ok(format!("{nanos:032x}-{:08x}", std::process::id()))
}

struct RotatingLog {
    directory: PathBuf,
    session: String,
    next: u32,
    file: File,
    path: PathBuf,
    bytes: u64,
    max_bytes: u64,
    max_files: usize,
}

impl RotatingLog {
    fn open(dir: &Path, session: String, max_bytes: u64, max_files: usize) -> io::Result<Self> {
        if max_bytes == 0 || max_files == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid log limits",
            ));
        }
        fs::create_dir_all(dir)?;
        let mut next = 0;
        let (file, path) = Self::create_file(dir, &session, &mut next)?;
        let log = Self {
            directory: dir.to_owned(),
            session,
            next,
            file,
            path,
            bytes: 0,
            max_bytes,
            max_files,
        };
        log.retain()?;
        Ok(log)
    }

    fn create_file(dir: &Path, session: &str, next: &mut u32) -> io::Result<(File, PathBuf)> {
        // create_new also rejects existing symlinks. Never truncate a collision.
        for _ in 0..1024 {
            let index = *next;
            *next = next
                .checked_add(1)
                .ok_or_else(|| io::Error::other("log index exhausted"))?;
            let path = dir.join(format!("{PREFIX}{session}-{index:08x}.log"));
            match OpenOptions::new().create_new(true).write(true).open(&path) {
                Ok(file) => return Ok((file, path)),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "too many log name collisions",
        ))
    }

    fn retain(&self) -> io::Result<()> {
        let mut owned = Vec::new();
        for item in fs::read_dir(&self.directory)? {
            let item = item?;
            // Do not follow or remove symlinks, directories or unrelated log files.
            if !item.file_type()?.is_file() {
                continue;
            }
            if item.file_name().to_str().is_some_and(owned_name) {
                owned.push(item.path());
            }
        }
        owned.sort();
        let excess = owned.len().saturating_sub(self.max_files);
        let mut removed = 0;
        for path in owned {
            if removed >= excess {
                break;
            }
            if path == self.path {
                continue;
            }
            match fs::remove_file(path) {
                Ok(()) => removed += 1,
                Err(error) if error.kind() == io::ErrorKind::NotFound => removed += 1,
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }

    fn rotate(&mut self) -> io::Result<()> {
        self.file.flush()?;
        let (file, path) = Self::create_file(&self.directory, &self.session, &mut self.next)?;
        self.file = file;
        self.path = path;
        self.bytes = 0;
        self.retain()
    }
}

fn owned_name(name: &str) -> bool {
    let Some(body) = name
        .strip_prefix(PREFIX)
        .and_then(|s| s.strip_suffix(".log"))
    else {
        return false;
    };
    let fields: Vec<_> = body.split('-').collect();
    fields.len() == 3
        && fields.iter().zip([32, 8, 8]).all(|(field, length)| {
            field.len() == length && field.bytes().all(|byte| byte.is_ascii_hexdigit())
        })
}

impl Write for RotatingLog {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let mut consumed = 0;
        while consumed < buffer.len() {
            if self.bytes >= self.max_bytes {
                self.rotate()?;
            }
            let count =
                (self.max_bytes - self.bytes).min((buffer.len() - consumed) as u64) as usize;
            self.file.write_all(&buffer[consumed..consumed + count])?;
            self.bytes += count as u64;
            consumed += count;
        }
        Ok(consumed)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

#[derive(Clone)]
struct SharedWriter(Arc<Mutex<RotatingLog>>);

struct LockedWriter<'a>(MutexGuard<'a, RotatingLog>);

impl<'a> MakeWriter<'a> for SharedWriter {
    type Writer = LockedWriter<'a>;

    fn make_writer(&'a self) -> Self::Writer {
        LockedWriter(
            self.0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        )
    }
}

impl Write for LockedWriter<'_> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.0.write(buffer)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct TestDirectory(PathBuf);
    impl TestDirectory {
        fn new() -> Self {
            static INDEX: AtomicUsize = AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "musicterrain-log-test-{}-{}",
                session_id().expect("test clock"),
                INDEX.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).expect("create test directory");
            Self(path)
        }

        fn logs(&self) -> Vec<PathBuf> {
            fs::read_dir(&self.0)
                .expect("read logs")
                .map(|item| item.expect("entry").path())
                .filter(|path| {
                    path.file_name()
                        .and_then(|s| s.to_str())
                        .is_some_and(owned_name)
                })
                .collect()
        }
    }
    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn oversized_records_and_repeated_sessions_stay_bounded() {
        let dir = TestDirectory::new();
        for _ in 0..3 {
            let mut writer = RotatingLog::open(&dir.0, session_id().unwrap(), 32, 4).unwrap();
            writer.write_all(&[b'x'; 250]).unwrap();
            writer.flush().unwrap();
            let logs = dir.logs();
            assert_eq!(logs.len(), 4);
            assert!(
                logs.iter()
                    .all(|path| fs::metadata(path).unwrap().len() <= 32)
            );
        }
    }

    #[test]
    fn collisions_and_unrelated_files_are_never_truncated() {
        let dir = TestDirectory::new();
        let session = session_id().unwrap();
        let collision = dir.0.join(format!("{PREFIX}{session}-00000000.log"));
        fs::write(&collision, b"existing").unwrap();
        fs::write(dir.0.join("latest.log"), b"user log").unwrap();
        fs::write(dir.0.join("musicterrain-v1-not-a-session.log"), b"user log").unwrap();
        let mut writer = RotatingLog::open(&dir.0, session, 32, 4).unwrap();
        writer.write_all(b"new session").unwrap();
        assert_eq!(fs::read(collision).unwrap(), b"existing");
        writer.write_all(&[b'y'; 250]).unwrap();
        assert_eq!(fs::read(dir.0.join("latest.log")).unwrap(), b"user log");
        assert_eq!(
            fs::read(dir.0.join("musicterrain-v1-not-a-session.log")).unwrap(),
            b"user log"
        );
    }

    #[test]
    fn concurrent_events_are_complete() {
        let dir = TestDirectory::new();
        let writer = SharedWriter(Arc::new(Mutex::new(
            RotatingLog::open(&dir.0, session_id().unwrap(), 4096, 4).unwrap(),
        )));
        let threads: Vec<_> = (0..8)
            .map(|n| {
                let writer = writer.clone();
                std::thread::spawn(move || {
                    for _ in 0..20 {
                        writeln!(writer.make_writer(), "event-{n}").unwrap();
                    }
                })
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }
        let content = fs::read_to_string(&dir.logs()[0]).unwrap();
        assert_eq!(content.lines().count(), 160);
        assert!(
            content
                .lines()
                .all(|line| line.len() == 7 && line.starts_with("event-"))
        );
    }

    #[test]
    fn tracing_events_reach_the_file_writer() {
        let dir = TestDirectory::new();
        let writer = SharedWriter(Arc::new(Mutex::new(
            RotatingLog::open(&dir.0, session_id().unwrap(), 4096, 4).unwrap(),
        )));
        let subscriber = tracing_subscriber::fmt()
            .with_ansi(false)
            .with_target(false)
            .with_writer(writer)
            .finish();
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(revision = 42u64, queued_jobs = 1, "mesh completed");
        });
        let content = fs::read_to_string(&dir.logs()[0]).unwrap();
        assert!(content.contains("mesh completed"));
        assert!(content.contains("revision=42"));
        assert!(content.contains("queued_jobs=1"));
        assert!(!content.contains(&dir.0.to_string_lossy().to_string()));
    }

    #[test]
    fn invalid_limits_and_file_destinations_return_errors() {
        let dir = TestDirectory::new();
        assert!(RotatingLog::open(&dir.0, session_id().unwrap(), 0, 4).is_err());
        let file = dir.0.join("regular-file");
        fs::write(&file, b"preserve").unwrap();
        assert!(RotatingLog::open(&file, session_id().unwrap(), 32, 4).is_err());
        assert_eq!(fs::read(file).unwrap(), b"preserve");
    }
}
