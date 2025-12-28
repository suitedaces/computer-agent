use std::process::{Command, Stdio};
use std::time::Duration;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum BashError {
    #[error("Command blocked: {0}")]
    Blocked(String),
    #[error("Execution failed: {0}")]
    Execution(String),
    #[error("Timeout after {0} seconds")]
    Timeout(u64),
}

// dangerous commands/patterns to block
const BLOCKED_PATTERNS: &[&str] = &[
    // destructive
    "rm -rf /",
    "rm -rf /*",
    "rm -rf ~",
    "rm -rf $HOME",
    ":(){:|:&};:",  // fork bomb
    "mkfs",
    "dd if=",
    "> /dev/sd",
    "chmod -R 777 /",

    // system modification
    "sudo rm",
    "sudo mkfs",
    "sudo dd",

    // network attacks
    "nc -l",  // netcat listener
    "nmap",

    // credential theft
    "curl.*|.*sh",
    "wget.*|.*sh",

    // disable security
    "csrutil disable",
    "SIP",
];

// commands that need extra caution
const WARN_PATTERNS: &[&str] = &[
    "sudo",
    "rm -rf",
    "chmod",
    "chown",
    "kill -9",
    "pkill",
    "shutdown",
    "reboot",
];

pub struct BashExecutor {
    timeout_secs: u64,
    working_dir: Option<String>,
}

impl BashExecutor {
    pub fn new() -> Self {
        Self {
            timeout_secs: 30,
            working_dir: None,
        }
    }

    pub fn set_working_dir(&mut self, dir: String) {
        self.working_dir = Some(dir);
    }

    fn is_blocked(&self, command: &str) -> Option<String> {
        let cmd_lower = command.to_lowercase();

        for pattern in BLOCKED_PATTERNS {
            if cmd_lower.contains(&pattern.to_lowercase()) {
                return Some(format!("Command contains blocked pattern: {}", pattern));
            }
        }
        None
    }

    fn has_warning(&self, command: &str) -> Option<String> {
        let cmd_lower = command.to_lowercase();

        for pattern in WARN_PATTERNS {
            if cmd_lower.contains(&pattern.to_lowercase()) {
                return Some(format!("⚠️ Command uses: {}", pattern));
            }
        }
        None
    }

    pub fn execute(&self, command: &str) -> Result<BashOutput, BashError> {
        // check for blocked commands
        if let Some(reason) = self.is_blocked(command) {
            return Err(BashError::Blocked(reason));
        }

        // log warning if applicable
        if let Some(warning) = self.has_warning(command) {
            println!("[bash] {}", warning);
        }

        println!("[bash] Executing: {}", command);

        let mut cmd = Command::new("bash");
        cmd.arg("-c")
            .arg(command)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if let Some(ref dir) = self.working_dir {
            cmd.current_dir(dir);
        }

        let output = cmd
            .output()
            .map_err(|e| BashError::Execution(e.to_string()))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code().unwrap_or(-1);

        // truncate long outputs
        let stdout = truncate_output(&stdout, 5000);
        let stderr = truncate_output(&stderr, 2000);

        Ok(BashOutput {
            stdout,
            stderr,
            exit_code,
        })
    }

    pub fn restart(&mut self) {
        self.working_dir = None;
        println!("[bash] Session restarted");
    }
}

#[derive(Debug, Clone)]
pub struct BashOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

impl BashOutput {
    pub fn to_string(&self) -> String {
        let mut result = String::new();

        if !self.stdout.is_empty() {
            result.push_str(&self.stdout);
        }

        if !self.stderr.is_empty() {
            if !result.is_empty() {
                result.push_str("\n");
            }
            result.push_str("stderr: ");
            result.push_str(&self.stderr);
        }

        if self.exit_code != 0 {
            if !result.is_empty() {
                result.push_str("\n");
            }
            result.push_str(&format!("(exit code: {})", self.exit_code));
        }

        if result.is_empty() {
            result = "(no output)".to_string();
        }

        result
    }
}

fn truncate_output(s: &str, max_chars: usize) -> String {
    if s.len() <= max_chars {
        s.to_string()
    } else {
        format!(
            "{}...\n[truncated, {} total chars]",
            &s[..max_chars],
            s.len()
        )
    }
}
