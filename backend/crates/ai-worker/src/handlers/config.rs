use config_store::ConfigStore;
use protocol::{ReadConfigResult, WriteConfigRequest, WriteConfigResult};

use super::{HandlerError, HandlerResult};

pub fn read() -> HandlerResult<ReadConfigResult> {
    let store = user_store()?;
    let config = store
        .read()
        .map_err(|error| HandlerError::new("CONFIG_READ_FAILED", error.to_string()))?;

    Ok(ReadConfigResult {
        config: Box::new(config),
        path: store.path().to_string_lossy().into_owned(),
    })
}

pub fn write(request: WriteConfigRequest) -> HandlerResult<WriteConfigResult> {
    let store = user_store()?;
    store
        .write(&request.config)
        .map_err(|error| HandlerError::new("CONFIG_WRITE_FAILED", error.to_string()))?;

    Ok(WriteConfigResult {
        config: request.config,
        path: store.path().to_string_lossy().into_owned(),
    })
}

fn user_store() -> HandlerResult<ConfigStore> {
    ConfigStore::for_current_user()
        .map_err(|error| HandlerError::new("CONFIG_PATH_FAILED", error.to_string()))
}
