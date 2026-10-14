//! Live PTY ownership only: never reconstruct signal targets from stored PIDs.
use portable_pty::Child;
use std::{
    collections::BTreeSet,
    io,
    process::Command,
    thread,
    time::{Duration, Instant},
};

struct Process {
    pid: i32,
    parent: i32,
    group: i32,
    session: i32,
    running: bool,
}

fn processes() -> io::Result<Vec<Process>> {
    let output = Command::new("/bin/ps")
        .args(["-axo", "pid=,ppid=,pgid=,stat="])
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other("Cannot inspect owned PTY processes"));
    }
    let mut result = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let columns: Vec<_> = line.split_whitespace().collect();
        if columns.len() != 4 {
            return Err(io::Error::other("Unexpected ps process fields"));
        }
        let number = |index: usize| columns[index].parse::<i32>().map_err(io::Error::other);
        let pid = number(0)?;
        // The child handle remains unreaped until shutdown finishes, protecting its PID/SID.
        let session = unsafe { libc::getsid(pid) };
        if session > 0 {
            result.push(Process {
                pid,
                parent: number(1)?,
                group: number(2)?,
                session,
                running: !columns[3].starts_with('Z'),
            });
        }
    }
    Ok(result)
}

pub struct OwnedProcess {
    child: Box<dyn Child + Send + Sync>,
    root: i32,
    sessions: BTreeSet<i32>,
    stopped: bool,
}

#[derive(Debug)]
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
        Ok(Self {
            child,
            root,
            sessions: BTreeSet::from([root]),
            stopped: false,
        })
    }

    fn observe(&mut self) -> io::Result<Vec<Process>> {
        let all = processes()?;
        let mut descendants = BTreeSet::from([self.root]);
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
        for process in &all {
            if descendants.contains(&process.pid) {
                self.sessions.insert(process.session);
            }
        }
        Ok(all
            .into_iter()
            .filter(|p| p.running && self.sessions.contains(&p.session))
            .collect())
    }

    fn signal(&self, owned: &[Process], signal: i32) -> io::Result<()> {
        let groups: BTreeSet<_> = owned.iter().map(|p| p.group).collect();
        for group in groups {
            if group <= 1 || group == unsafe { libc::getpgrp() } {
                return Err(io::Error::other(
                    "Refusing to signal an unrelated process group",
                ));
            }
            // Check a live member still belongs to a captured owned session immediately before signaling.
            if !owned.iter().any(|p| {
                p.group == group
                    && unsafe { libc::getsid(p.pid) } == p.session
                    && unsafe { libc::getpgid(p.pid) } == group
            }) {
                continue;
            }
            if unsafe { libc::kill(-group, signal) } != 0 {
                let error = io::Error::last_os_error();
                if error.raw_os_error() != Some(libc::ESRCH) {
                    return Err(error);
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

    /// Graceful HUP first; TERM/KILL escalation for remaining owned session groups.
    /// A forced root exit must not be described as acknowledged agent persistence.
    pub fn stop(&mut self) -> io::Result<StopReport> {
        if self.stopped {
            return Ok(StopReport {
                escalated: false,
                root_forced: false,
            });
        }
        let owned = self.observe()?;
        if owned.iter().any(|p| p.pid == self.root)
            && unsafe { libc::kill(self.root, libc::SIGHUP) } != 0
        {
            let error = io::Error::last_os_error();
            if error.raw_os_error() != Some(libc::ESRCH) {
                return Err(error);
            }
        }
        let mut remaining = self.wait_until(Duration::from_millis(1500))?;
        let report = StopReport {
            escalated: !remaining.is_empty(),
            root_forced: remaining.iter().any(|p| p.pid == self.root),
        };
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
                "Owned processes remain after shutdown deadline; Quit canceled",
            ));
        }
        // All owned processes are exited/zombies; reaping our child cannot block.
        let status = self.child.wait()?;
        self.stopped = true;
        eprintln!(
            "owned PTY {} stopped before Quit: {status:?}; {report:?}",
            self.root
        );
        Ok(report)
    }

    /// Last-resort signal on unexpected teardown, not proof of clean shutdown.
    pub fn emergency_stop(&mut self) {
        if !self.stopped {
            if let Ok(owned) = self.observe() {
                let _ = self.signal(&owned, libc::SIGKILL);
            }
        }
    }
}
