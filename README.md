# portcull
Port Cull. No, the name isn't that accurate, but I couldn't think of a better one. It culls processes... by port.

`portcull` is a Rust util that accepts a port (or list of ports), finds the processes attached to them, and kills them.

## Example Usage
Were you vibe-coding (admit it, you were, we all do sometimes) and your agent left a rogue `node server.js` running, and you want to free that port so you can do... whatever you do on your ports?
Simply run `portcull`!
```bash
$ portcull 3030
Are you sure you want to kill these processes: ["node.exe"]? (y/N): y
Killing processes!
Killed PID 28644 (node.exe)
```

Additionally, you can now query a port without killing the process:
```bash
$ portcull --query 8000
Active processes found:
PID: 27656 | Name: node.exe
```
(you can run it with as many ports as you want and it'll attempt to kill processes associated with all of them)

Current --help/-h output:
```bash
Usage: portcull.exe [OPTIONS] [PORTS]...

Arguments:
  [PORTS]...  The ports to kill or query

Options:
      --query              Display active processes on the port(s) without killing
  -q, --quiet              Run without confirmation prompt
  -g, --graceful           Use SIGTERM instead of SIGKILL (more graceful exit; defaults to SIGKILL)
      --timeout <TIMEOUT>  Override default timeout for graceful exit (will then fall back to SIGKILL - default timeout is 3s)
      --licenses           Print third-party license notices and exit
  -h, --help               Print help
```

## Exit Codes
`portcull` exits with a nonzero status when it didn't do what you asked, so `portcull -q` can be used in scripts:

| Code | Meaning |
|------|---------|
| 0 | Success (processes were killed, or `--query` found processes) |
| 1 | No processes found on the provided ports |
| 2 | Invalid arguments |
| 3 | Canceled at the confirmation prompt |
| 4 | One or more processes could not be killed |
| 5 | Port lookup failed (e.g. `lsof` is not installed) |

## Installation
### Quick Setup
Go to our releases page, download the correct release for your architecture and OS, rename it to `portcull`, and put it somewhere that's in your $PATH!
(note that if the release is very recent, our GitHub Action may still be running, so you may need to wait)

### Manual
Clone the repo and install via `cargo`:
```bash
git clone https://github.com/chris-3345/portcull.git
cd portcull
cargo install --path .
```

For the above to work, make sure ~/.cargo/bin is in your PATH. If the command worked, then it probably is on your PATH, so this note was a bit unnecessary actually. Oh well.

## OS Support/How it works
`portcull` is cross-platform and uses native system commands to find PIDs. On Linux/macOS, it uses lsof. On Windows, it uses netstat.

**Depending on the ports you are trying to clear, you may need to run `portcull` with admin privileges (e.g., `sudo portcull <ports>` on Linux/Mac or an elevated Command Prompt/PowerShell on Windows) so it has permission to look up and kill those processes.**

## Special Features
If `portcull` detects you are trying to kill `ollama`, it remins you that Ollama will immediately restart itself anyway (but it still *attempts* to kill it for you).

## FAQ

### Q. "Why is Claude in your Contributors?! I hate AI!!! I'm going to self destructjdiosajf90e3!(!!!)((93058743783$^#&@*!!!!"
A. I use AI for commits. I use it when a critical bug is found and I don't have the time to fix it that instant. I use Claude to create a PR and then I review it and accept the PR if it's fine.  
If you don't like the policy that you can see in [CONTRIBUTING.md](CONTRIBUTING.md), then this repo probably isn't for you. 

### Q. "Why does it say no processes are found when I know that something is using that port?"
A. Try using `sudo` on *NIX or an elevated command prompt or PowerShell on Windows. Unprivileged `lsof`/`netstat` can't see other users' sockets.  

### Q. Why is it telling me to run it with `sudo` if I'm already root?
A. I need to check user ID properly so this'll be fixed soon (hopefully) by adding `libc`.

### Q. "Does this work on Windows?"
A. Yes; it uses `netstat` instead of `lsof` on Windows.

### Q. "Why can't I kill the process running on 11434?"
A. It's probably Ollama; if it is, run `systemctl stop ollama` or quit the app on macOS/Windows.


## License
[MIT License](LICENSE)

Third-party license notices for the crates `portcull` depends on are in [CREDITS.txt](CREDITS.txt), and are embedded in the binary (run `portcull --licenses` to print them).
