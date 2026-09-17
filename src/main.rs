mod args;
mod port_lookup;
mod processes;

use args::Args;
use clap::Parser;
use port_lookup::find_pids;
use processes::{get_process_names, kill_pids};

fn main() {
    let args = Args::parse();

    let proc_ids = find_pids(&args);

    if proc_ids.is_empty() {
        println!("No active processes found on provided ports; exiting.");
        return;
    }

    let process_names = get_process_names(&proc_ids);

    if args.query {
        println!("Active processes found:");

        for (pid, name) in proc_ids.iter().zip(process_names.iter()) {
            println!("PID: {} | Name: {}", pid, name);
        }

        return;
    }

    kill_pids(&proc_ids, &process_names, &args);
}
