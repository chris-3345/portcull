use clap::Parser;

#[derive(Parser, Debug)]
pub(crate) struct Args {
    /// The ports to kill or query
    #[arg(required_unless_present = "licenses")]
    pub(crate) ports: Vec<u16>,

    /// Display active processes on the port(s) without killing
    #[arg(long)]
    pub(crate) query: bool,

    /// Run without confirmation prompt
    #[arg(long, short)]
    pub(crate) quiet: bool,

    /// Use SIGTERM instead of SIGKILL (more graceful exit; defaults to SIGKILL)
    #[arg(long, short, conflicts_with = "query")]
    pub(crate) graceful: bool,

    /// Override default timeout for graceful exit (will then fall back to SIGKILL - default timeout is 3s)
    #[arg(long, requires = "graceful")]
    pub(crate) timeout: Option<u64>,

    /// Print third-party license notices and exit
    #[arg(long, exclusive = true)]
    pub(crate) licenses: bool,
}
