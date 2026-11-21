use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug)]
pub struct Directory(PathBuf);

impl Directory {
    #[must_use]
    pub fn new() -> Self {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let moment = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.subsec_nanos());
        let name = format!(
            "post-test-{}-{}-{moment}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let path = std::env::temp_dir().join(name);
        std::fs::create_dir_all(&path).expect("the system has a temporary directory");
        Self(path)
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.0
    }

    #[must_use]
    pub fn join(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Default for Directory {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::Directory;

    #[test]
    fn makes_an_empty_directory_and_removes_it_at_the_end() {
        let directory = Directory::default();
        let path = directory.path().to_owned();
        assert!(path.is_dir());
        assert_eq!(std::fs::read_dir(&path).expect("a directory").count(), 0);

        let file = directory.join("note.txt");
        std::fs::write(&file, "text").expect("a file");
        assert_eq!(file.parent(), Some(path.as_path()));

        drop(directory);
        assert!(!path.exists());
    }

    #[test]
    fn makes_a_different_directory_each_time() {
        let first = Directory::new();
        let second = Directory::new();

        assert_ne!(first.path(), second.path());
    }
}
