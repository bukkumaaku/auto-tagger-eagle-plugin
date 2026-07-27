fn main() -> Result<(), ai_worker::WorkerError> {
    if llamafile::run_process_supervisor_if_requested() {
        return Ok(());
    }
    ai_worker::run_stdio()
}
