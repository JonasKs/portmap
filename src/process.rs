use std::process::Command;

/// Find PIDs listening on a given port.
/// Returns `Err` if lsof fails to execute, `Ok(vec)` otherwise.
pub fn find_listeners(port: u16) -> Result<Vec<String>, std::io::Error> {
    let output = Command::new("lsof")
        .args(["-ti", &format!(":{port}"), "-sTCP:LISTEN"])
        .output()?;
    // lsof exits 1 when no matches found (normal), but other codes are errors
    if !output.status.success() && output.status.code() != Some(1) {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(std::io::Error::other(format!(
            "lsof exited with {}: {}",
            output.status,
            stderr.trim()
        )));
    }
    let stdout = String::from_utf8(output.stdout).unwrap_or_default();
    Ok(stdout
        .lines()
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect())
}

/// Kill result indicating what happened.
#[derive(Debug)]
pub enum KillResult {
    /// Nothing was listening on the port.
    NotFound,
    /// Process exited after SIGTERM.
    Killed,
    /// Process required SIGKILL after SIGTERM didn't work.
    ForceKilled,
    /// Failed to execute lsof or kill, or permission denied.
    Error(String),
}

/// Check if a PID exists, independent of process ownership.
/// Uses `ps -p` which works for any process regardless of who owns it.
/// Returns `Err` if `ps` itself fails to execute.
fn pid_exists(pid: &str) -> Result<bool, String> {
    Command::new("ps")
        .args(["-p", pid, "-o", "pid="])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .map(|o| o.status.success())
        .map_err(|e| format!("Failed to check process state: {e}"))
}

fn all_exited(pids: &[String]) -> Result<bool, String> {
    for pid in pids {
        if pid_exists(pid)? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Kill the process listening on a port. Sends SIGTERM first, polls for exit
/// (up to 2s), then SIGKILL any survivors.
pub async fn kill_port(port: u16) -> KillResult {
    let pids = match find_listeners(port) {
        Ok(p) => p,
        Err(e) => return KillResult::Error(format!("Failed to find listeners: {e}")),
    };
    if pids.is_empty() {
        return KillResult::NotFound;
    }

    // SIGTERM (graceful)
    for pid in &pids {
        let _ = Command::new("kill").arg(pid).status();
    }

    // Poll for exit using ps (ownership-independent)
    for _ in 0..20 {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        match all_exited(&pids) {
            Ok(true) => return KillResult::Killed,
            Err(e) => return KillResult::Error(e),
            Ok(false) => {}
        }
    }

    // Still alive after 2s — SIGKILL survivors
    let mut sent_sigkill = false;
    for pid in &pids {
        match pid_exists(pid) {
            Ok(false) => {} // already gone
            Err(e) => return KillResult::Error(e),
            Ok(true) => {
                let ok = Command::new("kill")
                    .args(["-9", pid])
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status()
                    .is_ok_and(|s| s.success());
                if ok {
                    sent_sigkill = true;
                } else {
                    match pid_exists(pid) {
                        Ok(true) => {
                            return KillResult::Error(format!(
                                "Failed to kill PID {pid} (permission denied?)"
                            ));
                        }
                        Err(e) => return KillResult::Error(e),
                        Ok(false) => {} // raced away
                    }
                }
            }
        }
    }

    if sent_sigkill {
        KillResult::ForceKilled
    } else {
        KillResult::Killed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_listeners_unused_port() {
        let result = find_listeners(19);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_find_listeners_returns_ok() {
        let result = find_listeners(1);
        assert!(result.is_ok());
    }

    #[test]
    fn test_pid_exists_init() {
        // PID 1 (launchd/init) always exists
        assert_eq!(pid_exists("1"), Ok(true));
    }

    #[test]
    fn test_pid_exists_dead() {
        // PID 999999999 should not exist
        assert_eq!(pid_exists("999999999"), Ok(false));
    }

    #[tokio::test]
    async fn test_kill_port_not_found() {
        let result = kill_port(19).await;
        assert!(matches!(result, KillResult::NotFound));
    }

    #[tokio::test]
    async fn test_kill_port_kills_subprocess() {
        use std::process::Stdio;

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        let child = Command::new("nc")
            .args(["-l", &port.to_string()])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();

        let Ok(mut child) = child else {
            return; // nc not available, skip
        };

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        let result = kill_port(port).await;
        assert!(matches!(
            result,
            KillResult::Killed | KillResult::ForceKilled
        ));

        let _ = child.kill();
        let _ = child.wait();
    }
}
