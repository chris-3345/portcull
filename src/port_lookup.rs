use crate::args::Args;
use std::collections::HashSet;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::io;
use std::process::Command;

// ----------- Platform dispatch -----------

// Returns an error message (instead of panicking, which aborts the process) if the lookup tool can't be run
pub(crate) fn find_pids(args: &Args) -> Result<Vec<String>, String> {
    #[cfg(target_os = "windows")]
    {
        run_windows(args)
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        run_unix(args)
    }
}

// ----------- Linux / macOS -----------

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn run_unix(args: &Args) -> Result<Vec<String>, String> {
    let mut pids = HashSet::new();

    // map [3000, 8080] into "3000,8080"
    let ports_string = args
        .ports
        .iter()
        .map(|p| p.to_string())
        .collect::<Vec<String>>()
        .join(",");

    let port_arg = format!("-iTCP:{}", ports_string);

    // run lsof exactly once for all ports
    let output = Command::new("lsof")
        .args(["-t", &port_arg, "-sTCP:LISTEN"])
        .output()
        .map_err(|err| lsof_error_message(&err, &ports_string))?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    for line in stdout.lines() {
        let pid = line.trim();

        if !pid.is_empty() && pid.chars().all(|c| c.is_ascii_digit()) {
            pids.insert(pid.to_string());
        }
    }

    Ok(pids.into_iter().collect())
}

// lsof isn't installed by default on many minimal distros and containers,
// so tell the user how to fix it rather than just reporting the raw OS error
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn lsof_error_message(err: &io::Error, ports_string: &str) -> String {
    if err.kind() == io::ErrorKind::NotFound {
        "lsof was not found, but portcull needs it to look up ports on Linux/macOS. \
         Install it with your package manager (e.g. `apt install lsof`, `dnf install lsof` or `apk add lsof`) and try again."
            .to_string()
    } else {
        format!("Failed to execute lsof for ports {}: {}", ports_string, err)
    }
}

// ----------- Windows -----------

#[cfg(target_os = "windows")]
fn run_windows(args: &Args) -> Result<Vec<String>, String> {
    let target_ports: HashSet<u16> = args.ports.iter().cloned().collect();

    let output = Command::new("netstat")
        .arg("-ano")
        .output()
        .map_err(|err| format!("Failed to execute netstat: {}", err))?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    Ok(parse_netstat_listeners(&stdout, &target_ports)
        .into_iter()
        .collect())
}

// Split out from run_windows (and also compiled for tests) so the parsing can be unit tested on any OS
#[cfg(any(target_os = "windows", test))]
fn parse_netstat_listeners(stdout: &str, target_ports: &HashSet<u16>) -> HashSet<String> {
    let mut pids = HashSet::new();

    for line in stdout.lines() {
        let columns: Vec<&str> = line.split_whitespace().collect();

        // check if it's a valid TCP row: Proto, Local Address, Foreign Address, State, PID
        // (UDP rows have no State column and are skipped, matching the TCP-only lsof lookup on Unix)
        if columns.len() != 5 || columns[0] != "TCP" {
            continue;
        }

        let local_addr = columns[1];
        let foreign_addr = columns[2];
        let state = columns[3];
        let pid = columns[4]; // always the last column

        // only LISTENING sockets own the port (matches lsof -sTCP:LISTEN on Unix);
        // TIME_WAIT/ESTABLISHED/etc. rows are connections, and TIME_WAIT reports PID 0.
        // netstat localizes the State column on non-English Windows, so also accept the
        // wildcard foreign address (0.0.0.0:0 / [::]:0) that only listening sockets have
        if state != "LISTENING" && !foreign_addr.ends_with(":0") {
            continue;
        }

        // PID 0 (System Idle Process) and PID 4 (System, e.g. http.sys on port 80) are kernel-owned
        if pid == "0" || pid == "4" {
            continue;
        }

        // extract the port by finding the last colon in the addr
        if let Some(pos) = local_addr.rfind(':') {
            let port_str = &local_addr[pos + 1..];

            if let Ok(parsed_port) = port_str.parse::<u16>() {
                // check if it's what the user wanted
                if target_ports.contains(&parsed_port) && pid.chars().all(|c| c.is_ascii_digit()) {
                    pids.insert(pid.to_string());
                }
            }
        }
    }

    pids
}

#[cfg(test)]
mod tests {
    use super::*;

    const NETSTAT_SAMPLE: &str = "
Active Connections

  Proto  Local Address          Foreign Address        State           PID
  TCP    0.0.0.0:80             0.0.0.0:0              LISTENING       4
  TCP    0.0.0.0:3000           0.0.0.0:0              LISTENING       1111
  TCP    127.0.0.1:3000         127.0.0.1:52000        ESTABLISHED     1111
  TCP    127.0.0.1:52000        127.0.0.1:3000         ESTABLISHED     2222
  TCP    127.0.0.1:3001         127.0.0.1:52001        TIME_WAIT       0
  TCP    0.0.0.0:3002           0.0.0.0:0              ABHÖREN         3333
  TCP    [::]:3000              [::]:0                 LISTENING       1111
  TCP    [::]:8080              [::]:0                 LISTENING       4444
  UDP    0.0.0.0:3000           *:*                                    5555
  UDP    [::]:3003              *:*                                    6666
";

    fn pids_for(ports: &[u16]) -> Vec<String> {
        let target_ports: HashSet<u16> = ports.iter().cloned().collect();
        let mut pids: Vec<String> = parse_netstat_listeners(NETSTAT_SAMPLE, &target_ports)
            .into_iter()
            .collect();
        pids.sort();
        pids
    }

    #[test]
    fn netstat_matches_tcp_listeners_only() {
        // the ESTABLISHED and UDP rows on 3000 must not add extra PIDs
        assert_eq!(pids_for(&[3000]), vec!["1111"]);
    }

    #[test]
    fn netstat_matches_ipv6_listeners() {
        assert_eq!(pids_for(&[8080]), vec!["4444"]);
    }

    #[test]
    fn netstat_skips_time_wait_and_udp() {
        assert!(pids_for(&[3001, 3003]).is_empty());
    }

    #[test]
    fn netstat_skips_system_pids() {
        assert!(pids_for(&[80]).is_empty());
    }

    #[test]
    fn netstat_accepts_localized_listening_state() {
        assert_eq!(pids_for(&[3002]), vec!["3333"]);
    }

    #[test]
    fn netstat_ignores_client_side_of_connection() {
        // PID 2222 only has an outbound connection *to* 3000, it doesn't listen on 52000
        assert_eq!(pids_for(&[3000, 52000]), vec!["1111"]);
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn missing_lsof_gives_install_hint() {
        let message = lsof_error_message(&io::Error::from(io::ErrorKind::NotFound), "3000");
        assert!(message.contains("lsof was not found"));
        assert!(message.contains("install lsof"));
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn other_lsof_errors_include_ports_and_cause() {
        let message = lsof_error_message(
            &io::Error::from(io::ErrorKind::PermissionDenied),
            "3000,8080",
        );
        assert!(message.contains("3000,8080"));
        assert!(!message.contains("not found"));
    }
}
