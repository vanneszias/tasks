//! Live PTY ownership only: never reconstruct signal targets from stored PIDs.
use portable_pty::Child;
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{self, Read},
    process::{Command, Stdio},
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, Instant},
};

const OBSERVATION_LIMIT: Duration = Duration::from_millis(400);

#[derive(Clone)]
struct Process {
    pid: i32,
    parent: i32,
    group: i32,
    session: i32,
    birth: u128,
    running: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use portable_pty::{CommandBuilder, MasterPty, PtySize, native_pty_system};

    struct Disposable {
        owned: OwnedProcess,
        _master: Box<dyn MasterPty + Send>,
        ready: std::path::PathBuf,
    }

    impl Disposable {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let ready = std::env::temp_dir().join(format!(
                "tasks-shutdown-fault-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            let pair = native_pty_system()
                .openpty(PtySize {
                    rows: 24,
                    cols: 80,
                    pixel_width: 0,
                    pixel_height: 0,
                })
                .unwrap();
            let mut command = CommandBuilder::new("python3");
            command.args(["-c", "import signal, sys, time; signal.signal(signal.SIGHUP, signal.SIG_IGN); signal.signal(signal.SIGTERM, signal.SIG_IGN); open(sys.argv[1], 'w').close(); time.sleep(60)", ready.to_str().unwrap()]);
            let owned = OwnedProcess::new(pair.slave.spawn_command(command).unwrap()).unwrap();
            drop(pair.slave);
            let result = Self {
                owned,
                _master: pair.master,
                ready,
            };
            let deadline = Instant::now() + Duration::from_secs(5);
            while !result.ready.exists() {
                assert!(
                    Instant::now() < deadline,
                    "disposable child startup timed out"
                );
                thread::sleep(Duration::from_millis(20));
            }
            result
        }
    }

    impl Drop for Disposable {
        fn drop(&mut self) {
            self.owned.signal_error = false;
            self.owned.ownership.lock().unwrap().observation_error = None;
            self.owned.emergency_stop();
            let _ = std::fs::remove_file(&self.ready);
        }
    }

    #[test]
    fn observation_and_signal_failures_cancel_stop_without_losing_live_retry_ownership() {
        let mut fixture = Disposable::new();
        fixture.owned.ownership.lock().unwrap().observation_error =
            Some("injected observation failure".into());
        let started = Instant::now();
        assert!(
            fixture
                .owned
                .stop()
                .unwrap_err()
                .to_string()
                .contains("observation failure")
        );
        assert!(started.elapsed() < Duration::from_secs(1));
        assert!(!fixture.owned.stopped);
        fixture.owned.signal_error = true;
        let started = Instant::now();
        assert!(
            fixture
                .owned
                .stop()
                .unwrap_err()
                .to_string()
                .contains("signal failure")
        );
        assert!(started.elapsed() < Duration::from_secs(4));
        assert!(!fixture.owned.stopped);
        assert!(
            !fixture.owned.report.root_forced,
            "a refused signal is not a forced exit"
        );
        fixture.owned.signal_error = false;
        let started = Instant::now();
        assert!(fixture.owned.stop().unwrap().root_forced);
        assert!(started.elapsed() < Duration::from_secs(6));
    }

    #[test]
    fn reused_birth_identity_is_not_adopted_or_signaled_even_if_pid_and_session_match() {
        let mut fixture = Disposable::new();
        let root = fixture.owned.ownership.lock().unwrap().root;
        let actual = fixture
            .owned
            .observe()
            .unwrap()
            .into_iter()
            .find(|p| p.pid == root)
            .unwrap();
        let mut stale = actual.clone();
        stale.birth += 1;
        let mut ownership = Ownership {
            root,
            root_birth: stale.birth,
            members: BTreeMap::from([(root, stale.birth)]),
            observation_error: None,
            active: true,
        };
        assert!(ownership.capture(vec![actual]).unwrap().is_empty());
        fixture.owned.signal(&[stale], libc::SIGKILL).unwrap();
        assert!(
            fixture.owned.child.try_wait().unwrap().is_none(),
            "stale identity killed live replacement"
        );
        fixture.owned.stop().unwrap();
    }
}

// Birth identity is checked again before every signal. Session/group numbers alone
// can be reused after detached descendants exit, even while our root is unreaped.
#[cfg(target_os = "macos")]
fn birth(pid: i32) -> io::Result<u128> {
    let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of_val(&info) as i32;
    let read = unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDTBSDINFO,
            0,
            (&mut info as *mut libc::proc_bsdinfo).cast(),
            size,
        )
    };
    if read != size {
        return Err(io::Error::last_os_error());
    }
    Ok((u128::from(info.pbi_start_tvsec) << 64) | u128::from(info.pbi_start_tvusec))
}

#[cfg(target_os = "linux")]
fn birth(pid: i32) -> io::Result<u128> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat"))?;
    stat.rsplit_once(") ")
        .and_then(|(_, fields)| fields.split_whitespace().nth(19))
        .ok_or_else(|| io::Error::other("Missing process birth identity"))?
        .parse()
        .map_err(io::Error::other)
}

fn gone(error: &io::Error) -> bool {
    matches!(error.raw_os_error(), Some(libc::ESRCH) | Some(libc::ENOENT))
}

fn processes() -> io::Result<Vec<Process>> {
    let mut child = Command::new("/bin/ps")
        .args(["-axo", "pid=,ppid=,pgid=,stat=,uid="])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let mut stdout = child.stdout.take().unwrap();
    let (send, receive) = mpsc::channel();
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = stdout.read_to_end(&mut bytes).map(|_| bytes);
        let _ = send.send(result);
    });
    let deadline = Instant::now() + OBSERVATION_LIMIT;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            // Reap off-thread: even observation failure must not block Quit/retry.
            thread::spawn(move || {
                let _ = child.wait();
            });
            return Err(io::Error::other(
                "Process observation timed out; retry Quit",
            ));
        }
        thread::sleep(Duration::from_millis(5));
    };
    if !status.success() {
        return Err(io::Error::other(
            "Cannot inspect owned PTY processes; retry Quit",
        ));
    }
    let bytes = receive
        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        .map_err(|_| io::Error::other("Process observation output timed out; retry Quit"))??;
    let text = String::from_utf8(bytes).map_err(io::Error::other)?;
    let mut result = Vec::new();
    for line in text.lines() {
        if Instant::now() >= deadline {
            return Err(io::Error::other(
                "Process observation deadline exceeded; retry Quit",
            ));
        }
        let columns: Vec<_> = line.split_whitespace().collect();
        if columns.len() != 5 {
            return Err(io::Error::other("Unexpected ps process fields"));
        }
        let number = |index: usize| columns[index].parse::<i32>().map_err(io::Error::other);
        let same_user = number(4)? as u32 == unsafe { libc::getuid() };
        let pid = number(0)?;
        // Keep foreign-user ancestry/session metadata, but don't require identity
        // access to unrelated protected processes. If one is owned, capture fails.
        let identity = match if same_user { birth(pid) } else { Ok(0) } {
            Ok(identity) => identity,
            Err(error) if gone(&error) => continue,
            Err(error) => return Err(error),
        };
        let mut session = unsafe { libc::getsid(pid) };
        if session < 0 {
            let error = io::Error::last_os_error();
            if gone(&error) {
                continue;
            }
            if same_user {
                return Err(error);
            }
            session = 0;
        }
        result.push(Process {
            pid,
            parent: number(1)?,
            group: number(2)?,
            session,
            birth: identity,
            running: !columns[3].starts_with('Z'),
        });
    }
    Ok(result)
}

struct Ownership {
    root: i32,
    root_birth: u128,
    members: BTreeMap<i32, u128>,
    observation_error: Option<String>,
    active: bool,
}

impl Ownership {
    fn capture(&mut self, all: Vec<Process>) -> io::Result<Vec<Process>> {
        let mut descendants: BTreeSet<_> = all
            .iter()
            .filter(|p| self.members.get(&p.pid) == Some(&p.birth))
            .map(|p| p.pid)
            .collect();
        if all
            .iter()
            .any(|p| p.pid == self.root && p.birth == self.root_birth)
        {
            descendants.insert(self.root);
        }
        if all
            .iter()
            .any(|p| descendants.contains(&p.pid) && p.birth == 0)
        {
            return Err(io::Error::other(
                "Cannot validate an owned process after its user identity changed; Quit canceled",
            ));
        }
        loop {
            let before = descendants.len();
            for process in &all {
                if descendants.contains(&process.parent) {
                    descendants.insert(process.pid);
                }
            }
            if descendants.len() == before {
                break;
            }
        }
        let sessions: BTreeSet<_> = all
            .iter()
            .filter(|p| descendants.contains(&p.pid))
            .map(|p| p.session)
            .collect();
        let owned: Vec<_> = all
            .into_iter()
            .filter(|p| sessions.contains(&p.session))
            .collect();
        if owned.iter().any(|p| p.birth == 0) {
            return Err(io::Error::other(
                "Cannot validate all members of an owned session; Quit canceled",
            ));
        }
        // A session without a still-live captured identity is never adopted by SID alone.
        self.members = owned.iter().map(|p| (p.pid, p.birth)).collect();
        Ok(owned.into_iter().filter(|p| p.running).collect())
    }
}

pub struct OwnedProcess {
    child: Box<dyn Child + Send + Sync>,
    ownership: Arc<Mutex<Ownership>>,
    stopped: bool,
    report: StopReport,
    #[cfg(test)]
    signal_error: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct StopReport {
    pub escalated: bool,
    pub root_forced: bool,
}

impl OwnedProcess {
    pub fn new(child: Box<dyn Child + Send + Sync>) -> io::Result<Self> {
        let root = child
            .process_id()
            .ok_or_else(|| io::Error::other("PTY child has no PID"))? as i32;
        if unsafe { libc::getsid(root) } != root {
            return Err(io::Error::other("PTY child must own a new Unix session"));
        }
        let root_birth = birth(root)?;
        let ownership = Arc::new(Mutex::new(Ownership {
            root,
            root_birth,
            members: BTreeMap::from([(root, root_birth)]),
            observation_error: None,
            active: true,
        }));
        // Capture descendant sessions during execution, before their parents can exit.
        // Unix ancestry polling cannot prove ownership of a double-fork between polls.
        let weak = Arc::downgrade(&ownership);
        thread::spawn(move || {
            loop {
                thread::sleep(Duration::from_millis(100));
                let Some(ownership) = weak.upgrade() else {
                    break;
                };
                // Serialize snapshots with stop; never apply an old snapshot after a newer one.
                let mut ownership = ownership.lock().unwrap();
                if !ownership.active {
                    break;
                }
                match processes().and_then(|all| ownership.capture(all)) {
                    Ok(_) => {}
                    Err(error) => {
                        ownership.observation_error = Some(error.to_string());
                    }
                }
            }
        });
        Ok(Self {
            child,
            ownership,
            stopped: false,
            report: StopReport::default(),
            #[cfg(test)]
            signal_error: false,
        })
    }

    fn observe(&mut self) -> io::Result<Vec<Process>> {
        let mut ownership = self.ownership.lock().unwrap();
        if let Some(error) = ownership.observation_error.take() {
            return Err(io::Error::other(format!(
                "Owned process observation failed: {error}; retry Quit"
            )));
        }
        ownership.capture(processes()?)
    }

    fn signal(&mut self, owned: &[Process], signal: i32) -> io::Result<()> {
        #[cfg(test)]
        if self.signal_error {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "injected signal failure",
            ));
        }
        let root = self.ownership.lock().unwrap().root;
        for group in owned.iter().map(|p| p.group).collect::<BTreeSet<_>>() {
            if group <= 1 || group == unsafe { libc::getpgrp() } {
                return Err(io::Error::other(
                    "Refusing to signal an unrelated process group",
                ));
            }
            let mut live = false;
            for p in owned.iter().filter(|p| p.group == group) {
                match birth(p.pid) {
                    Ok(identity) if identity == p.birth => {
                        let session = unsafe { libc::getsid(p.pid) };
                        let current_group = unsafe { libc::getpgid(p.pid) };
                        if session < 0 || current_group < 0 {
                            let error = io::Error::last_os_error();
                            if !gone(&error) {
                                return Err(error);
                            }
                        } else if session == p.session && current_group == group {
                            live = true;
                        }
                    }
                    Ok(_) => {}
                    Err(error) if gone(&error) => {}
                    Err(error) => return Err(error),
                }
            }
            if live {
                if unsafe { libc::kill(-group, signal) } != 0 {
                    let error = io::Error::last_os_error();
                    if !gone(&error) {
                        return Err(error);
                    }
                } else if signal != libc::SIGHUP
                    && owned.iter().any(|p| p.pid == root && p.group == group)
                {
                    self.report.root_forced = true;
                }
            }
        }
        Ok(())
    }

    fn wait_until(&mut self, duration: Duration) -> io::Result<Vec<Process>> {
        let deadline = Instant::now() + duration;
        loop {
            let owned = self.observe()?;
            if owned.is_empty() || Instant::now() >= deadline {
                return Ok(owned);
            }
            thread::sleep(Duration::from_millis(25));
        }
    }

    /// HUP the root to let standalone persist; then TERM/KILL remaining owned groups.
    /// Forced-root history uncertainty is latched across retries, including after reaping.
    pub fn stop(&mut self) -> io::Result<StopReport> {
        if self.stopped {
            return Ok(self.report);
        }
        let owned = self.observe()?;
        let root = self.ownership.lock().unwrap().root;
        if let Some(p) = owned.iter().find(|p| p.pid == root) {
            if birth(root)? == p.birth && unsafe { libc::kill(root, libc::SIGHUP) } != 0 {
                let error = io::Error::last_os_error();
                if !gone(&error) {
                    return Err(error);
                }
            }
        }
        let mut remaining = self.wait_until(Duration::from_millis(1500))?;
        self.report.escalated |= !remaining.is_empty();
        if !remaining.is_empty() {
            self.signal(&remaining, libc::SIGTERM)?;
            remaining = self.wait_until(Duration::from_millis(750))?;
        }
        if !remaining.is_empty() {
            self.signal(&remaining, libc::SIGKILL)?;
            remaining = self.wait_until(Duration::from_millis(750))?;
        }
        if !remaining.is_empty() {
            return Err(io::Error::other(
                "Owned processes remain after shutdown deadline; retry Quit",
            ));
        }
        // Never call blocking wait(), even after the process snapshot says it exited.
        let deadline = Instant::now() + Duration::from_millis(400);
        loop {
            if let Some(status) = self.child.try_wait()? {
                self.report.root_forced |= !status.success();
                self.stopped = true;
                self.ownership.lock().unwrap().active = false;
                eprintln!(
                    "owned PTY {root} stopped before Quit: {status:?}; {:?}",
                    self.report
                );
                return Ok(self.report);
            }
            if Instant::now() >= deadline {
                return Err(io::Error::other(
                    "PTY exit acknowledgement timed out; retry Quit",
                ));
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    /// Unexpected teardown is best effort, never clean-shutdown/persistence evidence.
    pub fn emergency_stop(&mut self) {
        if !self.stopped {
            if let Ok(owned) = self.observe() {
                let _ = self.signal(&owned, libc::SIGKILL);
            }
        }
    }
}
