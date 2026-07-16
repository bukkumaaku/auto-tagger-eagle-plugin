use std::fs;
use std::path::{Path, PathBuf};

use protocol::{
    BackupFileEntry, BackupListRequest, BackupListResult, BackupReadRequest, BackupReadResult,
    BackupWriteRequest, BackupWriteResult,
};

use super::{HandlerError, HandlerResult};

pub fn write(request: BackupWriteRequest) -> HandlerResult<BackupWriteResult> {
    let directory = required_path(&request.directory, "directory")?;
    if request.filename.trim().is_empty()
        || request.filename == "."
        || request.filename == ".."
        || request.filename.contains('/')
        || request.filename.contains('\\')
    {
        return Err(HandlerError::new(
            "BACKUP_FILENAME_INVALID",
            "backup filename is invalid",
        ));
    }
    let path = directory.join(&request.filename);
    fs::create_dir_all(&directory)
        .map_err(|error| io_error("BACKUP_DIRECTORY_FAILED", &directory, error))?;
    fs::write(&path, request.content)
        .map_err(|error| io_error("BACKUP_WRITE_FAILED", &path, error))?;
    Ok(BackupWriteResult {
        path: path.to_string_lossy().into_owned(),
    })
}

pub fn list(request: BackupListRequest) -> HandlerResult<BackupListResult> {
    let directory = required_path(&request.directory, "directory")?;
    let mut entries = Vec::new();
    let read_dir = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(BackupListResult {
                directory: request.directory,
                entries,
            });
        }
        Err(error) => return Err(io_error("BACKUP_LIST_FAILED", &directory, error)),
    };
    for entry in read_dir {
        let entry = entry.map_err(|error| io_error("BACKUP_LIST_FAILED", &directory, error))?;
        let path = entry.path();
        if path.is_file()
            && path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
        {
            entries.push(BackupFileEntry {
                name: entry.file_name().to_string_lossy().into_owned(),
                path: path.to_string_lossy().into_owned(),
            });
        }
    }
    entries.sort_by(|left, right| right.name.cmp(&left.name));
    Ok(BackupListResult {
        directory: request.directory,
        entries,
    })
}

pub fn read(request: BackupReadRequest) -> HandlerResult<BackupReadResult> {
    let path = required_path(&request.path, "path")?;
    let content =
        fs::read_to_string(&path).map_err(|error| io_error("BACKUP_READ_FAILED", &path, error))?;
    Ok(BackupReadResult {
        path: request.path,
        content,
    })
}

fn required_path(value: &str, name: &str) -> HandlerResult<PathBuf> {
    if value.trim().is_empty() {
        return Err(HandlerError::new(
            "BACKUP_PATH_EMPTY",
            format!("backup {name} cannot be empty"),
        ));
    }
    Ok(PathBuf::from(value))
}

fn io_error(code: &str, path: &Path, error: std::io::Error) -> HandlerError {
    HandlerError::new(code, format!("{}: {error}", path.display()))
}
