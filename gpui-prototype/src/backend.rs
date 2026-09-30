use std::{
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
        Arc, Mutex,
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Debug)]
pub struct ConfigStore {
    config_path: PathBuf,
    backup_dir: PathBuf,
}

impl ConfigStore {
    pub fn default_location() -> Self {
        let root = if let Some(path) = std::env::var_os("ECR_DATA_DIR") {
            PathBuf::from(path)
        } else {
            #[cfg(target_os = "macos")]
            let root = std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(std::env::temp_dir)
                .join("Library/Application Support/SleepyKanata/EasyCommandRunner");
            #[cfg(target_os = "windows")]
            let root = std::env::var_os("APPDATA")
                .map(PathBuf::from)
                .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))
                .unwrap_or_else(std::env::temp_dir)
                .join("SleepyKanata/EasyCommandRunner");
            #[cfg(target_os = "linux")]
            let root = std::env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .or_else(|| {
                    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config"))
                })
                .unwrap_or_else(std::env::temp_dir)
                .join("easy-command-runner");
            #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
            let root = std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(std::env::temp_dir)
                .join(".config/easy-command-runner");
            root
        };
        Self::at(root.join("config.json"), root.join("backup"))
    }

    pub fn at(config_path: impl Into<PathBuf>, backup_dir: impl Into<PathBuf>) -> Self {
        Self {
            config_path: config_path.into(),
            backup_dir: backup_dir.into(),
        }
    }

    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    fn session_path(&self) -> PathBuf {
        self.config_path.with_extension("gpui-session.json")
    }

    pub fn has_tab_session(&self) -> bool {
        self.session_path().exists()
    }

    /// A separate, versioned view-only file remembers open tabs without saving unsaved commands
    /// or churning the configuration backup history. A changed config invalidates stale indices.
    pub fn config_digest(&self) -> Option<u64> {
        let bytes = fs::read(&self.config_path).ok()?;
        Some(bytes.iter().fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
        }))
    }

    pub fn load_tab_session(&self) -> Option<(Vec<usize>, usize)> {
        let digest = self.config_digest()?;
        let state: serde_json::Value =
            serde_json::from_slice(&fs::read(self.session_path()).ok()?).ok()?;
        if state["version"] != 1 || state["digest"].as_u64() != Some(digest) {
            return None;
        }
        let open = state["open_tabs"]
            .as_array()?
            .iter()
            .map(|index| index.as_u64().map(|index| index as usize))
            .collect::<Option<Vec<_>>>()?;
        Some((open, state["active"].as_u64()? as usize))
    }

    pub fn save_tab_session(
        &self,
        expected_digest: Option<u64>,
        open: &[usize],
        active: usize,
    ) -> Result<(), String> {
        let Some(digest) = expected_digest.filter(|digest| Some(*digest) == self.config_digest())
        else {
            // No saved file, or Qt/another process replaced it: never apply stale indices.
            return Ok(());
        };
        let value =
            serde_json::json!({"version":1,"digest":digest,"open_tabs":open,"active":active});
        write_json_atomically(
            &self.session_path(),
            &serde_json::to_string(&value).unwrap(),
        )
    }

    /// `None` means a fresh profile. Existing malformed data is an error and must not be overwritten.
    pub fn load(&self) -> Result<Option<serde_json::Value>, String> {
        if !self.config_path.exists() {
            return Ok(None);
        }
        let text = fs::read_to_string(&self.config_path)
            .map_err(|error| format!("读取配置失败（{}）：{error}", self.config_path.display()))?;
        let value: serde_json::Value = serde_json::from_str(&text).map_err(|error| {
            format!("配置 JSON 无效（{}）：{error}", self.config_path.display())
        })?;
        if !value.is_object() {
            return Err("配置根节点必须是 JSON 对象；为保护原文件，已禁止覆盖。".into());
        }
        normalize_qt_configuration(value).map(Some)
    }

    pub fn save(&self, value: &serde_json::Value) -> Result<(), String> {
        if !value.is_object() {
            return Err("配置根节点必须是 JSON 对象。".into());
        }
        let serialized = serde_json::to_string_pretty(value)
            .map_err(|error| format!("序列化配置失败：{error}"))?;

        if self.config_path.exists() {
            let previous = fs::read_to_string(&self.config_path)
                .map_err(|error| format!("读取旧配置失败，为保护原文件已取消保存：{error}"))?;
            let previous_value: serde_json::Value = serde_json::from_str(&previous)
                .map_err(|error| format!("旧配置 JSON 无效，为保护原文件已取消保存：{error}"))?;
            if previous_value != *value {
                fs::create_dir_all(&self.backup_dir)
                    .map_err(|error| format!("创建备份目录失败：{error}"))?;
                let stamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos();
                let backup = self.backup_dir.join(format!("config_{stamp}.json"));
                fs::write(&backup, previous)
                    .map_err(|error| format!("备份旧配置失败，为保护原文件已取消保存：{error}"))?;
                self.prune_backups()?;
            }
        }

        self.write_atomically(&serialized)
    }

    fn write_atomically(&self, serialized: &str) -> Result<(), String> {
        write_json_atomically(&self.config_path, serialized)
    }

    /// Importing a backup never changes the active configuration or the source file.
    pub fn import_backup(&self, source: &Path) -> Result<String, String> {
        let value = read_configuration(source)?;
        fs::create_dir_all(&self.backup_dir).map_err(|e| format!("创建备份目录失败：{e}"))?;
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let name = format!("imported_{stamp}.json");
        write_json_atomically(
            &self.backup_dir.join(&name),
            &serde_json::to_string_pretty(&value).unwrap(),
        )?;
        Ok(name)
    }

    pub fn read_backup(&self, name: &str) -> Result<serde_json::Value, String> {
        let path = Path::new(name);
        if path.file_name().and_then(|name| name.to_str()) != Some(name)
            || !path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
        {
            return Err("备份文件名无效。".into());
        }
        let path = self.backup_dir.join(name);
        if !fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.file_type().is_file()) {
            return Err("备份不存在或不是普通文件。".into());
        }
        read_configuration(&path)
    }

    pub fn export_configuration(
        &self,
        path: &Path,
        value: &serde_json::Value,
    ) -> Result<(), String> {
        // Never bypass the protection of the active file, even through an alias.
        if path == self.config_path
            || (path.exists()
                && fs::canonicalize(path).ok() == fs::canonicalize(&self.config_path).ok())
        {
            return Err("请使用其他文件名导出配置。".into());
        }
        let value = normalize_qt_configuration(value.clone())?;
        if path.exists() {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let safety = path.with_extension(format!("json.backup_{stamp}"));
            fs::copy(path, safety).map_err(|e| format!("备份目标文件失败，已取消导出：{e}"))?;
        }
        write_json_atomically(path, &serde_json::to_string_pretty(&value).unwrap())
    }

    /// Persist only appearance preferences; unsaved command edits must stay unsaved.
    pub fn save_preferences(&self, preferences: &serde_json::Value) -> Result<(), String> {
        let mut config = self
            .load()?
            .unwrap_or_else(|| serde_json::json!({"tabs":[]}));
        config["theme"] = preferences["theme"].clone();
        if !config["gpui"].is_object() {
            config["gpui"] = serde_json::json!({});
        }
        for key in [
            "accent",
            "font_size",
            "font_weight",
            "language",
            "reduced_motion",
        ] {
            config["gpui"][key] = preferences["gpui"][key].clone();
        }
        self.save(&config)
    }

    pub fn backups(&self) -> Result<Vec<String>, String> {
        if !self.backup_dir.exists() {
            return Ok(Vec::new());
        }
        let entries =
            fs::read_dir(&self.backup_dir).map_err(|error| format!("读取备份目录失败：{error}"))?;
        let mut names = entries
            .flatten()
            .filter_map(|entry| {
                let name = entry.file_name().into_string().ok()?;
                (entry.file_type().is_ok_and(|kind| kind.is_file())
                    && entry
                        .path()
                        .extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("json")))
                .then_some(name)
            })
            .collect::<Vec<_>>();
        names.sort_by(|left, right| right.cmp(left));
        Ok(names)
    }

    /// Validate the backup name, retain a safety backup of the current config, then restore atomically.
    pub fn restore_backup(&self, name: &str) -> Result<serde_json::Value, String> {
        let value = self.read_backup(name)?;
        self.replace_configuration(&value)?;
        Ok(value)
    }

    /// Explicit import/restore: preserve even corrupt bytes before replacing the active file.
    pub fn replace_configuration(&self, value: &serde_json::Value) -> Result<(), String> {
        let value = normalize_qt_configuration(value.clone())?;
        if self.config_path.exists() {
            let current = fs::read(&self.config_path)
                .map_err(|error| format!("备份当前配置失败，未执行恢复：{error}"))?;
            fs::create_dir_all(&self.backup_dir)
                .map_err(|error| format!("创建安全备份目录失败：{error}"))?;
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let safety = self.backup_dir.join(format!("config_{stamp}.json"));
            fs::write(safety, current)
                .map_err(|error| format!("备份当前配置失败，未执行恢复：{error}"))?;
            self.prune_backups()?;
        }
        let serialized = serde_json::to_string_pretty(&value)
            .map_err(|error| format!("序列化备份失败：{error}"))?;
        self.write_atomically(&serialized)
    }

    fn prune_backups(&self) -> Result<(), String> {
        let Ok(entries) = fs::read_dir(&self.backup_dir) else {
            return Ok(());
        };
        let mut backups = entries
            .flatten()
            .filter(|entry| {
                entry.file_name().to_string_lossy().starts_with("config_")
                    && entry
                        .path()
                        .extension()
                        .is_some_and(|extension| extension == "json")
            })
            .collect::<Vec<_>>();
        backups.sort_by_key(|entry| {
            entry
                .metadata()
                .and_then(|metadata| metadata.modified())
                .ok()
        });
        for entry in backups.iter().take(backups.len().saturating_sub(10)) {
            fs::remove_file(entry.path()).map_err(|error| format!("清理旧备份失败：{error}"))?;
        }
        Ok(())
    }
}

pub fn read_configuration(path: &Path) -> Result<serde_json::Value, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("读取配置失败：{e}"))?;
    let value = serde_json::from_str(&text).map_err(|e| format!("配置 JSON 无效：{e}"))?;
    normalize_qt_configuration(value)
}

fn write_json_atomically(path: &Path, serialized: &str) -> Result<(), String> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|e| format!("创建配置目录失败：{e}"))?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temporary = parent.join(format!(".config-{}-{stamp}.tmp", std::process::id()));
    let result = (|| {
        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(serialized.as_bytes())?;
        file.sync_all()?;
        drop(file);
        // std::fs::rename replaces an existing file on Windows as well; never unlink first.
        fs::rename(&temporary, path)
    })();
    if let Err(error) = result {
        let _ = fs::remove_file(&temporary);
        return Err(format!("保存配置失败（{}）：{error}", path.display()));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputStream {
    Stdout,
    Stderr,
}

#[derive(Debug)]
pub enum RunEvent {
    Output { stream: OutputStream, text: String },
    Finished { exit_code: i32, stopped: bool },
}

#[cfg(windows)]
fn hide_windows_console(command: &mut Command) {
    use std::os::windows::process::CommandExt;

    // CREATE_NO_WINDOW: keep GUI applications and their helper processes silent.
    command.creation_flags(0x0800_0000);
}

#[cfg(any(windows, test))]
fn windows_shell_arguments(shell: &str, command: &str) -> String {
    // Match Qt: set the console code page in an outer cmd, then let an inner cmd
    // parse the user's command. This preserves quotes around paths with spaces.
    format!(r#"/D /S /C "chcp 65001>nul & "{shell}" /D /S /C "{command}"""#)
}

#[derive(Clone)]
pub struct RunHandle {
    child: Arc<Mutex<Child>>,
    stopped: Arc<AtomicBool>,
}

impl RunHandle {
    pub fn stop(&self) -> Result<(), String> {
        let child = self
            .child
            .lock()
            .map_err(|_| "进程控制锁损坏".to_string())?;
        #[cfg(windows)]
        let mut child = child; // Child::kill needs &mut self only on the Windows fallback.
        #[cfg(unix)]
        {
            // 启动时为 Shell 建立独立进程组；停止管道/脚本时连同 Shell 子进程一起结束。
            let result = unsafe { libc::kill(-(child.id() as libc::pid_t), libc::SIGKILL) };
            if result == 0 {
                self.stopped.store(true, Ordering::SeqCst);
                return Ok(());
            }
            if io::Error::last_os_error().kind() == io::ErrorKind::NotFound {
                return Ok(());
            }
            return Err(format!("停止进程组失败：{}", io::Error::last_os_error()));
        }
        #[cfg(windows)]
        {
            let pid = child.id().to_string();
            let taskkill = std::env::var_os("SystemRoot")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(r"C:\Windows"))
                .join("System32/taskkill.exe");
            let mut taskkill_command = Command::new(taskkill);
            hide_windows_console(&mut taskkill_command);
            match taskkill_command
                .args(["/PID", &pid, "/T", "/F"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
            {
                Ok(_taskkill) => {
                    self.stopped.store(true, Ordering::SeqCst);
                    Ok(())
                }
                Err(error) => {
                    let _ = child.kill();
                    Err(format!("终止命令进程树失败：{error}"))
                }
            }
        }
    }
}

/// Launch the platform shell, stream both pipes, and reap the child on a worker thread.
pub fn start_shell(
    command: &str,
    working_dir: &str,
) -> Result<(RunHandle, Receiver<RunEvent>), String> {
    if command.trim().is_empty() {
        return Err("命令为空，请先填写程序或参数。".into());
    }
    let current_dir = if working_dir.trim().is_empty() {
        std::env::current_dir().map_err(|error| format!("获取当前目录失败：{error}"))?
    } else {
        let path = PathBuf::from(working_dir);
        if !path.is_dir() {
            return Err(format!("工作目录不存在或不是目录：{}", path.display()));
        }
        path
    };

    #[cfg(windows)]
    let mut child = {
        use std::os::windows::process::CommandExt;

        let shell = std::env::var_os("COMSPEC").unwrap_or_else(|| "cmd.exe".into());
        let shell_text = shell.to_string_lossy().into_owned();
        let mut process = Command::new(shell);
        hide_windows_console(&mut process);
        // Use the same nested cmd/code-page wrapper as Qt; raw_arg is required so
        // Rust's CRT argument quoting does not alter cmd's own quote semantics.
        process
            .raw_arg(windows_shell_arguments(&shell_text, command))
            // Match Qt: Python tools write UTF-8 to the redirected stdout/stderr pipes.
            .env("PYTHONIOENCODING", "utf-8")
            .env("PYTHONUNBUFFERED", "1")
            .current_dir(current_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("启动命令失败：{error}"))?
    };
    #[cfg(not(windows))]
    let mut child = {
        use std::os::unix::process::CommandExt;
        let shell = std::env::var_os("SHELL").unwrap_or_else(|| "/bin/sh".into());
        Command::new(shell)
            .arg("-c")
            .arg(command)
            .current_dir(current_dir)
            .process_group(0)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("启动命令失败：{error}"))?
    };

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "无法读取标准输出管道".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "无法读取标准错误管道".to_string())?;
    let child = Arc::new(Mutex::new(child));
    let stopped = Arc::new(AtomicBool::new(false));
    let (sender, receiver) = mpsc::channel();
    let stdout_reader = match spawn_reader(stdout, OutputStream::Stdout, sender.clone()) {
        Ok(reader) => reader,
        Err(error) => {
            let _ = child.lock().map(|mut child| child.kill());
            return Err(format!("创建标准输出读取线程失败：{error}"));
        }
    };
    let stderr_reader = match spawn_reader(stderr, OutputStream::Stderr, sender.clone()) {
        Ok(reader) => reader,
        Err(error) => {
            let _ = child.lock().map(|mut child| child.kill());
            return Err(format!("创建标准错误读取线程失败：{error}"));
        }
    };

    let monitor_child = child.clone();
    let monitor_stopped = stopped.clone();
    let monitor = thread::Builder::new()
        .name("ecr-process-monitor".into())
        .spawn(move || {
            let exit_code = loop {
                let status = match monitor_child.lock() {
                    Ok(mut child) => child.try_wait(),
                    Err(_) => break -1,
                };
                match status {
                    Ok(Some(status)) => break status.code().unwrap_or(-1),
                    Ok(None) => thread::sleep(Duration::from_millis(20)),
                    Err(_) => break -1,
                }
            };
            let _ = stdout_reader.join();
            let _ = stderr_reader.join();
            let _ = sender.send(RunEvent::Finished {
                exit_code,
                stopped: monitor_stopped.load(Ordering::SeqCst),
            });
        });
    if let Err(error) = monitor {
        let _ = (RunHandle {
            child: child.clone(),
            stopped: stopped.clone(),
        })
        .stop();
        return Err(format!("启动进程监控失败：{error}"));
    }

    Ok((RunHandle { child, stopped }, receiver))
}

/// Preserve incomplete UTF-8 code points between pipe reads without retaining completed output.
#[derive(Default)]
struct Utf8StreamDecoder {
    pending: Vec<u8>,
}

impl Utf8StreamDecoder {
    fn push(&mut self, bytes: &[u8]) -> String {
        self.pending.extend_from_slice(bytes);
        let mut text = String::new();
        let mut consumed = 0;
        while consumed < self.pending.len() {
            match std::str::from_utf8(&self.pending[consumed..]) {
                Ok(valid) => {
                    text.push_str(valid);
                    consumed = self.pending.len();
                }
                Err(error) => {
                    let valid_end = consumed + error.valid_up_to();
                    text.push_str(std::str::from_utf8(&self.pending[consumed..valid_end]).unwrap());
                    consumed = valid_end;
                    if let Some(invalid_len) = error.error_len() {
                        text.push('\u{fffd}');
                        consumed += invalid_len;
                    } else {
                        break; // The next read may complete this multi-byte code point.
                    }
                }
            }
        }
        self.pending.drain(..consumed);
        text
    }

    fn finish(&mut self) -> String {
        let remaining = String::from_utf8_lossy(&self.pending).into_owned();
        self.pending.clear();
        remaining
    }
}

fn spawn_reader<R: Read + Send + 'static>(
    mut reader: R,
    stream: OutputStream,
    sender: Sender<RunEvent>,
) -> io::Result<thread::JoinHandle<()>> {
    thread::Builder::new()
        .name(
            match stream {
                OutputStream::Stdout => "ecr-stdout-reader",
                OutputStream::Stderr => "ecr-stderr-reader",
            }
            .into(),
        )
        .spawn(move || {
            let mut buffer = [0u8; 8192];
            let mut decoder = Utf8StreamDecoder::default();
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(count) => {
                        let text = decoder.push(&buffer[..count]);
                        if !text.is_empty()
                            && sender.send(RunEvent::Output { stream, text }).is_err()
                        {
                            return;
                        }
                    }
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    Err(_) => break,
                }
            }
            let text = decoder.finish();
            if !text.is_empty() {
                let _ = sender.send(RunEvent::Output { stream, text });
            }
        })
}

/// Apply the same PyQt5 legacy migration as Qt, then reject malformed typed fields before they can be saved over.
fn normalize_qt_configuration(mut config: serde_json::Value) -> Result<serde_json::Value, String> {
    if !config.is_object() {
        return Err("配置根节点必须是 JSON 对象。".into());
    }
    let tabs = config
        .get("tabs")
        .cloned()
        .unwrap_or_else(|| serde_json::json!([]));
    let tabs = tabs
        .as_array()
        .ok_or_else(|| "配置中的 tabs 必须是数组；为保护原文件，已禁止覆盖。".to_string())?;
    let legacy_rows = config
        .get("line_codes")
        .and_then(serde_json::Value::as_array);
    let legacy_checks = config
        .get("checkbox_statuses")
        .and_then(serde_json::Value::as_array);
    let mut normalized = Vec::with_capacity(tabs.len());

    for (tab_index, value) in tabs.iter().enumerate() {
        let mut tab = value.as_object().cloned().ok_or_else(|| {
            format!(
                "配置中的第 {} 个标签不是对象；为保护原文件，已禁止覆盖。",
                tab_index + 1
            )
        })?;
        if tab.contains_key("name_edit2") {
            let checks = legacy_checks
                .and_then(|items| items.get(tab_index))
                .and_then(serde_json::Value::as_object);
            let mut rows = Vec::new();
            rows.push(serde_json::json!({
                "function": tab.get("name_edit3_1").and_then(serde_json::Value::as_str).unwrap_or(""),
                "parameter": tab.get("name_edit3_2").and_then(serde_json::Value::as_str).unwrap_or(""),
                "comment": tab.get("name_edit3_3").and_then(serde_json::Value::as_str).unwrap_or(""),
                "enabled": checks.and_then(|items| items.get("chkbox1")).and_then(serde_json::Value::as_bool).unwrap_or(true),
            }));
            let mut codes = legacy_rows
                .and_then(|items| items.get(tab_index))
                .and_then(serde_json::Value::as_object)
                .map(|items| items.keys().cloned().collect::<Vec<_>>())
                .unwrap_or_default();
            codes.sort_by_key(|code| code.parse::<i64>().unwrap_or(i64::MAX));
            for code in codes {
                rows.push(serde_json::json!({
                    "function": tab.get(&format!("function{code}")).and_then(serde_json::Value::as_str).unwrap_or(""),
                    "parameter": tab.get(&format!("parameter{code}")).and_then(serde_json::Value::as_str).unwrap_or(""),
                    "comment": tab.get(&format!("comment{code}")).and_then(serde_json::Value::as_str).unwrap_or(""),
                    "enabled": checks.and_then(|items| items.get(&format!("chkbox{code}"))).and_then(serde_json::Value::as_bool).unwrap_or(true),
                }));
            }
            tab = serde_json::Map::from_iter([
                (
                    "name".into(),
                    tab.get("name_edit_title")
                        .cloned()
                        .unwrap_or_else(|| serde_json::json!("")),
                ),
                (
                    "working_dir".into(),
                    tab.get("name_edit1")
                        .cloned()
                        .unwrap_or_else(|| serde_json::json!("")),
                ),
                (
                    "program".into(),
                    tab.get("name_edit2")
                        .cloned()
                        .unwrap_or_else(|| serde_json::json!("")),
                ),
                (
                    "other_args".into(),
                    tab.get("name_editOther")
                        .cloned()
                        .unwrap_or_else(|| serde_json::json!("")),
                ),
                (
                    "description".into(),
                    tab.get("editDescription")
                        .cloned()
                        .unwrap_or_else(|| serde_json::json!("")),
                ),
                ("functions".into(), serde_json::Value::Array(rows)),
            ]);
        }
        for key in [
            "name",
            "working_dir",
            "program",
            "other_args",
            "description",
        ] {
            if tab.get(key).is_some_and(|value| !value.is_string()) {
                return Err(format!(
                    "标签 {} 的 {key} 字段类型错误；为保护原文件，已禁止覆盖。",
                    tab_index + 1
                ));
            }
        }
        if let Some(rows) = tab.get("functions") {
            let rows = rows
                .as_array()
                .ok_or_else(|| format!("标签 {} 的 functions 必须是数组。", tab_index + 1))?;
            for (row_index, row) in rows.iter().enumerate() {
                let row = row.as_object().ok_or_else(|| {
                    format!(
                        "标签 {} 的参数行 {} 不是对象。",
                        tab_index + 1,
                        row_index + 1
                    )
                })?;
                for key in ["function", "parameter", "comment"] {
                    if row.get(key).is_some_and(|value| !value.is_string()) {
                        return Err(format!(
                            "标签 {} 的参数行 {} 字段类型错误。",
                            tab_index + 1,
                            row_index + 1
                        ));
                    }
                }
                if row.get("enabled").is_some_and(|value| !value.is_boolean()) {
                    return Err(format!(
                        "标签 {} 的参数行 {} enabled 必须是布尔值。",
                        tab_index + 1,
                        row_index + 1
                    ));
                }
            }
        }
        normalized.push(serde_json::Value::Object(tab));
    }
    config["tabs"] = serde_json::Value::Array(normalized);
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(any(unix, windows))]
    use std::time::Instant;

    struct TestDir(PathBuf);
    static NEXT_TEMP_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    impl TestDir {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "ecr-gpui-backend-{}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn windows_shell_wrapper_preserves_quoted_unicode_paths() {
        let shell = r"C:\Windows\System32\cmd.exe";
        let command = r#"python E:\code-repository\ass_translate.py "H:\虹虹\声优个人相关\Lumina Charis 1-147\Lumina Charis_9995_0110.ass" --gemini --no-confirm"#;
        let arguments = windows_shell_arguments(shell, command);
        let expected = [
            "/D /S /C \"chcp 65001>nul & \"",
            shell,
            "\" /D /S /C \"",
            command,
            "\"\"",
        ]
        .concat();
        assert_eq!(arguments, expected);
        assert!(arguments
            .contains(r#""H:\虹虹\声优个人相关\Lumina Charis 1-147\Lumina Charis_9995_0110.ass""#));
    }

    #[test]
    fn import_export_and_preferences_preserve_active_commands() {
        let dir = TestDir::new();
        let path = dir.0.join("config.json");
        let store = ConfigStore::at(&path, dir.0.join("backup"));
        let config = serde_json::json!({"tabs":[{"name":"saved command","program":"echo safe"}],"gpui":{"show_log":true,"font_size":14}});
        store.save(&config).unwrap();
        let source = dir.0.join("external.json");
        fs::write(&source, r#"{"tabs":[{"name":"external"}]}"#).unwrap();
        let original = fs::read(&source).unwrap();
        let name = store.import_backup(&source).unwrap();
        let second = store.import_backup(&source).unwrap();
        assert_ne!(name, second);
        assert_eq!(fs::read(&source).unwrap(), original);
        assert_eq!(store.load().unwrap().unwrap(), config);
        assert_eq!(
            store.read_backup(&name).unwrap()["tabs"][0]["name"],
            "external"
        );
        let mut preferences = config.clone();
        preferences["tabs"][0]["program"] = "UNSAVED".into();
        preferences["theme"] = "light".into();
        preferences["gpui"]["font_size"] = 24.into();
        preferences["gpui"]["font_weight"] = 800.into();
        preferences["gpui"]["accent"] = "teal".into();
        preferences["gpui"]["language"] = "en_US".into();
        preferences["gpui"]["reduced_motion"] = true.into();
        store.save_preferences(&preferences).unwrap();
        let saved = store.load().unwrap().unwrap();
        assert_eq!(saved["tabs"][0]["program"], "echo safe");
        assert_eq!(saved["gpui"]["show_log"], true);
        assert_eq!(saved["gpui"]["font_size"], 24);
        let output = dir.0.join("export.json");
        fs::write(&output, b"old export").unwrap();
        store.export_configuration(&output, &saved).unwrap();
        assert_eq!(read_configuration(&output).unwrap(), saved);
        assert!(fs::read_dir(&dir.0).unwrap().flatten().any(|e| e
            .file_name()
            .to_string_lossy()
            .starts_with("export.json.backup_")
            && fs::read(e.path()).unwrap() == b"old export"));
        assert!(store.export_configuration(&path, &preferences).is_err());
        assert_eq!(store.load().unwrap().unwrap(), saved);
        // Qt imports use arbitrary filenames rather than config_* names.
        fs::write(dir.0.join("backup/Qt imported.JSON"), original).unwrap();
        assert!(store
            .backups()
            .unwrap()
            .contains(&"Qt imported.JSON".to_string()));
        assert!(store.restore_backup("Qt imported.JSON").is_ok());
    }

    #[test]
    fn invalid_imports_and_failed_exports_do_not_damage_current_files() {
        let dir = TestDir::new();
        let store = ConfigStore::at(dir.0.join("config.json"), dir.0.join("backup"));
        store
            .save(&serde_json::json!({"tabs":[{"name":"keep"}]}))
            .unwrap();
        let before = fs::read(store.config_path()).unwrap();
        let source = dir.0.join("import.json");
        for content in [
            "[]",
            "null",
            "broken",
            r#"{"tabs":"wrong"}"#,
            r#"{"tabs":[{"functions":42}]}"#,
        ] {
            fs::write(&source, content).unwrap();
            assert!(store.import_backup(&source).is_err());
            assert_eq!(fs::read(store.config_path()).unwrap(), before);
            assert!(store.backups().unwrap().is_empty());
        }
        let file_parent = dir.0.join("not-a-directory");
        fs::write(&file_parent, "keep").unwrap();
        assert!(store
            .export_configuration(&file_parent.join("out.json"), &serde_json::json!({}))
            .is_err());
        assert_eq!(fs::read_to_string(file_parent).unwrap(), "keep");
    }

    #[test]
    fn config_store_round_trips_and_backs_up_changed_files() {
        let dir = TestDir::new();
        let store = ConfigStore::at(dir.0.join("config.json"), dir.0.join("backup"));
        assert!(store.load().unwrap().is_none());
        let first = serde_json::json!({"tabs":[{"name":"one"}],"current_tab_index":0});
        store.save(&first).unwrap();
        assert_eq!(store.load().unwrap(), Some(first.clone()));
        let second = serde_json::json!({"tabs":[{"name":"two"}],"current_tab_index":0});
        store.save(&second).unwrap();
        assert_eq!(store.load().unwrap(), Some(second));
        assert_eq!(fs::read_dir(dir.0.join("backup")).unwrap().count(), 1);
    }

    #[test]
    fn restore_backup_saves_a_safety_copy_and_rejects_path_traversal() {
        let dir = TestDir::new();
        let store = ConfigStore::at(dir.0.join("config.json"), dir.0.join("backup"));
        store
            .save(&serde_json::json!({"tabs":[{"name":"old"}]}))
            .unwrap();
        store
            .save(&serde_json::json!({"tabs":[{"name":"new"}]}))
            .unwrap();
        let backup = store.backups().unwrap().remove(0);
        assert!(store.restore_backup("../config.json").is_err());
        let restored = store.restore_backup(&backup).unwrap();
        assert_eq!(restored["tabs"][0]["name"], "old");
        assert_eq!(store.load().unwrap().unwrap()["tabs"][0]["name"], "old");
        assert_eq!(
            store.backups().unwrap().len(),
            2,
            "restore keeps a safety backup"
        );
    }

    #[test]
    fn legacy_pyqt_configuration_migrates_without_touching_the_source_file() {
        let dir = TestDir::new();
        let path = dir.0.join("config.json");
        let original = serde_json::json!({
            "tabs":[{"name_edit_title":"旧标签","name_edit1":"/tmp","name_edit2":"echo",
                     "name_edit3_1":"-n","name_edit3_2":"hello","function3":"--file",
                     "parameter3":"a b","name_editOther":"out.txt","editDescription":"memo"}],
            "line_codes":[{"3":3}],
            "checkbox_statuses":[{"chkbox1":false,"chkbox3":true}]
        });
        fs::write(&path, serde_json::to_string(&original).unwrap()).unwrap();
        let store = ConfigStore::at(&path, dir.0.join("backup"));
        let config = store.load().unwrap().unwrap();
        assert_eq!(config["tabs"][0]["name"], "旧标签");
        assert_eq!(config["tabs"][0]["working_dir"], "/tmp");
        assert_eq!(config["tabs"][0]["functions"][0]["enabled"], false);
        assert_eq!(config["tabs"][0]["functions"][1]["function"], "--file");
        assert_eq!(config["tabs"][0]["functions"][1]["parameter"], "a b");
        assert_eq!(
            fs::read_to_string(path).unwrap(),
            serde_json::to_string(&original).unwrap()
        );
    }

    #[test]
    fn malformed_existing_configuration_cannot_be_overwritten() {
        let dir = TestDir::new();
        let path = dir.0.join("config.json");
        let store = ConfigStore::at(&path, dir.0.join("backup"));
        store
            .save(&serde_json::json!({"tabs":[{"name":"recoverable"}]}))
            .unwrap();
        store
            .save(&serde_json::json!({"tabs":[{"name":"current"}]}))
            .unwrap();
        let backup = store.backups().unwrap().remove(0);
        fs::write(&path, "not json").unwrap();
        assert!(store.load().is_err());
        assert!(store.save(&serde_json::json!({"tabs":[]})).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "not json");
        let restored = store.restore_backup(&backup).unwrap();
        assert_eq!(restored["tabs"][0]["name"], "recoverable");
        assert_eq!(
            store.load().unwrap().unwrap()["tabs"][0]["name"],
            "recoverable"
        );
        assert_eq!(
            store.backups().unwrap().len(),
            2,
            "corrupt bytes are preserved as a safety backup"
        );
    }

    #[test]
    fn shell_reader_preserves_multibyte_text_across_read_boundaries() {
        struct OneByteAtATime(std::io::Cursor<Vec<u8>>);
        impl Read for OneByteAtATime {
            fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
                let count = buffer.len().min(1);
                self.0.read(&mut buffer[..count])
            }
        }

        for (stream, bytes, expected) in [
            (
                OutputStream::Stdout,
                "中文🙂\n".as_bytes().to_vec(),
                "中文🙂\n",
            ),
            (
                OutputStream::Stderr,
                b"bad\xff\xe4\xb8\xad!".to_vec(),
                "bad\u{fffd}中!",
            ),
            (OutputStream::Stdout, b"end\xe4".to_vec(), "end\u{fffd}"),
        ] {
            let (sender, receiver) = mpsc::channel();
            let reader =
                spawn_reader(OneByteAtATime(std::io::Cursor::new(bytes)), stream, sender).unwrap();
            reader.join().unwrap();
            let output = receiver
                .try_iter()
                .map(|event| match event {
                    RunEvent::Output {
                        stream: actual,
                        text,
                    } => {
                        assert_eq!(actual, stream);
                        text
                    }
                    RunEvent::Finished { .. } => panic!("reader must not finish the process"),
                })
                .collect::<String>();
            assert_eq!(output, expected);
        }
    }

    #[cfg(unix)]
    #[test]
    fn shell_streams_stdout_stderr_and_exit_status() {
        let (handle, events) =
            start_shell("printf 'first\\n'; printf 'problem\\n' >&2; exit 7", "").unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut output = String::new();
        let mut stderr = String::new();
        let mut exit = None;
        while Instant::now() < deadline {
            if let Ok(event) = events.recv_timeout(Duration::from_millis(100)) {
                match event {
                    RunEvent::Output {
                        stream: OutputStream::Stdout,
                        text,
                    } => output.push_str(&text),
                    RunEvent::Output {
                        stream: OutputStream::Stderr,
                        text,
                    } => stderr.push_str(&text),
                    RunEvent::Finished { exit_code, stopped } => {
                        exit = Some((exit_code, stopped));
                        break;
                    }
                }
            }
        }
        assert!(output.contains("first"));
        assert!(stderr.contains("problem"));
        assert_eq!(exit, Some((7, false)));
        drop(handle);
    }

    #[cfg(windows)]
    #[test]
    fn windows_shell_streams_output_and_stops_process_tree() {
        let (_handle, events) =
            start_shell("echo backend-out & echo backend-err 1>&2 & exit /b 7", "").unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut stdout = String::new();
        let mut stderr = String::new();
        let mut exit = None;
        while Instant::now() < deadline {
            if let Ok(event) = events.recv_timeout(Duration::from_millis(100)) {
                match event {
                    RunEvent::Output {
                        stream: OutputStream::Stdout,
                        text,
                    } => stdout.push_str(&text),
                    RunEvent::Output {
                        stream: OutputStream::Stderr,
                        text,
                    } => stderr.push_str(&text),
                    RunEvent::Finished { exit_code, stopped } => {
                        exit = Some((exit_code, stopped));
                        break;
                    }
                }
            }
        }
        assert!(stdout.contains("backend-out"), "Windows stdout: {stdout}");
        assert!(stderr.contains("backend-err"), "Windows stderr: {stderr}");
        assert_eq!(exit, Some((7, false)));

        let (handle, events) = start_shell(
            r#"python -c "import sys; print('中文标准输出'); print('中文错误输出', file=sys.stderr)""#,
            "",
        )
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut stdout = String::new();
        let mut stderr = String::new();
        let mut exit = None;
        while Instant::now() < deadline {
            if let Ok(event) = events.recv_timeout(Duration::from_millis(100)) {
                match event {
                    RunEvent::Output {
                        stream: OutputStream::Stdout,
                        text,
                    } => stdout.push_str(&text),
                    RunEvent::Output {
                        stream: OutputStream::Stderr,
                        text,
                    } => stderr.push_str(&text),
                    RunEvent::Finished { exit_code, stopped } => {
                        exit = Some((exit_code, stopped));
                        break;
                    }
                }
            }
        }
        assert!(
            stdout.contains("中文标准输出"),
            "Windows UTF-8 stdout: {stdout}"
        );
        assert!(
            stderr.contains("中文错误输出"),
            "Windows UTF-8 stderr: {stderr}"
        );
        assert_eq!(exit, Some((0, false)));
        drop(handle);

        let (handle, events) = start_shell("ping -n 30 127.0.0.1 >NUL", "").unwrap();
        thread::sleep(Duration::from_millis(100));
        handle.stop().unwrap();
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut stopped = false;
        while Instant::now() < deadline {
            if let Ok(RunEvent::Finished {
                stopped: result, ..
            }) = events.recv_timeout(Duration::from_millis(100))
            {
                stopped = result;
                break;
            }
        }
        assert!(
            stopped,
            "Windows taskkill must finish the running shell and its child"
        );
    }

    #[cfg(unix)]
    #[test]
    fn shell_can_be_stopped() {
        let (handle, events) = start_shell("while :; do sleep 0.05; done", "").unwrap();
        thread::sleep(Duration::from_millis(80));
        handle.stop().unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut result = None;
        while Instant::now() < deadline {
            if let Ok(RunEvent::Finished { exit_code, stopped }) =
                events.recv_timeout(Duration::from_millis(100))
            {
                result = Some((exit_code, stopped));
                break;
            }
        }
        assert!(result.is_some_and(|(_, stopped)| stopped));
    }
}
