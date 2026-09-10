//! Atomic snapshots. Never overwrite unreadable input silently.
use crate::model::Document;
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use windows_sys::Win32::Storage::FileSystem::{
    MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
};

#[cfg(windows)]
fn path_wide(path: &Path) -> Vec<u16> {
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}

#[cfg(not(windows))]
fn path_wide(path: &Path) -> Vec<u16> {
    path.to_string_lossy()
        .encode_utf16()
        .chain(Some(0))
        .collect()
}

pub fn data_dir() -> Result<PathBuf, String> {
    if let Some(p) = std::env::args_os().skip_while(|a| a != "--data-dir").nth(1) {
        return Ok(PathBuf::from(p));
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let parent = exe.parent().ok_or("找不到程序目录")?;
    if parent.join("portable.flag").exists() {
        return Ok(parent.join("data"));
    }
    Ok(
        PathBuf::from(std::env::var_os("LOCALAPPDATA").ok_or("找不到本地应用数据目录")?)
            .join("LiteList"),
    )
}

fn read(path: &Path) -> Result<Document, String> {
    let meta = fs::metadata(path).map_err(|e| e.to_string())?;
    if meta.len() > 32 * 1024 * 1024 {
        return Err("数据文件超过 32 MB 读取上限".into());
    }
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    let mut doc: Document = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    doc.validate()?;
    Ok(doc)
}
pub fn load(dir: &Path) -> Result<(Document, Option<String>), String> {
    let main = dir.join("state.json");
    let backup = dir.join("state.backup.json");
    if !main.exists() && !backup.exists() {
        return Ok((Document::default(), None));
    }
    match read(&main) {
        Ok(doc) => Ok((doc, None)),
        Err(error) => {
            // Never downgrade newer schemas using an old backup.
            if let Ok(value) =
                fs::read(&main).map(|b| serde_json::from_slice::<serde_json::Value>(&b))
            {
                if let Ok(v) = value {
                    if v["version"].as_u64().is_some_and(|n| n > 2) {
                        return Err(error);
                    }
                }
            }
            match read(&backup) {
                Ok(doc) => {
                    if main.exists() {
                        fs::rename(
                            &main,
                            dir.join(format!("state.corrupt-{}.json", crate::model::now())),
                        )
                        .map_err(|e| e.to_string())?;
                    }
                    Ok((doc, Some("主数据异常，已从备份恢复。原文件已保留。".into())))
                }
                Err(_) => Err(format!(
                    "数据及备份无法读取，已停止启动以保护原文件。\n{}\n{error}",
                    dir.display()
                )),
            }
        }
    }
}
pub fn save(dir: &Path, doc: &Document) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let bytes = serde_json::to_vec_pretty(doc).map_err(|e| e.to_string())?;
    let main = dir.join("state.json");
    if main.exists() && read(&main).is_ok() {
        let old = fs::read(&main).map_err(|e| e.to_string())?;
        atomic_write(&dir.join("state.backup.json"), &old)?;
    }
    atomic_write(&main, &bytes)
}
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let temp = path.with_extension("tmp");
    let mut file = fs::File::create(&temp).map_err(|e| e.to_string())?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|e| e.to_string())?;
    drop(file);
    // Keep model/storage tests buildable on Linux without weakening exact
    // Windows path handling (which preserves unpaired UTF-16 code units).
    let src = path_wide(&temp);
    let dst = path_wide(path);
    // SAFETY: null-terminated paths live through the synchronous call.
    if unsafe {
        MoveFileExW(
            src.as_ptr(),
            dst.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    } == 0
    {
        return Err(std::io::Error::last_os_error().to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn dir() -> PathBuf {
        std::env::temp_dir().join(format!(
            "litelist-test-{}-{}",
            std::process::id(),
            crate::model::now()
        ))
    }
    #[test]
    fn corrupt_main_recovers_last_snapshot() {
        let p = dir();
        let mut d = Document::default();
        save(&p, &d).unwrap();
        d.settings.opacity = 100;
        save(&p, &d).unwrap();
        fs::write(p.join("state.json"), b"{truncated").unwrap();
        let (d, notice) = load(&p).unwrap();
        assert_eq!(d.settings.opacity, 220);
        assert!(notice.is_some());
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn future_schema_is_never_overwritten() {
        let p = dir().with_extension("future");
        save(&p, &Document::default()).unwrap();
        save(&p, &Document::default()).unwrap();
        fs::write(p.join("state.json"), br#"{"version":99}"#).unwrap();
        assert!(load(&p).is_err());
        assert_eq!(
            fs::read(p.join("state.json")).unwrap(),
            br#"{"version":99}"#
        );
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn no_valid_copy_fails_closed() {
        let p = dir().with_extension("bad");
        fs::create_dir_all(&p).unwrap();
        fs::write(p.join("state.json"), b"bad").unwrap();
        assert!(load(&p).is_err());
        assert!(p.join("state.json").exists());
        fs::remove_dir_all(p).unwrap();
    }
}
