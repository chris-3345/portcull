use crate::args::Args;
use std::collections::HashSet;
use std::process::Command;

// ----------- Platform dispatch -----------

pub(crate) fn find_pids(args: &Args) -> Vec<String> {
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
fn run_unix(args: &Args) -> Vec<String> {
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
        .unwrap_or_else(|err| {
            panic!("Failed to execute lsof for ports {}: {}", ports_string, err);
        });

    let stdout = String::from_utf8_lossy(&output.stdout);

    for line in stdout.lines() {
        let pid = line.trim();

        if !pid.is_empty() && pid.chars().all(|c| c.is_ascii_digit()) {
            pids.insert(pid.to_string());
        }
    }

    pids.into_iter().collect()
}

// ----------- Windows -----------

#[cfg(target_os = "windows")]
fn run_windows(args: &Args) -> Vec<String> {
    let mut pids = HashSet::new();

    let target_ports: HashSet<u16> = args.ports.iter().cloned().collect();

    let output = Command::new("netstat")
        .arg("-ano")
        .output()
        .expect("Failed to execute netstat");

    let stdout = String::from_utf8_lossy(&output.stdout);

    for line in stdout.lines() {
        let columns: Vec<&str> = line.split_whitespace().collect();

        // check if it's a valid connection
        if columns.len() >= 4 {
            let local_addr = columns[1];
            let pid = columns[columns.len() - 1]; // always the last column

            // extract the port by finding the last colon in the addr
            if let Some(pos) = local_addr.rfind(':') {
                let port_str = &local_addr[pos + 1..];

                if let Ok(parsed_port) = port_str.parse::<u16>() {
                    // check if it's what the user wanted
                    if target_ports.contains(&parsed_port) {
                        if pid.chars().all(|c| c.is_ascii_digit()) {
                            pids.insert(pid.to_string());
                        }
                    }
                }
            }
        }
    }

    pids.into_iter().collect()
}
