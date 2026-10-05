use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use futures::channel::mpsc::UnboundedSender;
use mc_launcher_core::prelude::{
    Account, InstallRequest, JavaInstallPolicy, LaunchOptions, Launcher, LoaderSpec, LoaderVersion,
    ProgressEvent,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::versions::LoaderChoice;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InstanceProfile {
    pub id: Uuid,
    pub name: String,
    pub game_version: String,
    pub loader: LoaderChoice,
    pub loader_version: Option<String>,
    #[serde(default = "default_memory_mb")]
    pub memory_mb: u32,
    #[serde(default)]
    pub java_path: Option<PathBuf>,
    #[serde(default)]
    pub installation_root: PathBuf,
    pub installed_version_id: Option<String>,
}

impl InstanceProfile {
    pub fn new(
        name: String,
        game_version: String,
        loader: LoaderChoice,
        loader_version: Option<String>,
        installation_root: PathBuf,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            game_version,
            loader,
            loader_version,
            memory_mb: 4096,
            java_path: None,
            installation_root,
            installed_version_id: None,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub enum InstanceRuntime {
    #[default]
    NotInstalled,
    Installing {
        stage: String,
        task: Option<String>,
        received: u64,
        total: Option<u64>,
        completed_tasks: usize,
    },
    DownloadingMod {
        name: String,
    },
    ModDownloadFailed {
        message: String,
    },
    ModDownloaded {
        filename: String,
    },
    Ready,
    Launching,
    Running {
        process_id: u32,
    },
    Removing,
    RemovalFailed {
        message: String,
    },
    Failed {
        message: String,
    },
}

#[derive(Debug, Default)]
pub struct InstancesState {
    profiles: Vec<InstanceProfile>,
    runtime: HashMap<Uuid, InstanceRuntime>,
}

impl InstancesState {
    pub fn from_profiles(profiles: Vec<InstanceProfile>) -> Self {
        let runtime = profiles
            .iter()
            .map(|profile| {
                let state = if profile.installed_version_id.is_some() {
                    InstanceRuntime::Ready
                } else {
                    InstanceRuntime::NotInstalled
                };
                (profile.id, state)
            })
            .collect();
        Self { profiles, runtime }
    }

    pub fn profiles(&self) -> &[InstanceProfile] {
        &self.profiles
    }

    pub fn profile(&self, id: Uuid) -> Option<&InstanceProfile> {
        self.profiles.iter().find(|profile| profile.id == id)
    }

    pub fn remove(&mut self, id: Uuid) -> Option<InstanceProfile> {
        let index = self.profiles.iter().position(|profile| profile.id == id)?;
        self.runtime.remove(&id);
        Some(self.profiles.remove(index))
    }

    pub fn runtime(&self, id: &Uuid) -> InstanceRuntime {
        self.runtime.get(id).cloned().unwrap_or_default()
    }

    pub fn add(&mut self, profile: InstanceProfile) {
        self.runtime.insert(
            profile.id,
            InstanceRuntime::Installing {
                stage: "Queued".to_owned(),
                task: None,
                received: 0,
                total: None,
                completed_tasks: 0,
            },
        );
        self.profiles.push(profile);
    }

    pub fn mark_installing(&mut self, id: Uuid) {
        self.runtime.insert(
            id,
            InstanceRuntime::Installing {
                stage: "Queued".to_owned(),
                task: None,
                received: 0,
                total: None,
                completed_tasks: 0,
            },
        );
    }

    pub fn mark_mod_downloading(&mut self, id: Uuid, name: String) {
        self.runtime
            .insert(id, InstanceRuntime::DownloadingMod { name });
    }

    pub fn mark_mod_downloaded(&mut self, id: Uuid, filename: String) {
        self.runtime
            .insert(id, InstanceRuntime::ModDownloaded { filename });
    }

    pub fn mark_mod_download_failed(&mut self, id: Uuid, message: String) {
        self.runtime
            .insert(id, InstanceRuntime::ModDownloadFailed { message });
    }

    pub fn update_progress(
        &mut self,
        id: Uuid,
        stage: Option<String>,
        task: Option<String>,
        bytes: Option<(u64, Option<u64>)>,
        completed: bool,
    ) {
        let current = self.runtime.get(&id).cloned().unwrap_or_default();
        let InstanceRuntime::Installing {
            stage: current_stage,
            task: current_task,
            received,
            total,
            completed_tasks,
        } = current
        else {
            return;
        };

        let task_changed = task
            .as_ref()
            .is_some_and(|task| current_task.as_ref() != Some(task));
        let current_total = total;
        let (received, total) = if task_changed {
            (0, None)
        } else {
            bytes.map_or((received, current_total), |(received, new_total)| {
                (received, new_total.or(current_total))
            })
        };
        self.runtime.insert(
            id,
            InstanceRuntime::Installing {
                stage: stage.unwrap_or(current_stage),
                task: task.or(current_task),
                received,
                total,
                completed_tasks: completed_tasks + usize::from(completed),
            },
        );
    }

    pub fn mark_installed(&mut self, id: Uuid, version_id: String) {
        if let Some(profile) = self.profiles.iter_mut().find(|profile| profile.id == id) {
            profile.installed_version_id = Some(version_id);
        }
        self.runtime.insert(id, InstanceRuntime::Ready);
    }

    pub fn set_memory_mb(&mut self, id: Uuid, memory_mb: u32) {
        if let Some(profile) = self.profiles.iter_mut().find(|profile| profile.id == id) {
            profile.memory_mb = memory_mb.clamp(512, 16_384);
        }
    }

    pub fn set_java_path(&mut self, id: Uuid, java_path: Option<PathBuf>) {
        if let Some(profile) = self.profiles.iter_mut().find(|profile| profile.id == id) {
            profile.java_path = java_path;
        }
    }

    pub fn mark_failed(&mut self, id: Uuid, message: String) {
        self.runtime.insert(id, InstanceRuntime::Failed { message });
    }

    pub fn mark_removing(&mut self, id: Uuid) {
        self.runtime.insert(id, InstanceRuntime::Removing);
    }

    pub fn mark_removal_failed(&mut self, id: Uuid, message: String) {
        self.runtime
            .insert(id, InstanceRuntime::RemovalFailed { message });
    }

    pub fn mark_launching(&mut self, id: Uuid) {
        self.runtime.insert(id, InstanceRuntime::Launching);
    }

    pub fn mark_running(&mut self, id: Uuid, process_id: u32) {
        self.runtime
            .insert(id, InstanceRuntime::Running { process_id });
    }

    pub fn mark_game_stopped(&mut self, id: Uuid, exit_code: Option<i32>, stderr: Option<String>) {
        if self
            .profiles
            .iter()
            .any(|profile| profile.id == id && profile.installed_version_id.is_some())
        {
            if exit_code == Some(0) {
                self.runtime.insert(id, InstanceRuntime::Ready);
            } else {
                self.runtime.insert(
                    id,
                    InstanceRuntime::Failed {
                        message: stderr.map_or_else(
                            || format!("Minecraft завершился с кодом {exit_code:?}"),
                            |stderr| {
                                format!(
                                    "Minecraft завершился с кодом {exit_code:?}:\n{}",
                                    stderr.trim()
                                )
                            },
                        ),
                    },
                );
            }
        } else {
            self.runtime.insert(id, InstanceRuntime::NotInstalled);
        }
    }
}

#[derive(Debug)]
pub enum InstanceWorkerEvent {
    Progress {
        id: Uuid,
        stage: Option<String>,
        task: Option<String>,
        bytes: Option<(u64, Option<u64>)>,
        completed: bool,
    },
    Installed {
        id: Uuid,
        version_id: String,
    },
    InstallFailed {
        id: Uuid,
        message: String,
    },
    Launched {
        id: Uuid,
        process_id: u32,
    },
    GameExited {
        id: Uuid,
        code: Option<i32>,
        stderr: Option<String>,
    },
    LaunchFailed {
        id: Uuid,
        message: String,
    },
    ProfileRemoved {
        id: Uuid,
    },
    ProfileRemovalFailed {
        id: Uuid,
        message: String,
    },
}

pub fn install_profile(
    profile: InstanceProfile,
    minecraft_directory: PathBuf,
    events: UnboundedSender<InstanceWorkerEvent>,
) {
    let id = profile.id;
    let forge_prefix = format!("{}-", profile.game_version);
    let loader = match profile.loader {
        LoaderChoice::Vanilla => None,
        LoaderChoice::Fabric => profile.loader_version.map(|version| LoaderSpec::Fabric {
            version: LoaderVersion::Exact(version),
        }),
        LoaderChoice::Forge => profile.loader_version.map(|version| {
            let version = if version.starts_with(&forge_prefix) {
                version
            } else {
                format!("{forge_prefix}{version}")
            };
            LoaderSpec::Forge {
                version: LoaderVersion::Exact(version),
            }
        }),
        LoaderChoice::NeoForge => profile.loader_version.map(|version| LoaderSpec::NeoForge {
            version: LoaderVersion::Exact(version),
        }),
    };
    let loader = match (profile.loader, loader) {
        (LoaderChoice::Vanilla, _) => None,
        (_, Some(loader)) => Some(loader),
        _ => {
            let _ = events.unbounded_send(InstanceWorkerEvent::InstallFailed {
                id,
                message: "Для выбранного загрузчика не указана версия".to_owned(),
            });
            return;
        }
    };

    let launcher = Launcher::new(minecraft_directory);
    let request = InstallRequest {
        minecraft_version: profile.game_version,
        loader,
        java: JavaInstallPolicy::Auto,
    };
    let mut reporter = |event: ProgressEvent| {
        let update = match event {
            ProgressEvent::StageStarted { stage } => InstanceWorkerEvent::Progress {
                id,
                stage: Some(format!("{stage:?}")),
                task: None,
                bytes: None,
                completed: false,
            },
            ProgressEvent::TaskStarted { label, .. } => InstanceWorkerEvent::Progress {
                id,
                stage: None,
                task: Some(label),
                bytes: None,
                completed: false,
            },
            ProgressEvent::TaskSkipped { label, .. } => InstanceWorkerEvent::Progress {
                id,
                stage: None,
                task: Some(label),
                bytes: None,
                completed: true,
            },
            ProgressEvent::TaskFinished { label } => InstanceWorkerEvent::Progress {
                id,
                stage: None,
                task: Some(label),
                bytes: None,
                completed: true,
            },
            ProgressEvent::BytesReceived {
                label,
                received,
                total,
            } => InstanceWorkerEvent::Progress {
                id,
                stage: None,
                task: Some(label),
                bytes: Some((received, total)),
                completed: false,
            },
        };
        let _ = events.unbounded_send(update);
    };

    match launcher.install_with_java_progress(request, profile.java_path.clone(), &mut reporter) {
        Ok(installed) => {
            let _ = events.unbounded_send(InstanceWorkerEvent::Installed {
                id,
                version_id: installed.version_id,
            });
        }
        Err(error) => {
            let _ = events.unbounded_send(InstanceWorkerEvent::InstallFailed {
                id,
                message: error.to_string(),
            });
        }
    }
}

pub fn launch_profile(
    profile: InstanceProfile,
    minecraft_directory: PathBuf,
    events: UnboundedSender<InstanceWorkerEvent>,
) {
    let id = profile.id;
    let Some(version_id) = profile.installed_version_id else {
        let _ = events.unbounded_send(InstanceWorkerEvent::LaunchFailed {
            id,
            message: "Профиль ещё не установлен".to_owned(),
        });
        return;
    };

    let game_directory = match crate::platform::profile_store::ensure_instance_directory(
        &profile.installation_root,
        id,
    ) {
        Ok(path) => path,
        Err(error) => {
            let path =
                crate::platform::profile_store::instance_directory(&profile.installation_root, id);
            let _ = events.unbounded_send(InstanceWorkerEvent::LaunchFailed {
                id,
                message: format!(
                    "Не удалось создать папку профиля {}: {error}",
                    path.display()
                ),
            });
            return;
        }
    };

    let launcher = Launcher::new(minecraft_directory);
    let version = match launcher.load_version(&version_id) {
        Ok(version) => version,
        Err(error) => {
            let _ = events.unbounded_send(InstanceWorkerEvent::LaunchFailed {
                id,
                message: error.to_string(),
            });
            return;
        }
    };

    let java_executable = profile
        .java_path
        .as_deref()
        .unwrap_or_else(|| Path::new("java"));
    let java_major_version = java_major_version(java_executable);
    if version.main_class.as_deref() == Some("net.minecraft.launchwrapper.Launch")
        && java_major_version.is_some_and(|major_version| major_version >= 9)
    {
        let _ = events.unbounded_send(InstanceWorkerEvent::LaunchFailed {
            id,
            message: format!(
                "Эта версия LaunchWrapper несовместима с Java 9 и новее (обнаружена Java {}). Установите Java 8 и выберите её java.exe в настройках профиля.",
                java_major_version.unwrap_or_default()
            ),
        });
        return;
    }

    let command = match launcher.build_launch_command_from_version(
        &version,
        LaunchOptions {
            account: Account::offline("Player"),
            java_executable: profile.java_path.clone(),
            game_directory: Some(game_directory),
            ..Default::default()
        },
    ) {
        Ok(command) => command,
        Err(error) => {
            let _ = events.unbounded_send(InstanceWorkerEvent::LaunchFailed {
                id,
                message: error.to_string(),
            });
            return;
        }
    };

    let max_memory_mb = profile.memory_mb.clamp(512, 16_384);
    let initial_memory_mb = (max_memory_mb / 4).clamp(512, 1024);
    let mut launch_args = vec![
        format!("-Xms{initial_memory_mb}M"),
        format!("-Xmx{max_memory_mb}M"),
    ];
    launch_args.extend(
        command
            .args
            .iter()
            .filter(|argument| !argument.starts_with("-Xms") && !argument.starts_with("-Xmx"))
            .cloned(),
    );

    let mut process = Command::new(&command.executable);
    process
        .args(&launch_args)
        .current_dir(&command.working_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in &command.env {
        process.env(key, value);
    }
    crate::platform::process::hide_console_window(&mut process);
    let mut child = match process.spawn() {
        Ok(child) => child,
        Err(error) => {
            let _ = events.unbounded_send(InstanceWorkerEvent::LaunchFailed {
                id,
                message: format!("Не удалось запустить Java: {error}"),
            });
            return;
        }
    };

    let _ = events.unbounded_send(InstanceWorkerEvent::Launched {
        id,
        process_id: child.id(),
    });

    let stdout_reader =
        match spawn_output_reader(child.stdout.take(), format!("read-minecraft-stdout-{id}")) {
            Ok(reader) => reader,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = events.unbounded_send(InstanceWorkerEvent::LaunchFailed {
                    id,
                    message: format!("Не удалось прочитать вывод Java: {error}"),
                });
                return;
            }
        };
    let stderr_reader =
        match spawn_output_reader(child.stderr.take(), format!("read-minecraft-stderr-{id}")) {
            Ok(reader) => reader,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                if let Some(reader) = stdout_reader {
                    let _ = reader.join();
                }
                let _ = events.unbounded_send(InstanceWorkerEvent::LaunchFailed {
                    id,
                    message: format!("Не удалось прочитать вывод Java: {error}"),
                });
                return;
            }
        };

    match child.wait() {
        Ok(status) => {
            let stdout = stdout_reader
                .and_then(|reader| reader.join().ok())
                .filter(|stdout| !stdout.trim().is_empty());
            let stderr = stderr_reader
                .and_then(|reader| reader.join().ok())
                .filter(|stderr| !stderr.trim().is_empty());
            let launch_details = format!(
                "Java: {} (версия {})\nSTDOUT:\n{}\nSTDERR:\n{}",
                command.executable.display(),
                java_major_version.map_or_else(|| "не определена".to_owned(), |v| v.to_string()),
                stdout.unwrap_or_default().trim(),
                stderr.unwrap_or_default().trim(),
            );
            let _ = events.unbounded_send(InstanceWorkerEvent::GameExited {
                id,
                code: status.code(),
                stderr: Some(launch_details),
            });
        }
        Err(error) => {
            let _ = events.unbounded_send(InstanceWorkerEvent::LaunchFailed {
                id,
                message: format!("Не удалось дождаться завершения игры: {error}"),
            });
        }
    }
}

fn spawn_output_reader<R>(
    stream: Option<R>,
    name: String,
) -> std::io::Result<Option<std::thread::JoinHandle<String>>>
where
    R: Read + Send + 'static,
{
    stream
        .map(|stream| {
            std::thread::Builder::new()
                .name(name)
                .spawn(move || read_output_tail(stream))
        })
        .transpose()
}

fn read_output_tail(mut stream: impl Read) -> String {
    const MAX_CAPTURED_BYTES: usize = 6 * 1024;

    let mut captured = Vec::new();
    let mut buffer = [0; 2048];
    loop {
        match stream.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(bytes_read) => {
                captured.extend_from_slice(&buffer[..bytes_read]);
                if captured.len() > MAX_CAPTURED_BYTES {
                    let excess = captured.len() - MAX_CAPTURED_BYTES;
                    captured.drain(..excess);
                }
            }
        }
    }

    String::from_utf8_lossy(&captured).into_owned()
}

fn java_major_version(java_executable: &Path) -> Option<u32> {
    let mut command = Command::new(java_executable);
    command.arg("-version");
    crate::platform::process::hide_console_window(&mut command);
    let output = command.output().ok()?;
    let version = [
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    ]
    .into_iter()
    .find_map(|stream| {
        stream
            .lines()
            .find_map(|line| line.split('"').nth(1).map(str::to_owned))
    })?;

    let mut components = version
        .split(|character: char| !character.is_ascii_digit())
        .filter(|component| !component.is_empty())
        .filter_map(|component| component.parse::<u32>().ok());
    let first = components.next()?;
    if first == 1 {
        components.next()
    } else {
        Some(first)
    }
}

pub fn remove_profile_data(profile: InstanceProfile, events: UnboundedSender<InstanceWorkerEvent>) {
    let id = profile.id;
    match crate::platform::profile_store::remove_instance_directory(&profile.installation_root, id)
    {
        Ok(()) => {
            let _ = events.unbounded_send(InstanceWorkerEvent::ProfileRemoved { id });
        }
        Err(error) => {
            let path =
                crate::platform::profile_store::instance_directory(&profile.installation_root, id);
            let _ = events.unbounded_send(InstanceWorkerEvent::ProfileRemovalFailed {
                id,
                message: format!("Не удалось удалить папку {}: {error}", path.display()),
            });
        }
    }
}

fn default_memory_mb() -> u32 {
    4096
}
