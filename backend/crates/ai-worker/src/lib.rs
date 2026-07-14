pub mod dispatch;
pub mod handlers;
pub mod runner;
pub mod sessions;
pub mod transport;

use std::io;

use handlers::BuiltinHandlers;
use runner::RunnerError;
use thiserror::Error;
use transport::JsonlTransport;

#[derive(Debug, Error)]
pub enum WorkerError {
    #[error(transparent)]
    Runner(#[from] RunnerError),
}

pub fn run_stdio() -> Result<(), WorkerError> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut transport = JsonlTransport::new(stdin.lock(), stdout.lock());
    let mut handlers = BuiltinHandlers::default();

    runner::run(&mut transport, &mut handlers)?;
    Ok(())
}
