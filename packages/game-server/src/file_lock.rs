use std::{
    fs::{File, TryLockError},
    io,
};

pub(crate) struct FileLock(File);

impl FileLock {
    pub(crate) fn try_lock(file: File) -> Result<Self, TryLockError> {
        file.try_lock()?;
        Ok(Self(file))
    }

    pub(crate) fn lock(file: File) -> io::Result<Self> {
        file.lock()?;
        Ok(Self(file))
    }

    pub(crate) fn file(&self) -> &File {
        &self.0
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        // flock belongs to the open file description. A forked child shares it
        // until exec closes its CLOEXEC descriptor. LOCK_UN releases it even
        // while that descriptor is open: https://man7.org/linux/man-pages/man2/flock.2.html
        let _ = self.0.unlock();
    }
}
