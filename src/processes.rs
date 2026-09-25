use crate::args::Args;
use std::io::{self, Write};
use std::thread;
use std::time::{Duration, Instant};
use sysinfo::{Pid, Signal, System};

const DEFAULT_GRACEFUL_TIMEOUT_SECONDS: u64 = 3;
const PROCESS_CHECK_INTERVAL_MS: u64 = 100;

// Result of kill_pids, mapped to an exit code in main
#[derive(Debug, PartialEq)]
pub(crate) enum KillOutcome {
    // every process was killed (or had already exited)
    Killed,
    // the user declined the confirmation prompt
    Cancelled,
    // at least one process could not be killed
    Failed,
}

// ----------- Process information -----------

pub(crate) fn get_process_names(pids: &[String]) -> Vec<String> {
    let mut sys = System::new();
    let mut process_names: Vec<String> = Vec::new();

    for pid_str in pids {
        if let Ok(pid_num) = pid_str.parse::<usize>() {
            let pid = Pid::from(pid_num);

            sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);

            if let Some(proc) = sys.process(pid) {
                process_names.push(proc.name().to_string_lossy().into_owned());
            } else {
                process_names.push("Unknown Process".to_string());
            }
        } else {
            process_names.push("Invalid PID".to_string());
        }
    }

    process_names
}

// ----------- Force killing -----------

// Returns false only if the kill failed (a process that already exited counts as success)
fn sigkill_pid(sys: &System, pid: Pid) -> bool {
    if let Some(proc) = sys.process(pid) {
        let process_name = proc.name().to_string_lossy();

        if proc.kill() {
            println!("Force killed PID {} ({})", pid, process_name);
            true
        } else {
            println!("Failed to force kill PID {} ({})", pid, process_name);
            false
        }
    } else {
        println!("PID {} closed on its own...", pid);
        true
    }
}

// ----------- Graceful killing -----------

// Returns false only if the fallback SIGKILL failed
fn wait_for_graceful_exit(
    sys: &mut System,
    pid: Pid,
    process_name: &str,
    timeout_seconds: u64,
) -> bool {
    let timeout = Duration::from_secs(timeout_seconds);
    let start = Instant::now();

    loop {
        sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);

        if sys.process(pid).is_none() {
            println!("PID {} ({}) exited gracefully.", pid, process_name);
            return true;
        }

        if start.elapsed() >= timeout {
            println!(
                "PID {} ({}) did not exit after {}s. Force killing...",
                pid, process_name, timeout_seconds
            );

            return sigkill_pid(sys, pid);
        }

        thread::sleep(Duration::from_millis(PROCESS_CHECK_INTERVAL_MS));
    }
}

// ----------- Main kill logic -----------

pub(crate) fn kill_pids(pids: &[String], process_names: &[String], args: &Args) -> KillOutcome {
    if process_names
        .iter()
        .any(|name| name.to_lowercase().contains("ollama"))
    {
        println!("Ollama was detected in your list of processes to kill.");
        println!("Killing Ollama will NOT work because it will start the server again.");
        println!("Will try to kill Ollama anyway, but it is probably going to fail.\n");
    }

    let mut confirmation_prompt = String::new();

    if !args.quiet {
        print!(
            "Are you sure you want to kill these processes: {:?}? (y/N): ",
            process_names
        );

        io::stdout().flush().expect("Failed to flush stdout");

        io::stdin()
            .read_line(&mut confirmation_prompt)
            .expect("Failed to read input");

        if confirmation_prompt.trim().to_lowercase() != "y" {
            println!("Canceled.");
            return KillOutcome::Cancelled;
        }
    }

    let mut sys = System::new(); // Re-init to get fresh state before killing
    println!("Killing processes!");

    let mut all_killed = true;

    for (pid_str, process_name) in pids.iter().zip(process_names.iter()) {
        if let Ok(pid_num) = pid_str.parse::<usize>() {
            let pid = Pid::from(pid_num);

            sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);

            if let Some(proc) = sys.process(pid) {
                if args.graceful {
                    match proc.kill_with(Signal::Term) {
                        Some(true) => {
                            println!("Sent SIGTERM to PID {} ({})", pid, process_name);

                            let timeout_seconds =
                                args.timeout.unwrap_or(DEFAULT_GRACEFUL_TIMEOUT_SECONDS);

                            all_killed &= wait_for_graceful_exit(
                                &mut sys,
                                pid,
                                process_name,
                                timeout_seconds,
                            );
                        }

                        Some(false) => {
                            println!(
                                "Failed to send SIGTERM to PID {} ({}). Force killing...",
                                pid, process_name
                            );

                            all_killed &= sigkill_pid(&sys, pid);
                        }

                        None => {
                            println!(
                                "SIGTERM is not supported on this platform. Force killing PID {}.",
                                pid
                            );

                            all_killed &= sigkill_pid(&sys, pid);
                        }
                    }
                } else {
                    all_killed &= sigkill_pid(&sys, pid);
                }
            } else {
                println!("PID {} closed on its own...", pid);
            }
        }
    }

    if all_killed {
        KillOutcome::Killed
    } else {
        KillOutcome::Failed
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use clap::Parser;
    use std::process::Command;

    // spawn a throwaway process and reap it in the background, so it disappears
    // from the process table once killed instead of lingering as a zombie
    fn spawn_sleeper() -> (String, thread::JoinHandle<()>) {
        let mut child = Command::new("sleep").arg("30").spawn().unwrap();
        let pid = child.id().to_string();
        let reaper = thread::spawn(move || {
            child.wait().unwrap();
        });
        (pid, reaper)
    }

    #[test]
    fn kill_reports_killed() {
        let (pid, reaper) = spawn_sleeper();
        let args = Args::parse_from(["portcull", "-q", "1"]);

        let outcome = kill_pids(&[pid], &["sleep".to_string()], &args);

        assert_eq!(outcome, KillOutcome::Killed);
        reaper.join().unwrap();
    }

    #[test]
    fn graceful_kill_reports_killed() {
        let (pid, reaper) = spawn_sleeper();
        let args = Args::parse_from(["portcull", "-q", "-g", "1"]);

        let outcome = kill_pids(&[pid], &["sleep".to_string()], &args);

        assert_eq!(outcome, KillOutcome::Killed);
        reaper.join().unwrap();
    }

    #[test]
    fn already_exited_process_counts_as_killed() {
        let mut child = Command::new("true").spawn().unwrap();
        let pid = child.id().to_string();
        child.wait().unwrap();
        let args = Args::parse_from(["portcull", "-q", "1"]);

        let outcome = kill_pids(&[pid], &["true".to_string()], &args);

        assert_eq!(outcome, KillOutcome::Killed);
    }
}
