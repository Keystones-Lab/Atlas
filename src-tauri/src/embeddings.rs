use serde::Deserialize;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::Mutex;

pub const DIM: usize = 256;
const WORKER: &str = include_str!("embedding_worker.py");
static PROCESS: Mutex<Option<Worker>> = Mutex::new(None);

#[derive(Deserialize)]
struct Reply {
    ready: Option<bool>,
    vector: Option<Vec<f32>>,
    error: Option<String>,
}

struct Worker {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
}

impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Worker {
    fn start() -> Result<Self, String> {
        let python = std::env::var_os("ATLAS_PYTHON").unwrap_or_else(|| {
            #[cfg(debug_assertions)]
            {
                let relative = if cfg!(windows) {
                    "../.venv/Scripts/python.exe"
                } else {
                    "../.venv/bin/python"
                };
                let local = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
                if local.is_file() {
                    return local.into_os_string();
                }
            }
            "python".into()
        });
        let mut command = Command::new(python);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = command
            .args(["-X", "utf8", "-u", "-c", WORKER])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Cannot start the embedding runtime. Check ATLAS_PYTHON: {e}"))?;
        let mut worker = Worker {
            input: child.stdin.take().ok_or("Missing embedding input")?,
            output: BufReader::new(child.stdout.take().ok_or("Missing embedding output")?),
            child,
        };
        let reply = worker.read()?;
        if reply.ready != Some(true) {
            return Err(reply
                .error
                .unwrap_or_else(|| "Embedding runtime failed to initialize".into()));
        }
        Ok(worker)
    }

    fn read(&mut self) -> Result<Reply, String> {
        let mut line = String::new();
        if self
            .output
            .read_line(&mut line)
            .map_err(|e| e.to_string())?
            == 0
        {
            return Err("The local embedding process stopped. Check the Python runtime and model installation.".into());
        }
        serde_json::from_str(&line).map_err(|e| format!("Invalid embedding response: {e}"))
    }
}

pub fn ensure_ready() -> Result<(), String> {
    let mut slot = PROCESS.lock().map_err(|e| e.to_string())?;
    if slot.is_none() {
        *slot = Some(Worker::start()?);
    }
    Ok(())
}

pub fn encode(kind: &str, value: &str, title: &str) -> Result<Vec<f32>, String> {
    let mut slot = PROCESS.lock().map_err(|e| e.to_string())?;
    if slot.is_none() {
        *slot = Some(Worker::start()?);
    }
    let worker = slot.as_mut().ok_or("Missing embedding runtime")?;
    let request = serde_json::json!({ "kind": kind, "value": value, "title": title });
    let response = writeln!(worker.input, "{request}")
        .map_err(|e| e.to_string())
        .and_then(|_| worker.read());
    let reply = match response {
        Ok(reply) => reply,
        Err(error) => {
            *slot = None;
            return Err(error);
        }
    };
    if let Some(error) = reply.error {
        return Err(error);
    }
    reply
        .vector
        .filter(|v| v.len() == DIM && v.iter().all(|x| x.is_finite()))
        .ok_or_else(|| "Invalid EmbeddingGemma 2 vector".into())
}
