use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, Weak},
};

use tokio::sync::Mutex as AsyncMutex;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum SaveTarget {
    Path(PathBuf),
    UntitledWindow(String),
}

/// Serializes persistence work per physical target while allowing unrelated
/// files to save concurrently. Pathless untitled documents use their window as
/// a temporary identity until the naming pipeline chooses a real path.
#[derive(Default)]
pub struct SaveCoordinator {
    locks: Mutex<HashMap<SaveTarget, Weak<AsyncMutex<()>>>>,
}

impl SaveCoordinator {
    pub fn for_path(&self, path: &Path) -> Arc<AsyncMutex<()>> {
        self.lock_for(SaveTarget::Path(path.to_path_buf()))
    }

    pub fn for_untitled_window(&self, window_label: &str) -> Arc<AsyncMutex<()>> {
        self.lock_for(SaveTarget::UntitledWindow(window_label.to_string()))
    }

    fn lock_for(&self, target: SaveTarget) -> Arc<AsyncMutex<()>> {
        let mut locks = self
            .locks
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        locks.retain(|_, lock| lock.strong_count() > 0);

        if let Some(lock) = locks.get(&target).and_then(Weak::upgrade) {
            return lock;
        }

        let lock = Arc::new(AsyncMutex::new(()));
        locks.insert(target, Arc::downgrade(&lock));
        lock
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_target_reuses_lock_and_different_targets_do_not() {
        let coordinator = SaveCoordinator::default();
        let first = coordinator.for_path(Path::new("/tmp/one.txt"));
        let same = coordinator.for_path(Path::new("/tmp/one.txt"));
        let different = coordinator.for_path(Path::new("/tmp/two.txt"));

        assert!(Arc::ptr_eq(&first, &same));
        assert!(!Arc::ptr_eq(&first, &different));
    }

    #[test]
    fn untitled_windows_have_independent_locks() {
        let coordinator = SaveCoordinator::default();
        let first = coordinator.for_untitled_window("main");
        let same = coordinator.for_untitled_window("main");
        let other = coordinator.for_untitled_window("secondary");

        assert!(Arc::ptr_eq(&first, &same));
        assert!(!Arc::ptr_eq(&first, &other));
    }

    #[test]
    fn unused_locks_are_reclaimed() {
        let coordinator = SaveCoordinator::default();
        let first = coordinator.for_path(Path::new("/tmp/one.txt"));
        drop(first);

        let _replacement = coordinator.for_path(Path::new("/tmp/two.txt"));
        let locks = coordinator.locks.lock().unwrap();
        assert_eq!(locks.len(), 1);
        assert!(locks.contains_key(&SaveTarget::Path(PathBuf::from("/tmp/two.txt"))));
    }

    #[test]
    fn same_target_allows_only_one_active_writer() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        runtime.block_on(async {
            let coordinator = SaveCoordinator::default();
            let first = coordinator.for_path(Path::new("/tmp/shared.txt"));
            let second = coordinator.for_path(Path::new("/tmp/shared.txt"));
            let unrelated = coordinator.for_path(Path::new("/tmp/other.txt"));

            let _first_guard = first.lock().await;
            assert!(second.try_lock().is_err());
            assert!(unrelated.try_lock().is_ok());
        });
    }
}
