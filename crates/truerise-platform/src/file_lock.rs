//! Cross-platform exclusive file lock (LockFileEx on Windows, flock on POSIX).
//!
//! Spec (§6.7 ledger row): ledger appends hold a file lock while reading the
//! tail - this module is the only platform boundary providing it. The lock is
//! OS-level, held by an RAII guard, and released on drop.

use std::fs::{File, OpenOptions};
use std::io;
use std::path::Path;

/// An exclusive lock over a file, held for the lifetime of the guard.
///
/// The guard owns the file handle: writes to the locked file go THROUGH the
/// guard ([`FileLock::write_all`]) - a second handle on the same file would
/// be blocked by the lock itself (ERROR_LOCK_VIOLATION).
#[derive(Debug)]
pub struct FileLock {
    file: File,
}

impl FileLock {
    /// Append-write through the locked handle (the ledger's append path).
    pub fn write_all(&mut self, bytes: &[u8]) -> io::Result<()> {
        use std::io::{Seek, SeekFrom, Write};
        self.file.seek(SeekFrom::End(0))?;
        self.file.write_all(bytes)?;
        self.file.flush()
    }

    /// Read the whole file through the locked handle.
    pub fn read_all(&mut self) -> io::Result<Vec<u8>> {
        use std::io::{Read, Seek, SeekFrom};
        let mut buf = Vec::new();
        self.file.seek(SeekFrom::Start(0))?;
        self.file.read_to_end(&mut buf)?;
        Ok(buf)
    }
}

/// Acquire an exclusive, blocking lock on ``path`` (creating it if missing).
pub fn lock_exclusive(path: &Path) -> io::Result<FileLock> {
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(path)?;
    lock_fd(&file)?;
    Ok(FileLock { file })
}

#[cfg(windows)]
mod win {
    #[repr(C)]
    pub struct Overlapped {
        pub internal: usize,
        pub internal_high: usize,
        pub offset: u32,
        pub offset_high: u32,
        pub event: *mut core::ffi::c_void,
    }

    unsafe extern "system" {
        pub fn LockFileEx(
            file: *mut core::ffi::c_void,
            flags: u32,
            reserved: u32,
            low: u32,
            high: u32,
            overlapped: *mut Overlapped,
        ) -> i32;
        pub fn UnlockFileEx(
            file: *mut core::ffi::c_void,
            reserved: u32,
            low: u32,
            high: u32,
            overlapped: *mut Overlapped,
        ) -> i32;
    }

    pub const LOCKFILE_EXCLUSIVE_LOCK: u32 = 0x2;
    pub const ONE_BYTE: u32 = 1;
}

#[cfg(windows)]
fn lock_fd(file: &File) -> io::Result<()> {
    use std::os::windows::io::AsRawHandle;
    use win::*;
    let handle = file.as_raw_handle();
    let mut overlapped = Overlapped {
        internal: 0,
        internal_high: 0,
        offset: 0,
        offset_high: 0,
        event: std::ptr::null_mut(),
    };
    // Exclusive + blocking (no FAIL_IMMEDIATELY flag) over byte 0.
    // SAFETY: `handle` is a valid open file handle borrowed from `file`; the
    // OVERLAPPED is zero-initialised, stack-allocated and outlives the call.
    // LockFileEx only writes kernel status into the OVERLAPPED on failure.
    let rc = unsafe {
        LockFileEx(
            handle,
            LOCKFILE_EXCLUSIVE_LOCK,
            0,
            ONE_BYTE,
            0,
            &mut overlapped,
        )
    };
    if rc == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(windows)]
impl FileLock {
    fn unlock(&mut self) {
        use std::os::windows::io::AsRawHandle;
        use win::*;
        let handle = self.file.as_raw_handle();
        let mut overlapped = Overlapped {
            internal: 0,
            internal_high: 0,
            offset: 0,
            offset_high: 0,
            event: std::ptr::null_mut(),
        };
        // SAFETY: same handle/overlapped invariants as `lock_fd`; unlocking a
        // range this guard exclusively holds.
        unsafe {
            UnlockFileEx(handle, 0, ONE_BYTE, 0, &mut overlapped);
        }
    }
}

#[cfg(unix)]
fn lock_fd(file: &File) -> io::Result<()> {
    use std::os::unix::io::AsRawFd;
    unsafe extern "C" {
        fn flock(fd: i32, operation: i32) -> i32;
    }
    const LOCK_EX: i32 = 2;
    // SAFETY: `flock` on a valid open fd; no memory is touched, only the
    // advisory lock state of that descriptor.
    let rc = unsafe { flock(file.as_raw_fd(), LOCK_EX) };
    if rc != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(unix)]
impl FileLock {
    fn unlock(&mut self) {
        use std::os::unix::io::AsRawFd;
        unsafe extern "C" {
            fn flock(fd: i32, operation: i32) -> i32;
        }
        const LOCK_UN: i32 = 8;
        // SAFETY: valid open fd owned by this guard; unlock its own lock.
        unsafe {
            flock(self.file.as_raw_fd(), LOCK_UN);
        }
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        self.unlock();
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn lock_write_via_guard_roundtrip() {
        let dir = std::env::temp_dir().join("truerise_platform_lock_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("lock_target.txt");
        {
            let mut lock = lock_exclusive(&path).expect("acquire lock");
            lock.write_all(
                b"row1
",
            )
            .expect("write through guard");
            lock.write_all(
                b"row2
",
            )
            .expect("append through guard");
            assert_eq!(
                lock.read_all().unwrap(),
                b"row1
row2
"
            );
        }
        {
            let mut lock = lock_exclusive(&path).expect("re-acquire after drop");
            assert_eq!(
                lock.read_all().unwrap(),
                b"row1
row2
"
            );
            lock.write_all(
                b"row3
",
            )
            .expect("append after re-acquire");
            assert_eq!(
                lock.read_all().unwrap(),
                b"row1
row2
row3
"
            );
        }
        let _ = std::fs::remove_file(&path);
    }
}
