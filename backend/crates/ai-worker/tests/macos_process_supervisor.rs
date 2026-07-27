#![cfg(target_os = "macos")]

use std::{
    os::unix::process::CommandExt,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

#[test]
fn supervisor_terminates_llamafile_after_parent_exit() {
    let mut llamafile = Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", "waits_for_supervisor"])
        .env("LLAMAFILE_SUPERVISOR_TEST_CHILD", "1")
        .process_group(0)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut supervisor = Command::new(env!("CARGO_BIN_EXE_ai-worker"))
        .args([
            "--internal-llamafile-supervisor",
            "0",
            &llamafile.id().to_string(),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    let deadline = Instant::now() + Duration::from_secs(5);
    while llamafile.try_wait().unwrap().is_none() {
        if Instant::now() >= deadline {
            let _ = supervisor.kill();
            let _ = supervisor.wait();
            let _ = llamafile.kill();
            let _ = llamafile.wait();
            panic!("llamafile survived after its parent exited");
        }
        thread::sleep(Duration::from_millis(50));
    }
    assert!(supervisor.wait().unwrap().success());
}

#[test]
#[ignore]
fn waits_for_supervisor() {
    if std::env::var_os("LLAMAFILE_SUPERVISOR_TEST_CHILD").is_some() {
        thread::sleep(Duration::from_secs(30));
    }
}
