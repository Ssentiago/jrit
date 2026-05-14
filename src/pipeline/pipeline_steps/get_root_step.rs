use anyhow::anyhow;
use std::path::PathBuf;

pub(crate) fn find_root() -> anyhow::Result<PathBuf> {
    let mut current = std::env::current_dir().expect("cwd unavailable");
    loop {
        if current.join("jrit.toml").exists() {
            return Ok(current);
        }

        let Some(parent) = current.parent() else {
            return Err(anyhow!("can't find jrit.toml",));
        };

        current = parent.to_path_buf();
    }
}
