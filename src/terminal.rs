use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use std::{
    collections::HashMap,
    io::{Read, Write},
    path::Path,
    sync::mpsc::SyncSender,
};

pub struct Session {
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn Child + Send + Sync>,
    writer: Box<dyn Write + Send>,
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

#[derive(Default)]
pub struct Terminals(pub HashMap<u64, Session>);

impl Terminals {
    pub fn open(
        &mut self,
        id: u64,
        cwd: &Path,
        cols: u16,
        rows: u16,
        output: SyncSender<(u64, Vec<u8>)>,
    ) -> Result<(), String> {
        let pair = native_pty_system()
            .openpty(size(cols, rows))
            .map_err(|e| e.to_string())?;
        #[cfg(target_os = "windows")]
        let mut command = {
            let system = std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into());
            let shell = std::path::PathBuf::from(system)
                .join("System32/WindowsPowerShell/v1.0/powershell.exe");
            let mut command = CommandBuilder::new(shell);
            command.args(["-NoLogo", "-NoProfile"]);
            command
        };
        #[cfg(not(target_os = "windows"))]
        let mut command =
            CommandBuilder::new(std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into()));
        command.cwd(cwd);
        let child = pair
            .slave
            .spawn_command(command)
            .map_err(|e| e.to_string())?;
        drop(pair.slave);
        let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
        let writer = pair.master.take_writer().map_err(|e| e.to_string())?;
        self.0.insert(
            id,
            Session {
                master: pair.master,
                child,
                writer,
            },
        );
        std::thread::spawn(move || {
            let mut bytes = [0u8; 4096];
            loop {
                match reader.read(&mut bytes) {
                    Ok(0) | Err(_) => break,
                    Ok(count) => {
                        if output.send((id, bytes[..count].to_vec())).is_err() {
                            break;
                        }
                    }
                }
            }
            let _ = output.send((id, b"\r\n[Session ended]\r\n".to_vec()));
        });
        Ok(())
    }

    pub fn input(&mut self, id: u64, text: &str) -> Result<(), String> {
        let session = self.0.get_mut(&id).ok_or("Terminal is closed")?;
        session
            .writer
            .write_all(text.as_bytes())
            .and_then(|_| session.writer.flush())
            .map_err(|e| e.to_string())
    }

    pub fn resize(&self, id: u64, cols: u16, rows: u16) -> Result<(), String> {
        self.0
            .get(&id)
            .ok_or("Terminal is closed")?
            .master
            .resize(size(cols, rows))
            .map_err(|e| e.to_string())
    }
}

fn size(cols: u16, rows: u16) -> PtySize {
    PtySize {
        rows: rows.clamp(2, 300),
        cols: cols.clamp(10, 500),
        pixel_width: 0,
        pixel_height: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn terminal_accepts_input_and_streams_output() {
        let (sender, receiver) = std::sync::mpsc::sync_channel(512);
        let mut sessions = Terminals::default();
        sessions
            .open(1, &std::env::temp_dir(), 80, 24, sender)
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        let mut output = String::new();
        let mut sent = false;
        while !output.contains("DADO_PTY_OK") && std::time::Instant::now() < deadline {
            if let Ok((_, bytes)) = receiver.recv_timeout(std::time::Duration::from_millis(200)) {
                let text = String::from_utf8_lossy(&bytes);
                // ConPTY queries the terminal's cursor before starting PowerShell.
                // The real xterm view answers this through its onData callback.
                if text.contains("\u{1b}[6n") {
                    sessions.input(1, "\u{1b}[1;1R").unwrap();
                }
                output.push_str(&text);
                if !sent && (output.contains("PS ") || cfg!(not(target_os = "windows"))) {
                    #[cfg(target_os = "windows")]
                    sessions
                        .input(1, "Write-Output ('DADO_' + 'PTY_OK')\r")
                        .unwrap();
                    #[cfg(not(target_os = "windows"))]
                    sessions.input(1, "printf 'DADO_%s\\n' 'PTY_OK'\r").unwrap();
                    sent = true;
                }
            }
        }
        assert!(
            output.contains("DADO_PTY_OK"),
            "PTY did not execute input: {output}"
        );
        sessions.resize(1, 100, 30).unwrap();
        sessions.0.remove(&1);
    }
}
