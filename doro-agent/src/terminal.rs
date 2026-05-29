use chrono::DateTime;
use chrono::Utc;
use portable_pty::Child;
use portable_pty::CommandBuilder;
use portable_pty::MasterPty;
use portable_pty::PtySize;
use portable_pty::native_pty_system;
use std::io::Read;
use std::io::Write;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::mpsc;
use std::time::Duration;
use std::time::Instant;

const DEFAULT_COLS: u16 = 80;
const DEFAULT_ROWS: u16 = 24;
const OUTPUT_LIMIT_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone)]
pub struct TerminalManager {
    session: Arc<Mutex<TerminalSession>>,
}

#[derive(Debug, Clone)]
pub struct TerminalCommand {
    pub command_id: String,
    pub input: String,
    pub cols: u16,
    pub rows: u16,
    pub timeout: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalCommandOutput {
    pub output: String,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
}

struct TerminalSession {
    _child: Box<dyn Child + Send>,
    master: Box<dyn MasterPty + Send>,
    output_rx: mpsc::Receiver<Vec<u8>>,
    writer: Box<dyn Write + Send>,
}

impl std::fmt::Debug for TerminalSession {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TerminalSession")
            .finish_non_exhaustive()
    }
}

impl TerminalManager {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            session: Arc::new(Mutex::new(TerminalSession::spawn(
                DEFAULT_COLS,
                DEFAULT_ROWS,
            )?)),
        })
    }

    pub async fn execute(&self, command: TerminalCommand) -> anyhow::Result<TerminalCommandOutput> {
        let session = self.session.clone();
        tokio::task::spawn_blocking(move || {
            let mut session = session
                .lock()
                .map_err(|_| anyhow::anyhow!("terminal session lock poisoned"))?;
            let result = session.execute(command);
            if result.as_ref().is_ok_and(|output| output.timed_out) {
                *session = TerminalSession::spawn(DEFAULT_COLS, DEFAULT_ROWS)?;
            }
            result
        })
        .await?
    }
}

impl TerminalSession {
    fn spawn(cols: u16, rows: u16) -> anyhow::Result<Self> {
        let pty_system = native_pty_system();
        let pair = pty_system.openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        let shell = default_shell();
        let command = CommandBuilder::new(shell);
        let child = pair.slave.spawn_command(command)?;
        let mut reader = pair.master.try_clone_reader()?;
        let mut writer = pair.master.take_writer()?;
        let (output_tx, output_rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut buffer = [0_u8; 4096];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(read) => {
                        if output_tx.send(buffer[..read].to_vec()).is_err() {
                            break;
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(_) => break,
                }
            }
        });
        #[cfg(not(windows))]
        {
            writer.write_all(b"stty -echo\n")?;
            writer.flush()?;
        }

        Ok(Self {
            _child: child,
            master: pair.master,
            output_rx,
            writer,
        })
    }

    fn execute(&mut self, command: TerminalCommand) -> anyhow::Result<TerminalCommandOutput> {
        while self.output_rx.try_recv().is_ok() {}
        self.master.resize(PtySize {
            rows: command.rows,
            cols: command.cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let started_at = Utc::now();
        let token = command.command_id.replace('-', "_");
        let sentinel_prefix = format!("__DORO_TERMINAL_DONE_{token}:");
        let script = command_script(&command.input, &sentinel_prefix);
        self.writer.write_all(script.as_bytes())?;
        self.writer.flush()?;

        let deadline = Instant::now() + command.timeout;
        let mut output = Vec::new();
        let mut exit_code = None;
        let mut timed_out = false;

        loop {
            let now = Instant::now();
            if now >= deadline {
                timed_out = true;
                break;
            }

            match self
                .output_rx
                .recv_timeout(deadline.saturating_duration_since(now))
            {
                Ok(chunk) => {
                    output.extend_from_slice(&chunk);
                    if output.len() > OUTPUT_LIMIT_BYTES {
                        let drain_to = output.len() - OUTPUT_LIMIT_BYTES;
                        output.drain(..drain_to);
                    }
                    let text = String::from_utf8_lossy(&output);
                    if let Some(code) = parse_exit_code(&text, &sentinel_prefix) {
                        exit_code = Some(code);
                        break;
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    timed_out = true;
                    break;
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    timed_out = true;
                    break;
                }
            }
        }

        let finished_at = Utc::now();
        let output = strip_sentinel(
            String::from_utf8_lossy(&output).into_owned(),
            &sentinel_prefix,
        );

        Ok(TerminalCommandOutput {
            output,
            exit_code,
            timed_out,
            started_at,
            finished_at,
        })
    }
}

fn default_shell() -> String {
    #[cfg(windows)]
    {
        "powershell.exe".to_string()
    }
    #[cfg(not(windows))]
    {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string())
    }
}

fn command_script(input: &str, sentinel_prefix: &str) -> String {
    #[cfg(windows)]
    {
        format!(
            "\r\n{}\r\nWrite-Output \"{}$global:LASTEXITCODE\"\r\n",
            input, sentinel_prefix
        )
    }
    #[cfg(not(windows))]
    {
        format!("\n{}\nprintf '\\n{}%s\\n' \"$?\"\n", input, sentinel_prefix)
    }
}

fn parse_exit_code(output: &str, sentinel_prefix: &str) -> Option<i32> {
    output
        .lines()
        .find_map(|line| line.trim().strip_prefix(sentinel_prefix))
        .and_then(|value| value.trim().parse().ok())
}

fn strip_sentinel(output: String, sentinel_prefix: &str) -> String {
    output
        .lines()
        .filter(|line| !line.trim().starts_with(sentinel_prefix))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_exit_code_from_sentinel_line() {
        let output = "hello\n__DORO_TERMINAL_DONE_abc:17\n";
        assert_eq!(
            parse_exit_code(output, "__DORO_TERMINAL_DONE_abc:"),
            Some(17)
        );
        assert_eq!(
            strip_sentinel(output.to_string(), "__DORO_TERMINAL_DONE_abc:"),
            "hello"
        );
    }

    #[tokio::test]
    async fn terminal_command_returns_output_and_exit_code() {
        let terminal = TerminalManager::new().expect("terminal should start");
        let output = terminal
            .execute(TerminalCommand {
                command_id: "test-command".to_string(),
                input: "printf doro".to_string(),
                cols: DEFAULT_COLS,
                rows: DEFAULT_ROWS,
                timeout: Duration::from_secs(5),
            })
            .await
            .expect("command should run");

        assert!(output.output.contains("doro"));
        assert_eq!(output.exit_code, Some(0), "{output:?}");
        assert!(!output.timed_out);
    }

    #[tokio::test]
    async fn terminal_command_reports_non_zero_exit_code() {
        let terminal = TerminalManager::new().expect("terminal should start");
        let output = terminal
            .execute(TerminalCommand {
                command_id: "test-false".to_string(),
                input: "false".to_string(),
                cols: DEFAULT_COLS,
                rows: DEFAULT_ROWS,
                timeout: Duration::from_secs(5),
            })
            .await
            .expect("command should run");

        assert_eq!(output.exit_code, Some(1), "{output:?}");
        assert!(!output.timed_out);
    }

    #[tokio::test]
    async fn terminal_command_times_out_and_recovers() {
        let terminal = TerminalManager::new().expect("terminal should start");
        let timed_out = terminal
            .execute(TerminalCommand {
                command_id: "test-timeout".to_string(),
                input: "sleep 1".to_string(),
                cols: DEFAULT_COLS,
                rows: DEFAULT_ROWS,
                timeout: Duration::from_millis(100),
            })
            .await
            .expect("command should return timeout output");

        assert!(timed_out.timed_out);
        assert_eq!(timed_out.exit_code, None);

        let recovered = terminal
            .execute(TerminalCommand {
                command_id: "test-recovered".to_string(),
                input: "printf recovered".to_string(),
                cols: DEFAULT_COLS,
                rows: DEFAULT_ROWS,
                timeout: Duration::from_secs(5),
            })
            .await
            .expect("terminal should recover after timeout");

        assert!(recovered.output.contains("recovered"));
        assert_eq!(recovered.exit_code, Some(0), "{recovered:?}");
    }
}
