mod args;
mod port_lookup;
mod processes;

use args::Args;
use clap::Parser;
use port_lookup::find_pids;
use processes::{KillOutcome, get_process_names, kill_pids};
use std::process::ExitCode;

// Exit codes so portcull can be scripted (2 is left to clap for invalid arguments)
const EXIT_NOT_FOUND: u8 = 1;
const EXIT_CANCELLED: u8 = 3;
const EXIT_KILL_FAILED: u8 = 4;
const EXIT_LOOKUP_FAILED: u8 = 5;

fn main() -> ExitCode {
    let args = Args::parse();

    let proc_ids = match find_pids(&args) {
        Ok(proc_ids) => proc_ids,
        Err(err) => {
            eprintln!("Error: {}", err);
            return ExitCode::from(EXIT_LOOKUP_FAILED);
        }
    };

    if proc_ids.is_empty() {
        println!("No active processes found on provided ports; exiting.");

        // unprivileged lsof silently can't see sockets owned by other users
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        eprintln!(
            "Hint: without root, lsof only sees your own processes. If you expected something here, try sudo."
        );

        return ExitCode::from(EXIT_NOT_FOUND);
    }

    let process_names = get_process_names(&proc_ids);

    if args.query {
        println!("Active processes found:");

        for (pid, name) in proc_ids.iter().zip(process_names.iter()) {
            println!("PID: {} | Name: {}", pid, name);
        }

        return ExitCode::SUCCESS;
    }

    match kill_pids(&proc_ids, &process_names, &args) {
        KillOutcome::Killed => ExitCode::SUCCESS,
        KillOutcome::Cancelled => ExitCode::from(EXIT_CANCELLED),
        KillOutcome::Failed => ExitCode::from(EXIT_KILL_FAILED),
    }
}
