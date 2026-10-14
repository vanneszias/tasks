#![cfg(all(feature = "pty", unix))]

use portable_pty::{
    Child, ChildKiller, CommandBuilder, ExitStatus, MasterPty, PtySize, native_pty_system,
};
use std::{
    fs, io,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
    thread,
    time::{Duration, Instant},
};
use tasks_foundation::process::OwnedProcess;

// Real PTYs, Unix sessions and foreground/background groups; no user's shell/config.
const FIXTURE: &str = r#"
import os, signal, sys, time
path, mode = sys.argv[1:]
signal.signal(signal.SIGTTOU, signal.SIG_IGN)
def worker(label):
    signal.signal(signal.SIGHUP, signal.SIG_IGN)
    signal.signal(signal.SIGTERM, signal.SIG_IGN)
    with open(path + '/' + label, 'w') as f: f.write(str(os.getpid()))
    time.sleep(8)
    with open(path + '/delayed-' + label, 'w') as f: f.write('unexpected execution')
    time.sleep(60)
if mode == 'groups':
    for label in ['foreground', 'background']:
        pid = os.fork()
        if pid == 0:
            os.setpgid(0, 0)
            worker(label)
            os._exit(0)
        if label == 'foreground':
            os.setpgid(pid, pid)
            os.tcsetpgrp(0, pid)
    pid = os.fork()
    if pid == 0:
        os.setsid()
        pid = os.fork()
        if pid == 0:
            os.setpgid(0, 0)
            worker('detached')
            os._exit(0)
        # Give the live ownership monitor time to capture this descendant session.
        time.sleep(0.7)
        os._exit(0)
    signal.signal(signal.SIGHUP, lambda *_: os._exit(0))
elif mode == 'stubborn':
    signal.signal(signal.SIGHUP, signal.SIG_IGN)
    signal.signal(signal.SIGTERM, signal.SIG_IGN)
else:
    signal.signal(signal.SIGHUP, lambda *_: os._exit(0))
with open(path + '/ready', 'w') as f: f.write(str(os.getpid()))
while True: time.sleep(0.05)
"#;

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Fixture {
    owned: OwnedProcess,
    _master: Box<dyn MasterPty + Send>,
    path: PathBuf,
    root: i32,
    started: Instant,
}

impl Fixture {
    fn new(mode: &str, fail_reap_once: bool) -> Self {
        let base = std::env::var_os("TMPDIR")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let path = base.join(format!(
            "tasks-shutdown-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        let pair = native_pty_system()
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .unwrap();
        let mut command = CommandBuilder::new("python3");
        command.args(["-c", FIXTURE, path.to_str().unwrap(), mode]);
        command.cwd(&path);
        let child = pair.slave.spawn_command(command).unwrap();
        let root = child.process_id().unwrap() as i32;
        let child: Box<dyn Child + Send + Sync> = if fail_reap_once {
            Box::new(FailedReap { child, fail: true })
        } else {
            child
        };
        let owned = OwnedProcess::new(child).unwrap();
        drop(pair.slave);
        let fixture = Self {
            owned,
            _master: pair.master,
            path,
            root,
            started: Instant::now(),
        };
        fixture.await_file("ready");
        if mode == "groups" {
            for name in ["foreground", "background", "detached"] {
                fixture.await_file(name);
            }
            thread::sleep(Duration::from_millis(1000));
        }
        fixture
    }

    fn await_file(&self, name: &str) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !self.path.join(name).exists() {
            assert!(
                Instant::now() < deadline,
                "fixture did not become ready: {}",
                self.path.display()
            );
            thread::sleep(Duration::from_millis(20));
        }
    }

    fn pid(&self, name: &str) -> i32 {
        fs::read_to_string(self.path.join(name))
            .unwrap()
            .parse()
            .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.owned.emergency_stop();
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn running(pid: i32) -> bool {
    let output = std::process::Command::new("/bin/ps")
        .args(["-p", &pid.to_string(), "-o", "stat="])
        .output()
        .unwrap();
    let status = String::from_utf8_lossy(&output.stdout);
    !status.trim().is_empty() && !status.trim().starts_with('Z')
}

#[test]
fn two_owned_ptys_stop_all_groups_and_captured_descendant_session_without_delayed_writes() {
    let mut first = Fixture::new("groups", false);
    let mut second = Fixture::new("groups", false);
    let mut unrelated = Fixture::new("stubborn", false);
    let started = Instant::now();
    for fixture in [&mut first, &mut second] {
        let pids = [
            fixture.root,
            fixture.pid("foreground"),
            fixture.pid("background"),
            fixture.pid("detached"),
        ];
        assert_ne!(unsafe { libc::getpgid(pids[1]) }, unsafe {
            libc::getpgid(pids[2])
        });
        assert_ne!(unsafe { libc::getsid(pids[3]) }, fixture.root);
        let report = fixture.owned.stop().unwrap();
        assert!(report.escalated, "stubborn groups must reach escalation");
        assert!(!report.root_forced, "root acknowledged HUP cleanly");
        for pid in pids {
            assert!(!running(pid), "owned process {pid} still executing");
        }
        assert!(running(unrelated.root), "unrelated session was signaled");
    }
    assert!(
        started.elapsed() < Duration::from_secs(12),
        "shutdown was not bounded"
    );
    // Pass the tools' delayed side-effect deadline, rather than infer from root exit.
    for fixture in [&first, &second] {
        thread::sleep((Duration::from_secs(9)).saturating_sub(fixture.started.elapsed()));
        for name in ["foreground", "background", "detached"] {
            assert!(!fixture.path.join(format!("delayed-{name}")).exists());
        }
    }
    assert!(running(unrelated.root));
    unrelated.owned.stop().unwrap();
}

#[test]
fn stubborn_root_is_bounded_and_forced_persistence_uncertainty_survives_retry() {
    let mut fixture = Fixture::new("stubborn", false);
    let started = Instant::now();
    let report = fixture.owned.stop().unwrap();
    assert!(report.root_forced && report.escalated);
    assert!(started.elapsed() < Duration::from_secs(6));
    assert!(!running(fixture.root));
    let retry = Instant::now();
    assert!(fixture.owned.stop().unwrap().root_forced);
    assert!(retry.elapsed() < Duration::from_millis(100));
}

// Inject acknowledgement failure at the existing Child boundary, never via a
// production environment-variable bypass or signals to arbitrary processes.
#[derive(Debug)]
struct FailedReap {
    child: Box<dyn Child + Send + Sync>,
    fail: bool,
}
impl ChildKiller for FailedReap {
    fn kill(&mut self) -> io::Result<()> {
        self.child.kill()
    }
    fn clone_killer(&self) -> Box<dyn ChildKiller + Send + Sync> {
        self.child.clone_killer()
    }
}
impl Child for FailedReap {
    fn process_id(&self) -> Option<u32> {
        self.child.process_id()
    }
    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        if std::mem::take(&mut self.fail) {
            Err(io::Error::other("injected exit acknowledgement failure"))
        } else {
            self.child.try_wait()
        }
    }
    fn wait(&mut self) -> io::Result<ExitStatus> {
        panic!("shutdown must never call blocking wait")
    }
}

#[test]
fn acknowledgement_failure_returns_error_and_retry_has_no_unbounded_wait() {
    let mut fixture = Fixture::new("graceful", true);
    let started = Instant::now();
    assert!(
        fixture
            .owned
            .stop()
            .unwrap_err()
            .to_string()
            .contains("acknowledgement failure")
    );
    assert!(started.elapsed() < Duration::from_secs(6));
    assert!(!running(fixture.root));
    let report = fixture.owned.stop().unwrap();
    assert!(!report.root_forced);
}
