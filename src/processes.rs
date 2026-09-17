use crate::args::Args;
use std::io::{self, Write};
use std::thread;
use std::time::{Duration, Instant};
use sysinfo::{Pid, Signal, System};

const DEFAULT_GRACEFUL_TIMEOUT_SECONDS: u64 = 3;
const PROCESS_CHECK_INTERVAL_MS: u64 = 100;

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
                process_names.push(format!("Unknown Process"));
            }
        } else {
            process_names.push(format!("Invalid PID"));
        }
    }

    process_names
}

// ----------- Force killing -----------

fn sigkill_pid(sys: &System, pid: Pid) {
    if let Some(proc) = sys.process(pid) {
        let process_name = proc.name().to_string_lossy();

        if proc.kill() {
            println!("Force killed PID {} ({})", pid, process_name);
        } else {
            println!("Failed to force kill PID {} ({})", pid, process_name);
        }
    } else {
        println!("PID {} closed on its own...", pid);
    }
}

// ----------- Graceful killing -----------

fn wait_for_graceful_exit(sys: &mut System, pid: Pid, process_name: &str, timeout_seconds: u64) {
    let timeout = Duration::from_secs(timeout_seconds);
    let start = Instant::now();

    loop {
        sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);

        if sys.process(pid).is_none() {
            println!("PID {} ({}) exited gracefully.", pid, process_name);
            return;
        }

        if start.elapsed() >= timeout {
            println!(
                "PID {} ({}) did not exit after {}s. Force killing...",
                pid, process_name, timeout_seconds
            );

            sigkill_pid(sys, pid);
            return;
        }

        thread::sleep(Duration::from_millis(PROCESS_CHECK_INTERVAL_MS));
    }
}

// ----------- Main kill logic -----------

pub(crate) fn kill_pids(pids: &[String], process_names: &[String], args: &Args) {
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
            return;
        }
    }

    let mut sys = System::new(); // Re-init to get fresh state before killing
    println!("Killing processes!");

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

                            wait_for_graceful_exit(&mut sys, pid, process_name, timeout_seconds);
                        }

                        Some(false) => {
                            println!(
                                "Failed to send SIGTERM to PID {} ({}). Force killing...",
                                pid, process_name
                            );

                            sigkill_pid(&sys, pid);
                        }

                        None => {
                            println!(
                                "SIGTERM is not supported on this platform. Force killing PID {}.",
                                pid
                            );

                            sigkill_pid(&sys, pid);
                        }
                    }
                } else {
                    sigkill_pid(&sys, pid);
                }
            } else {
                println!("PID {} closed on its own...", pid);
            }
        }
    }
}
