use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

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

    pub fn mark_game_stopped(&mut self, id: Uuid, exit_code: Option<i32>) {
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
                        message: format!("Minecraft завершился с кодом {exit_code:?}"),
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
    process.args(&launch_args).current_dir(&command.working_dir);
    for (key, value) in &command.env {
        process.env(key, value);
    }
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
    match child.wait() {
        Ok(status) => {
            let _ = events.unbounded_send(InstanceWorkerEvent::GameExited {
                id,
                code: status.code(),
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
