use anyhow::{Context, Result};
use fs2::FileExt;
use rumux_core::rpc::send_rpc;
use rumux_core::runtime::instance_lock_path;
use std::fs::{File, OpenOptions};
use std::io::Write;

const ACTIVATE_RETRY_ATTEMPTS: usize = 30;
const ACTIVATE_RETRY_DELAY: std::time::Duration = std::time::Duration::from_millis(100);

pub struct InstanceGuard {
    lock_file: File,
}

impl InstanceGuard {
    fn new(lock_file: File) -> Self {
        Self { lock_file }
    }
}

impl Drop for InstanceGuard {
    fn drop(&mut self) {
        let _ = self.lock_file.unlock();
    }
}

pub fn acquire_or_activate_existing() -> Result<Option<InstanceGuard>> {
    let lock_path = instance_lock_path();
    if let Some(parent) = lock_path.parent() {
        std::fs::create_dir_all(parent).with_context(|| {
            format!(
                "Failed to create rumux runtime directory at {}",
                parent.display()
            )
        })?;
    }

    let mut lock_file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)
        .with_context(|| {
            format!(
                "Failed to open rumux instance lock at {}",
                lock_path.display()
            )
        })?;

    match lock_file.try_lock_exclusive() {
        Ok(()) => {
            lock_file.set_len(0).ok();
            writeln!(lock_file, "{}", std::process::id()).ok();
            lock_file.flush().ok();
            Ok(Some(InstanceGuard::new(lock_file)))
        }
        Err(_) => {
            activate_existing_instance()?;
            Ok(None)
        }
    }
}

fn activate_existing_instance() -> Result<()> {
    for _ in 0..ACTIVATE_RETRY_ATTEMPTS {
        if let Ok(response) = send_rpc("system.activate", serde_json::json!({}))
            && response.get("ok") == Some(&serde_json::Value::Bool(true))
        {
            return Ok(());
        }
        std::thread::sleep(ACTIVATE_RETRY_DELAY);
    }

    anyhow::bail!(
        "Another rumux instance is already running, but it could not be activated over IPC"
    )
}
