use crate::platform::profile_store;
use crate::{
    features::instances::{
        InstanceProfile, InstanceRuntime, InstanceWorkerEvent, InstancesState, install_profile,
        launch_profile, remove_profile_data,
    },
    features::mods::{ModCategory, ModDetailTab, ModSample, ModSort, ModsState, SAMPLE_MODS},
    features::settings::{AccentColor, Locale, SettingsState, ThemeChoice},
    features::versions::{
        LoaderChoice, LoaderVersionMode, VersionCatalogState, fetch_fabric_game_loaders,
        fetch_manifests,
    },
    router::Page,
};
use futures::StreamExt;
use gpui_kit::assets::IconName;
use gpui_kit::base::{Button as UiButton, StyledExt};
use gpui_kit::component::button::{Button as ComponentButton, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::scroll::{ScrollableElement, Scrollbar};
use gpui_kit::component::slider::{Slider, SliderState};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{Theme, ThemeMode, WindowExt, tooltip::Tooltip};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use std::rc::Rc;

#[derive(Clone, Copy)]
struct Palette {
    background: u32,
    sidebar: u32,
    surface: u32,
    border: u32,
    selected: u32,
    control: u32,
    hover: u32,
    selected_hover: u32,
    pressed: u32,
    foreground: u32,
    muted: u32,
    accent: u32,
    accent_foreground: u32,
}

impl Palette {
    fn for_settings(theme: ThemeChoice, accent: AccentColor) -> Self {
        let mut palette = match theme {
            ThemeChoice::Dark => Self {
                background: 0x111111,
                sidebar: 0x0b0b0b,
                surface: 0x181818,
                border: 0x2b2b2b,
                selected: 0x252525,
                control: 0x141414,
                hover: 0x262626,
                selected_hover: 0x343434,
                pressed: 0x3b3b3b,
                foreground: 0xf2f2f2,
                muted: 0xa0a0a0,
                accent: 0,
                accent_foreground: 0,
            },
            ThemeChoice::Light => Self {
                background: 0xf3f3f1,
                sidebar: 0xffffff,
                surface: 0xffffff,
                border: 0xe2e2df,
                selected: 0xe9e9e7,
                control: 0xf8f8f6,
                hover: 0xefefec,
                selected_hover: 0xdfdfdc,
                pressed: 0xe4e4e1,
                foreground: 0x202020,
                muted: 0x747474,
                accent: 0,
                accent_foreground: 0,
            },
        };
        palette.accent = accent.color(theme);
        palette.accent_foreground = accent.foreground(theme);
        palette
    }
}

#[derive(Debug, Default)]
struct SidebarState {
    collapsed: bool,
}

impl SidebarState {
    fn is_collapsed(&self) -> bool {
        self.collapsed
    }

    fn toggle(&mut self) {
        self.collapsed = !self.collapsed;
    }
}

pub struct LauncherApp {
    active_page: Page,
    settings: SettingsState,
    mods: ModsState,
    versions: VersionCatalogState,
    instances: InstancesState,
    sidebar: SidebarState,
    search_input: Entity<InputState>,
    profile_name_input: Entity<InputState>,
    loader_version_input: Entity<InputState>,
    manifest_scroll: gpui_kit::base::VirtualListScrollHandle,
    search_query: String,
    _search_subscription: Subscription,
}

impl LauncherApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Поиск по названию, описанию или автору")
        });
        let profile_name_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Название профиля"));
        let loader_version_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Версия загрузчика"));
        let search_subscription =
            cx.subscribe(&search_input, |this, input, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    let query = input.read(cx).value().to_string();
                    this.search_query = query.clone();
                    this.mods.set_query(query);
                    cx.notify();
                }
            });
        let default_download_directory = match profile_store::default_download_directory() {
            Ok(path) => path,
            Err(error) => {
                tracing::warn!(%error, "launcher default download directory is unavailable");
                std::path::PathBuf::new()
            }
        };
        let download_directory = match profile_store::load_download_directory() {
            Ok(Some(path)) => path,
            Ok(None) => default_download_directory.clone(),
            Err(error) => {
                tracing::warn!(%error, "failed to load launcher download directory");
                default_download_directory.clone()
            }
        };
        let mut settings = SettingsState::default();
        settings.set_download_directory(download_directory.clone());

        let instances = match profile_store::load() {
            Ok(mut profiles) => {
                for profile in &mut profiles {
                    if profile.installation_root.as_os_str().is_empty() {
                        profile.installation_root = download_directory.clone();
                    }
                    profile.memory_mb = profile.memory_mb.clamp(512, 16_384);
                }
                InstancesState::from_profiles(profiles)
            }
            Err(error) => {
                tracing::warn!(%error, "failed to load saved Minecraft profiles");
                InstancesState::default()
            }
        };

        Self {
            active_page: Page::Home,
            settings,
            mods: ModsState::default(),
            versions: VersionCatalogState::default(),
            instances,
            sidebar: SidebarState::default(),
            search_input,
            profile_name_input,
            loader_version_input,
            manifest_scroll: gpui_kit::base::VirtualListScrollHandle::new(),
            search_query: String::new(),
            _search_subscription: search_subscription,
        }
    }

    fn set_locale(&mut self, locale: Locale, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.set_locale(locale);
        gpui_kit::component::set_locale(locale.tag());
        self.search_input.update(cx, |input, cx| {
            input.set_placeholder(
                locale.text(
                    "Поиск по названию, описанию или автору",
                    "Search by name, description, or author",
                ),
                window,
                cx,
            );
        });
        self.profile_name_input.update(cx, |input, cx| {
            input.set_placeholder(locale.text("Название профиля", "Profile name"), window, cx);
        });
        self.loader_version_input.update(cx, |input, cx| {
            input.set_placeholder(
                locale.text("Версия загрузчика", "Loader version"),
                window,
                cx,
            );
        });
        cx.notify();
    }

    fn choose_download_directory(&mut self, cx: &mut Context<Self>) {
        let locale = self.settings.locale();
        let response = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(
                locale
                    .text("Выбери папку для загрузок", "Choose a downloads folder")
                    .into(),
            ),
        });
        cx.spawn(async move |this, cx| {
            let selected_path = match response.await {
                Ok(Ok(Some(paths))) => paths.into_iter().find(|path| path.is_dir()),
                Ok(Ok(None)) => None,
                Ok(Err(error)) => {
                    tracing::warn!(%error, "failed to open folder picker");
                    None
                }
                Err(error) => {
                    tracing::warn!(%error, "folder picker response channel closed");
                    None
                }
            };
            let Some(path) = selected_path else {
                return;
            };
            let _ = this.update(cx, |this, cx| {
                this.settings.set_download_directory(path.clone());
                if let Err(error) = profile_store::save_download_directory(path) {
                    tracing::error!(%error, "failed to persist launcher download directory");
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn choose_java_for_profile(&mut self, id: uuid::Uuid, cx: &mut Context<Self>) {
        let locale = self.settings.locale();
        let response = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(
                locale
                    .text("Выбери java.exe", "Choose java executable")
                    .into(),
            ),
        });
        let owner = cx.entity().downgrade();
        cx.spawn(async move |this, cx| {
            match response.await {
                Ok(Ok(Some(paths))) => {
                    if let Some(java_path) = paths.into_iter().find(|path| path.is_file()) {
                        let _ = this.update(cx, |this, cx| {
                            this.set_profile_java_path(id, Some(java_path), cx);
                        });
                    }
                }
                Ok(Ok(None)) => {}
                Ok(Err(error)) => {
                    tracing::warn!(%error, "failed to open Java file picker");
                }
                Err(error) => {
                    tracing::warn!(%error, "Java file picker response channel closed");
                }
            }
            let _ = owner.update(cx, |_, cx| cx.notify());
        })
        .detach();
    }

    fn set_profile_java_path(
        &mut self,
        id: uuid::Uuid,
        java_path: Option<std::path::PathBuf>,
        cx: &mut Context<Self>,
    ) {
        self.instances.set_java_path(id, java_path);
        self.persist_profiles();
        cx.notify();
    }

    fn persist_profiles(&self) {
        if let Err(error) = profile_store::save(self.instances.profiles()) {
            tracing::error!(%error, "failed to persist Minecraft profiles");
        }
    }

    fn navigate_to(&mut self, page: Page, window: &mut Window, cx: &mut Context<Self>) {
        if self.active_page == page {
            return;
        }

        self.active_page = page;
        self.search_query.clear();
        self.mods.set_query(String::new());
        self.search_input
            .update(cx, |input, cx| input.set_value("", window, cx));
        cx.notify();
    }

    fn load_version_manifests(&mut self, cx: &mut Context<Self>) {
        if !self.versions.begin_loading() {
            return;
        }

        let client = cx.http_client();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = fetch_manifests(client).await;
            let _ = this.update(cx, |this, cx| {
                this.versions.finish_loading(result);
                this.load_fabric_game_loaders(cx);
                cx.notify();
            });
        })
        .detach();
    }

    fn load_fabric_game_loaders(&mut self, cx: &mut Context<Self>) {
        if self.versions.loader_choice() != LoaderChoice::Fabric {
            return;
        }
        let Some(game_version) = self.versions.game_version().map(str::to_owned) else {
            return;
        };
        if !self.versions.begin_fabric_game_load(&game_version) {
            return;
        }

        let client = cx.http_client();
        cx.spawn(async move |this, cx| {
            let result = fetch_fabric_game_loaders(client, game_version.clone()).await;
            let _ = this.update(cx, move |this, cx| {
                this.versions.finish_fabric_game_load(game_version, result);
                cx.notify();
            });
        })
        .detach();
    }

    fn start_profile_install(&mut self, id: uuid::Uuid, cx: &mut Context<Self>) {
        let Some(profile) = self.instances.profile(id).cloned() else {
            return;
        };
        let minecraft_directory = profile_store::shared_game_directory(&profile.installation_root);

        self.instances.mark_installing(id);
        let (sender, receiver) = futures::channel::mpsc::unbounded();
        let worker = std::thread::Builder::new()
            .name(format!("install-minecraft-{id}"))
            .spawn(move || install_profile(profile, minecraft_directory, sender));

        match worker {
            Ok(_) => self.listen_for_instance_events(receiver, cx),
            Err(error) => self
                .instances
                .mark_failed(id, format!("Не удалось запустить установщик: {error}")),
        }
        cx.notify();
    }

    fn start_profile_launch(&mut self, id: uuid::Uuid, cx: &mut Context<Self>) {
        let Some(profile) = self.instances.profile(id).cloned() else {
            return;
        };
        if profile.installed_version_id.is_none() {
            self.start_profile_install(id, cx);
            return;
        }
        let minecraft_directory = profile_store::shared_game_directory(&profile.installation_root);

        self.instances.mark_launching(id);
        let (sender, receiver) = futures::channel::mpsc::unbounded();
        let worker = std::thread::Builder::new()
            .name(format!("launch-minecraft-{id}"))
            .spawn(move || launch_profile(profile, minecraft_directory, sender));

        match worker {
            Ok(_) => self.listen_for_instance_events(receiver, cx),
            Err(error) => self
                .instances
                .mark_failed(id, format!("Не удалось запустить игру: {error}")),
        }
        cx.notify();
    }

    fn listen_for_instance_events(
        &self,
        mut events: futures::channel::mpsc::UnboundedReceiver<InstanceWorkerEvent>,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |this, cx| {
            while let Some(event) = events.next().await {
                let _ = this.update(cx, |this, cx| {
                    this.handle_instance_event(event);
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn handle_instance_event(&mut self, event: InstanceWorkerEvent) {
        match event {
            InstanceWorkerEvent::Progress {
                id,
                stage,
                task,
                bytes,
                completed,
            } => self
                .instances
                .update_progress(id, stage, task, bytes, completed),
            InstanceWorkerEvent::Installed { id, version_id } => {
                self.instances.mark_installed(id, version_id);
                if let Err(error) = profile_store::save(self.instances.profiles()) {
                    tracing::error!(%error, "failed to save installed Minecraft profile");
                    self.instances.mark_failed(
                        id,
                        format!("Установка завершилась, но профиль не сохранился: {error}"),
                    );
                }
            }
            InstanceWorkerEvent::InstallFailed { id, message }
            | InstanceWorkerEvent::LaunchFailed { id, message } => {
                self.instances.mark_failed(id, message);
            }
            InstanceWorkerEvent::ProfileRemoved { id } => {
                self.instances.remove(id);
                self.persist_profiles();
            }
            InstanceWorkerEvent::ProfileRemovalFailed { id, message } => {
                self.instances.mark_removal_failed(id, message);
            }
            InstanceWorkerEvent::Launched { id, process_id } => {
                self.instances.mark_running(id, process_id);
            }
            InstanceWorkerEvent::GameExited { id, code } => {
                self.instances.mark_game_stopped(id, code);
            }
        }
    }

    fn open_profile_settings(
        &mut self,
        id: uuid::Uuid,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(profile) = self.instances.profile(id) else {
            return;
        };
        let profile_name = profile.name.clone();
        let memory_slider = cx.new(|_| {
            SliderState::new()
                .max(16_384.)
                .min(512.)
                .step(256.)
                .default_value(profile.memory_mb as f32)
        });
        let locale = self.settings.locale();
        let palette = Palette::for_settings(self.settings.theme(), self.settings.accent());
        let title = format!(
            "{} · {profile_name}",
            locale.text("Настройки сборки", "Profile settings")
        );
        let owner = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _, _| {
            let content_owner = owner.clone();
            let content_slider = memory_slider.clone();
            dialog
                .w(px(520.))
                .title(title.clone())
                .content(move |content, _, cx| {
                    let current_profile = content_owner
                        .upgrade()
                        .and_then(|owner| owner.read(cx).instances.profile(id).cloned());
                    let java_path = current_profile
                        .as_ref()
                        .and_then(|profile| profile.java_path.as_ref())
                        .map(|path| path.to_string_lossy().to_string())
                        .unwrap_or_else(|| {
                            locale
                                .text("Автоматически (Java в PATH)", "Automatic (Java on PATH)")
                                .to_string()
                        });
                    let memory_mb = content_slider.read(cx).value().end().round() as u32;
                    let choose_owner = content_owner.clone();
                    let automatic_owner = content_owner.clone();
                    let save_owner = content_owner.clone();
                    let save_slider = content_slider.clone();

                    content.child(
                        div()
                            .v_flex()
                            .w_full()
                            .gap_4()
                            .p_4()
                            .text_color(rgb(palette.foreground))
                            .child(
                                div()
                                    .v_flex()
                                    .gap_2()
                                    .child(field_label(
                                        locale.text("Оперативная память", "Memory allocation"),
                                    ))
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .text_color(rgb(palette.muted))
                                            .child(locale.text(
                                                "Лимит памяти для этой сборки.",
                                                "Heap limit for this profile.",
                                            )),
                                    )
                                    .child(
                                        div()
                                            .h_flex()
                                            .items_center()
                                            .gap_3()
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .child(Slider::new(&content_slider).h(px(24.))),
                                            )
                                            .child(
                                                div()
                                                    .w(px(88.))
                                                    .px_2()
                                                    .py_2()
                                                    .rounded(px(7.))
                                                    .bg(rgb(palette.control))
                                                    .text_center()
                                                    .text_color(rgb(palette.foreground))
                                                    .child(format!("{memory_mb} MB")),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .h_flex()
                                            .justify_between()
                                            .text_size(px(10.))
                                            .text_color(rgb(palette.muted))
                                            .child("512 MB")
                                            .child("16 GB"),
                                    ),
                            )
                            .child(div().h(px(1.)).w_full().bg(rgb(palette.border)))
                            .child(
                                div()
                                    .v_flex()
                                    .gap_2()
                                    .child(field_label(locale.text("Java", "Java")))
                                    .child(
                                        div()
                                            .w_full()
                                            .px_3()
                                            .py_2()
                                            .rounded(px(7.))
                                            .bg(rgb(palette.control))
                                            .text_size(px(11.))
                                            .text_color(rgb(palette.muted))
                                            .child(java_path),
                                    )
                                    .child(
                                        div()
                                            .h_flex()
                                            .gap_2()
                                            .child(
                                                UiButton::new(format!("profile-java-choose-{id}"))
                                                    .px_3()
                                                    .py_2()
                                                    .rounded(px(7.))
                                                    .bg(rgb(palette.control))
                                                    .text_color(rgb(palette.foreground))
                                                    .hover(|style| style.bg(rgb(palette.hover)))
                                                    .on_click(move |_, _, cx| {
                                                        if let Some(owner) = choose_owner.upgrade()
                                                        {
                                                            let _ = owner.update(cx, |this, cx| {
                                                                this.choose_java_for_profile(
                                                                    id, cx,
                                                                );
                                                            });
                                                        }
                                                    })
                                                    .child(
                                                        locale
                                                            .text("Выбрать Java…", "Choose Java…"),
                                                    ),
                                            )
                                            .child(
                                                UiButton::new(format!("profile-java-auto-{id}"))
                                                    .px_3()
                                                    .py_2()
                                                    .rounded(px(7.))
                                                    .bg(rgb(palette.control))
                                                    .text_color(rgb(palette.muted))
                                                    .hover(|style| style.bg(rgb(palette.hover)))
                                                    .on_click(move |_, _, cx| {
                                                        if let Some(owner) =
                                                            automatic_owner.upgrade()
                                                        {
                                                            let _ = owner.update(cx, |this, cx| {
                                                                this.set_profile_java_path(
                                                                    id, None, cx,
                                                                );
                                                            });
                                                        }
                                                    })
                                                    .child(
                                                        locale.text("Автоматически", "Automatic"),
                                                    ),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(10.))
                                    .text_color(rgb(palette.muted))
                                    .child(locale.text(
                                        "Для запуска нужна Java, совместимая с версией Minecraft.",
                                        "The selected Java must support this Minecraft version.",
                                    )),
                            )
                            .child(
                                UiButton::new(format!("profile-settings-save-{id}"))
                                    .w_full()
                                    .px_3()
                                    .py_2()
                                    .rounded(px(8.))
                                    .bg(rgb(palette.accent))
                                    .text_color(rgb(palette.accent_foreground))
                                    .hover(|style| style.opacity(0.9))
                                    .on_click(move |_, window, cx| {
                                        let memory_mb =
                                            save_slider.read(cx).value().end().round() as u32;
                                        if let Some(owner) = save_owner.upgrade() {
                                            let _ = owner.update(cx, |this, cx| {
                                                this.instances.set_memory_mb(id, memory_mb);
                                                this.persist_profiles();
                                                cx.notify();
                                            });
                                            window.close_dialog(cx);
                                        }
                                    })
                                    .child(locale.text("Сохранить", "Save")),
                            ),
                    )
                })
        });
    }

    fn open_delete_profile_confirmation(
        &mut self,
        id: uuid::Uuid,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(profile) = self.instances.profile(id) else {
            return;
        };
        let profile_name = profile.name.clone();
        let locale = self.settings.locale();
        let palette = Palette::for_settings(self.settings.theme(), self.settings.accent());
        let title = locale.text("Удалить сборку?", "Delete profile?");
        let owner = cx.entity().downgrade();

        window.open_dialog(cx, move |dialog, _, _| {
            let delete_owner = owner.clone();
            let message = match locale {
                Locale::Ru => format!(
                    "Удалить «{profile_name}» и его локальные данные? Миры, настройки и папка профиля будут удалены. Общие assets и библиотеки сохранятся."
                ),
                Locale::En => format!(
                    "Delete “{profile_name}” and its local data? Worlds, settings, and the profile folder will be removed. Shared assets and libraries will be kept."
                ),
            };

            dialog
                .w(px(460.))
                .title(title)
                .content(move |content, _, _| {
                    let action_owner = delete_owner.clone();
                    content.child(
                        div()
                            .v_flex()
                            .w_full()
                            .gap_4()
                            .p_4()
                            .text_color(rgb(palette.foreground))
                            .child(div().text_size(px(13.)).child(message.clone()))
                            .child(
                                div()
                                    .h_flex()
                                    .justify_end()
                                    .gap_2()
                                    .child(
                                        UiButton::new(format!("cancel-delete-profile-{id}"))
                                            .px_3()
                                            .py_2()
                                            .rounded(px(8.))
                                            .bg(rgb(palette.control))
                                            .text_color(rgb(palette.foreground))
                                            .hover(|style| style.bg(rgb(palette.hover)))
                                            .on_click(|_, window, cx| window.close_dialog(cx))
                                            .child(locale.text("Отмена", "Cancel")),
                                    )
                                    .child(
                                        UiButton::new(format!("confirm-delete-profile-{id}"))
                                            .px_3()
                                            .py_2()
                                            .rounded(px(8.))
                                            .bg(rgb(0xc0392b))
                                            .text_color(rgb(0xffffff))
                                            .hover(|style| style.opacity(0.9))
                                            .on_click(move |_, window, cx| {
                                                if let Some(owner) = action_owner.upgrade() {
                                                    let _ = owner.update(cx, |this, cx| {
                                                        let Some(profile) = this
                                                            .instances
                                                            .profile(id)
                                                            .cloned()
                                                        else {
                                                            return;
                                                        };
                                                        let runtime = this.instances.runtime(&id);
                                                        if matches!(
                                                            runtime,
                                                            InstanceRuntime::Installing { .. }
                                                                | InstanceRuntime::Launching
                                                                | InstanceRuntime::Running { .. }
                                                                | InstanceRuntime::Removing
                                                        ) {
                                                            return;
                                                        }

                                                        this.instances.mark_removing(id);
                                                        let (sender, receiver) =
                                                            futures::channel::mpsc::unbounded();
                                                        let worker = std::thread::Builder::new()
                                                            .name(format!("remove-profile-{id}"))
                                                            .spawn(move || {
                                                                remove_profile_data(profile, sender)
                                                            });
                                                        match worker {
                                                            Ok(_) => this.listen_for_instance_events(
                                                                receiver, cx,
                                                            ),
                                                            Err(error) => this.instances
                                                                .mark_removal_failed(
                                                                    id,
                                                                    format!(
                                                                        "Не удалось запустить удаление: {error}"
                                                                    ),
                                                                ),
                                                        }
                                                        cx.notify();
                                                    });
                                                }
                                                window.close_dialog(cx);
                                            })
                                            .child(locale.text("Удалить", "Delete")),
                                    ),
                            ),
                    )
                })
        });
    }

    fn open_version_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.load_version_manifests(cx);
        self.versions.close_game_version_menu();
        self.profile_name_input
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.loader_version_input
            .update(cx, |input, cx| input.set_value("", window, cx));
        let owner = cx.entity().downgrade();
        let title = self
            .settings
            .locale()
            .text("Добавить Minecraft", "Add Minecraft");
        window.open_dialog(cx, move |dialog, _, _| {
            let content_owner = owner.clone();
            dialog
                .w(px(560.))
                .title(title)
                .content(move |content, _, cx| {
                    let Some(owner) = content_owner.upgrade() else {
                        return content;
                    };
                    let app = owner.read(cx);
                    let palette =
                        Palette::for_settings(app.settings.theme(), app.settings.accent());
                    let locale = app.settings.locale();
                    content.child(app.version_picker_body(
                        owner.clone(),
                        &app.profile_name_input,
                        &app.loader_version_input,
                        palette,
                        locale,
                    ))
                })
        });
    }

    fn version_picker_body(
        &self,
        owner: Entity<Self>,
        profile_name_input: &Entity<InputState>,
        loader_version_input: &Entity<InputState>,
        palette: Palette,
        locale: Locale,
    ) -> AnyElement {
        let current_loader = self.versions.loader_choice();
        let loader_options = [
            (LoaderChoice::Vanilla, "Vanilla"),
            (LoaderChoice::Fabric, "Fabric"),
            (LoaderChoice::NeoForge, "NeoForge"),
            (LoaderChoice::Forge, "Forge"),
        ];
        let loader_buttons = loader_options.into_iter().map(|(loader, label)| {
            let is_selected = current_loader == loader;
            let loader_owner = owner.downgrade();
            UiButton::new(format!("version-loader-{label}"))
                .flex_1()
                .px_2()
                .py_2()
                .rounded(px(8.))
                .border_1()
                .border_color(rgb(if is_selected {
                    palette.accent
                } else {
                    palette.border
                }))
                .bg(rgb(if is_selected {
                    palette.accent
                } else {
                    palette.control
                }))
                .text_color(rgb(if is_selected {
                    palette.accent_foreground
                } else {
                    palette.muted
                }))
                .hover(|style| style.bg(rgb(palette.hover)))
                .on_click(move |_, _, cx| {
                    if let Some(owner) = loader_owner.upgrade() {
                        let _ = owner.update(cx, |this, cx| {
                            this.versions.set_loader_choice(loader);
                            this.load_fabric_game_loaders(cx);
                            cx.notify();
                        });
                    }
                })
                .child(label)
        });

        let mode_buttons = [
            (
                LoaderVersionMode::Stable,
                locale.text("Стабильная", "Stable"),
            ),
            (
                LoaderVersionMode::Latest,
                locale.text("Последняя", "Latest"),
            ),
            (LoaderVersionMode::Other, locale.text("Другая", "Other")),
        ]
        .into_iter()
        .map(|(mode, label)| {
            let is_selected = self.versions.loader_version_mode() == mode;
            let mode_owner = owner.downgrade();
            UiButton::new(format!("loader-version-mode-{mode:?}"))
                .flex_1()
                .px_2()
                .py_2()
                .rounded(px(8.))
                .border_1()
                .border_color(rgb(if is_selected {
                    palette.accent
                } else {
                    palette.border
                }))
                .bg(rgb(if is_selected {
                    palette.accent
                } else {
                    palette.control
                }))
                .text_color(rgb(if is_selected {
                    palette.accent_foreground
                } else {
                    palette.muted
                }))
                .hover(|style| style.bg(rgb(palette.hover)))
                .on_click(move |_, _, cx| {
                    if let Some(owner) = mode_owner.upgrade() {
                        let _ = owner.update(cx, |this, cx| {
                            this.versions.set_loader_version_mode(mode);
                            cx.notify();
                        });
                    }
                })
                .child(label)
        });

        let game_version_label = self.versions.game_version().map_or_else(
            || {
                locale
                    .text("Загрузка манифеста…", "Loading manifest…")
                    .to_string()
            },
            str::to_owned,
        );
        let game_menu =
            if self.versions.game_version_menu_open() {
                if let Some(manifest) = self.versions.manifest() {
                    let show_all_versions = self.versions.shows_all_game_versions();
                    let visible_indices = Rc::new(
                        manifest
                            .versions
                            .iter()
                            .enumerate()
                            .filter_map(|(index, version)| {
                                (show_all_versions || version.version_type == "release")
                                    .then_some(index)
                            })
                            .collect::<Vec<_>>(),
                    );
                    let item_sizes = Rc::new(vec![size(px(1.), px(42.)); visible_indices.len()]);
                    let list_owner = owner.clone();
                    let scroll_handle = self.manifest_scroll.clone();
                    let visible_indices_for_render = visible_indices.clone();
                    let toggle_owner = owner.downgrade();
                    let release_count = manifest
                        .versions
                        .iter()
                        .filter(|version| version.version_type == "release")
                        .count();
                    let toggle_label = if show_all_versions {
                        locale
                            .text("Показывать только релизы", "Show releases only")
                            .to_string()
                    } else {
                        format!(
                            "{} ({})",
                            locale.text("Показать все версии", "Show all versions"),
                            manifest.versions.len().saturating_sub(release_count)
                        )
                    };
                    Some(
                        div()
                            .v_flex()
                            .w_full()
                            .gap_2()
                            .child(
                                UiButton::new("toggle-manifest-version-filter")
                                    .w_full()
                                    .px_2()
                                    .py_1()
                                    .rounded(px(6.))
                                    .text_size(px(11.))
                                    .text_color(rgb(palette.muted))
                                    .hover(|style| style.bg(rgb(palette.hover)))
                                    .on_click(move |_, _, cx| {
                                        if let Some(owner) = toggle_owner.upgrade() {
                                            let _ = owner.update(cx, |this, cx| {
                                                this.versions.toggle_all_game_versions();
                                                this.manifest_scroll
                                                    .scroll_to_item(0, ScrollStrategy::Top);
                                                cx.notify();
                                            });
                                        }
                                    })
                                    .child(toggle_label),
                            )
                            .child(
                                div()
                                    .w_full()
                                    .h(px(250.))
                                    .rounded(px(8.))
                                    .border_1()
                                    .border_color(rgb(palette.border))
                                    .bg(rgb(palette.surface))
                                    .overflow_hidden()
                                    .child(
                                        gpui_kit::base::v_virtual_list(
                                            owner.clone(),
                                            "minecraft-version-manifest",
                                            item_sizes,
                                            move |this, visible_range, _, _| {
                                                let Some(manifest) = this.versions.manifest()
                                                else {
                                                    return Vec::new();
                                                };
                                                visible_range
                                                .filter_map(|visible_index| {
                                                    let source_index = *visible_indices_for_render
                                                        .get(visible_index)?;
                                                    manifest.versions.get(source_index)
                                                })
                                                .map(|version| {
                                            let id = version.id.clone();
                                            let selected =
                                                this.versions.game_version() == Some(id.as_str());
                                            let is_release = version.version_type == "release";
                                            let version_type = version.version_type.clone();
                                            let select_owner = list_owner.downgrade();
                                            let selected_id = id.clone();
                                            UiButton::new(format!("select-game-version-{id}"))
                                                .w_full()
                                                .h(px(42.))
                                                .px_3()
                                                .rounded(px(6.))
                                                .bg(rgb(if selected {
                                                    palette.selected
                                                } else {
                                                    palette.surface
                                                }))
                                                .text_color(rgb(palette.foreground))
                                                .hover(|style| style.bg(rgb(palette.hover)))
                                                .on_click(move |_, _, cx| {
                                                    if let Some(owner) = select_owner.upgrade() {
                                                        let selected_id = selected_id.clone();
                                                        let _ = owner.update(cx, |this, cx| {
                                                            this.versions
                                                                .select_game_version(selected_id);
                                                            this.load_fabric_game_loaders(cx);
                                                            cx.notify();
                                                        });
                                                    }
                                                })
                                                .child(
                                                    div()
                                                        .h_flex()
                                                        .w_full()
                                                        .items_center()
                                                        .justify_between()
                                                        .child(id)
                                                        .child(
                                                            div()
                                                                .text_size(px(10.))
                                                                .text_color(rgb(if is_release {
                                                                    palette.accent
                                                                } else {
                                                                    palette.muted
                                                                }))
                                                                .child(version_type_label(
                                                                    &version_type,
                                                                    locale,
                                                                )),
                                                        ),
                                                )
                                                })
                                                .collect::<Vec<_>>()
                                            },
                                        )
                                        .with_sizing_behavior(ListSizingBehavior::Infer)
                                        .track_scroll(&scroll_handle)
                                        .into_any_element(),
                                    )
                                    .child(Scrollbar::vertical(&scroll_handle)),
                            )
                            .into_any_element(),
                    )
                } else if self.versions.is_loading() {
                    Some(
                        div()
                            .h(px(100.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .gap_2()
                            .text_color(rgb(palette.muted))
                            .child(IconName::LoaderCircle)
                            .child(locale.text("Загружаем версии…", "Loading versions…"))
                            .into_any_element(),
                    )
                } else {
                    let retry_owner = owner.downgrade();
                    Some(
                        div()
                            .h_flex()
                            .items_center()
                            .justify_between()
                            .text_color(rgb(palette.muted))
                            .child(locale.text("Манифест недоступен", "Manifest unavailable"))
                            .child(
                                UiButton::new("retry-version-manifests")
                                    .px_3()
                                    .py_2()
                                    .rounded(px(7.))
                                    .bg(rgb(palette.control))
                                    .text_color(rgb(palette.foreground))
                                    .on_click(move |_, _, cx| {
                                        if let Some(owner) = retry_owner.upgrade() {
                                            let _ = owner.update(cx, |this, cx| {
                                                this.load_version_manifests(cx);
                                            });
                                        }
                                    })
                                    .child(locale.text("Повторить", "Retry")),
                            )
                            .into_any_element(),
                    )
                }
            } else {
                None
            };

        let open_versions_owner = owner.downgrade();
        let profile_input = profile_name_input.clone();
        let loader_input = loader_version_input.clone();
        let create_owner = owner.downgrade();
        let show_loader_version = current_loader != LoaderChoice::Vanilla;
        let show_custom_loader_version =
            show_loader_version && self.versions.loader_version_mode() == LoaderVersionMode::Other;
        let loader_version_feedback = if show_loader_version {
            if show_custom_loader_version {
                Some(
                    locale
                        .text("Укажи номер версии вручную.", "Enter a loader version.")
                        .to_string(),
                )
            } else if let Some(version) = self.versions.loader_version(None) {
                Some(format!(
                    "{} {version}",
                    locale.text("Будет выбрана версия", "Selected version:")
                ))
            } else if current_loader == LoaderChoice::Fabric {
                self.versions.game_version().map(|game_version| {
                    if self.versions.fabric_game_loading(game_version) {
                        locale
                            .text(
                                "Загружаем совместимые версии Fabric…",
                                "Loading compatible Fabric versions…",
                            )
                            .to_string()
                    } else if let Some(error) = self.versions.fabric_game_error(game_version) {
                        format!(
                            "{}: {error}",
                            locale.text("Не удалось загрузить Fabric", "Could not load Fabric")
                        )
                    } else if self.versions.fabric_game_loader_count(game_version) == Some(0) {
                        if game_version == "1.7.10" {
                            locale
                                .text(
                                    "Официальный Fabric не поддерживает Minecraft 1.7.10. Для него нужен отдельный загрузчик LegacyFabric.",
                                    "Official Fabric does not support Minecraft 1.7.10. It requires the separate LegacyFabric loader.",
                                )
                                .to_string()
                        } else {
                            locale
                                .text(
                                    "Fabric API не предлагает совместимую версию для выбранного Minecraft.",
                                    "The Fabric API has no compatible loader for this Minecraft version.",
                                )
                                .to_string()
                        }
                    } else {
                        locale
                            .text(
                                "Для этой версии нет доступной версии загрузчика.",
                                "No compatible loader version is available.",
                            )
                            .to_string()
                    }
                })
            } else {
                Some(
                    locale
                        .text(
                            "Для этой версии игры нет доступной версии загрузчика.",
                            "No loader version is available for this game version.",
                        )
                        .to_string(),
                )
            }
        } else {
            None
        };
        let manifest_errors = self.versions.error().map(|error| {
            div()
                .text_size(px(10.))
                .text_color(rgb(palette.muted))
                .child(error.to_owned())
        });

        div()
            .v_flex()
            .w_full()
            .gap_3()
            .text_color(rgb(palette.foreground))
            .child(field_label(locale.text("Название", "Name")))
            .child(
                Input::new(profile_name_input)
                    .w_full()
                    .h(px(40.))
                    .px_3()
                    .rounded(px(8.))
                    .bg(rgb(palette.control))
                    .border_1()
                    .border_color(rgb(palette.border)),
            )
            .child(field_label(locale.text("Загрузчик", "Loader")))
            .child(div().h_flex().w_full().gap_2().children(loader_buttons))
            .child(field_label(locale.text("Версия игры", "Game version")))
            .child(
                UiButton::new("manifest-game-version-dropdown")
                    .w_full()
                    .h(px(40.))
                    .px_3()
                    .rounded(px(8.))
                    .border_1()
                    .border_color(rgb(palette.border))
                    .bg(rgb(palette.control))
                    .text_color(rgb(palette.foreground))
                    .hover(|style| style.bg(rgb(palette.hover)))
                    .on_click(move |_, _, cx| {
                        if let Some(owner) = open_versions_owner.upgrade() {
                            let _ = owner.update(cx, |this, cx| {
                                let opening = !this.versions.game_version_menu_open();
                                if opening {
                                    let selected_index = this
                                        .versions
                                        .game_version()
                                        .and_then(|selected| {
                                            this.versions.manifest().and_then(|manifest| {
                                                manifest
                                                    .versions
                                                    .iter()
                                                    .position(|version| version.id == selected)
                                            })
                                        })
                                        .unwrap_or(0);
                                    this.manifest_scroll
                                        .scroll_to_item(selected_index, ScrollStrategy::Center);
                                }
                                this.versions.toggle_game_version_menu();
                                cx.notify();
                            });
                        }
                    })
                    .child(
                        div()
                            .h_flex()
                            .w_full()
                            .items_center()
                            .justify_between()
                            .child(game_version_label)
                            .child(IconName::ChevronDown),
                    ),
            )
            .children(game_menu)
            .when(show_loader_version, |this| {
                this.child(field_label(
                    locale.text("Версия загрузчика", "Loader version"),
                ))
                .child(div().h_flex().w_full().gap_2().children(mode_buttons))
                .when(show_custom_loader_version, |this| {
                    this.child(
                        Input::new(loader_version_input)
                            .w_full()
                            .h(px(40.))
                            .px_3()
                            .rounded(px(8.))
                            .bg(rgb(palette.control))
                            .border_1()
                            .border_color(rgb(palette.border)),
                    )
                })
            })
            .when_some(loader_version_feedback, |this, feedback| {
                this.child(
                    div()
                        .text_size(px(11.))
                        .text_color(rgb(palette.muted))
                        .child(feedback),
                )
            })
            .children(manifest_errors)
            .child(
                UiButton::new("create-minecraft-profile")
                    .w_full()
                    .px_3()
                    .py_2()
                    .rounded(px(8.))
                    .bg(rgb(palette.accent))
                    .text_color(rgb(palette.accent_foreground))
                    .hover(|style| style.opacity(0.9))
                    .on_click(move |_, window, cx| {
                        if let Some(owner) = create_owner.upgrade() {
                            let Some(game_version) =
                                owner.read(cx).versions.game_version().map(str::to_owned)
                            else {
                                return;
                            };
                            let name = profile_input.read(cx).value().trim().to_owned();
                            let custom_loader_version =
                                loader_input.read(cx).value().trim().to_owned();
                            let loader = owner.read(cx).versions.loader_choice();
                            let loader_version = if loader == LoaderChoice::Vanilla {
                                None
                            } else {
                                owner
                                    .read(cx)
                                    .versions
                                    .loader_version(Some(&custom_loader_version))
                            };
                            if loader != LoaderChoice::Vanilla && loader_version.is_none() {
                                return;
                            }
                            let _ = owner.update(cx, move |this, cx| {
                                let loader_name = match loader {
                                    LoaderChoice::Vanilla => "Vanilla",
                                    LoaderChoice::Fabric => "Fabric",
                                    LoaderChoice::NeoForge => "NeoForge",
                                    LoaderChoice::Forge => "Forge",
                                };
                                let profile_name = if name.is_empty() {
                                    format!("{loader_name} {game_version}")
                                } else {
                                    name
                                };
                                let selection = if loader == LoaderChoice::Vanilla {
                                    format!("{profile_name} · Minecraft {game_version}")
                                } else {
                                    let Some(version) = loader_version.as_deref() else {
                                        return;
                                    };
                                    format!(
                                        "{profile_name} · {game_version} · {loader_name} {version}"
                                    )
                                };
                                let installation_root = this.settings.download_directory().clone();
                                let profile = InstanceProfile::new(
                                    profile_name,
                                    game_version,
                                    loader,
                                    loader_version,
                                    installation_root,
                                );
                                let profile_id = profile.id;
                                this.instances.add(profile);
                                this.versions.select(selection);
                                this.versions.close_game_version_menu();
                                if let Err(error) = profile_store::save(this.instances.profiles()) {
                                    tracing::error!(%error, "failed to save Minecraft profile");
                                }
                                this.start_profile_install(profile_id, cx);
                                cx.notify();
                            });
                            window.close_dialog(cx);
                        }
                    })
                    .child(locale.text("Создать профиль", "Create profile")),
            )
            .into_any_element()
    }

    fn search_bar(&self, palette: Palette, locale: Locale) -> AnyElement {
        Input::new(&self.search_input)
            .w_full()
            .h(px(40.))
            .px_3()
            .rounded(px(8.))
            .bg(rgb(palette.surface))
            .border_1()
            .border_color(rgb(palette.border))
            .prefix(IconName::Search)
            .aria_label(locale.text("Поиск по каталогу", "Search catalog"))
            .into_any_element()
    }

    fn navigation_button(
        &self,
        page: Page,
        palette: Palette,
        locale: Locale,
        cx: &Context<Self>,
    ) -> AnyElement {
        let is_selected = self.active_page == page
            || (page == Page::Mods && self.active_page == Page::ModDetails);
        let is_collapsed = self.sidebar.is_collapsed();
        let title = page.title(locale);
        let content = if is_collapsed {
            div()
                .w_full()
                .flex()
                .justify_center()
                .child(page.icon())
                .into_any_element()
        } else {
            div()
                .w_full()
                .h_flex()
                .items_center()
                .gap_3()
                .child(page.icon())
                .child(title)
                .into_any_element()
        };

        let button = UiButton::new(page.element_id())
            .w_full()
            .px_3()
            .py_3()
            .rounded(px(8.))
            .bg(rgb(if is_selected {
                palette.selected
            } else {
                palette.sidebar
            }))
            .text_color(rgb(if is_selected {
                palette.accent
            } else {
                palette.foreground
            }))
            .hover(|style| {
                style.bg(rgb(if is_selected {
                    palette.selected_hover
                } else {
                    palette.hover
                }))
            })
            .active(|style| style.bg(rgb(palette.pressed)))
            .selected(is_selected)
            .accessibility_label(title)
            .on_click(cx.listener(move |this, _, window, cx| {
                this.navigate_to(page, window, cx);
            }))
            .child(content);

        let button = if is_collapsed {
            button.tooltip(move |window, cx| Tooltip::new(title).build(window, cx))
        } else {
            button
        };

        button.into_any_element()
    }

    fn sidebar_toggle_button(
        &self,
        palette: Palette,
        locale: Locale,
        cx: &Context<Self>,
    ) -> AnyElement {
        let is_collapsed = self.sidebar.is_collapsed();
        let title = if is_collapsed {
            locale.text("Развернуть панель", "Expand sidebar")
        } else {
            locale.text("Свернуть панель", "Collapse sidebar")
        };

        UiButton::new("sidebar-toggle")
            .w(px(24.))
            .h(px(24.))
            .rounded(px(6.))
            .bg(rgb(palette.sidebar))
            .text_color(rgb(palette.muted))
            .hover(|style| {
                style
                    .bg(rgb(palette.hover))
                    .text_color(rgb(palette.foreground))
            })
            .active(|style| style.bg(rgb(palette.pressed)))
            .accessibility_label(title)
            .tooltip(move |window, cx| Tooltip::new(title).build(window, cx))
            .on_click(cx.listener(|this, _, _, cx| {
                this.sidebar.toggle();
                cx.notify();
            }))
            .child(if is_collapsed {
                IconName::PanelLeftOpen
            } else {
                IconName::PanelLeftClose
            })
            .into_any_element()
    }

    fn home_page(&self, palette: Palette, locale: Locale, cx: &Context<Self>) -> AnyElement {
        let destinations: Vec<_> = [Page::Modpacks, Page::Mods, Page::ResourcePacks]
            .into_iter()
            .map(|page| self.destination_card(page, palette, locale, cx))
            .collect();

        let popular_cards: Vec<_> = SAMPLE_MODS
            .iter()
            .take(3)
            .map(|sample| self.home_mod_card(sample, palette, locale))
            .collect();

        div()
            .v_flex()
            .w_full()
            .flex_1()
            .min_h_0()
            .overflow_y_scrollbar()
            .gap_5()
            .child(
                div()
                    .h_flex()
                    .w_full()
                    .items_center()
                    .gap_5()
                    .p_6()
                    .rounded(px(12.))
                    .bg(rgb(palette.surface))
                    .border_1()
                    .border_color(rgb(palette.border))
                    .child(
                        div()
                            .v_flex()
                            .flex_1()
                            .gap_3()
                            .child(
                                div()
                                    .text_size(px(22.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(palette.foreground))
                                    .child(locale.text(
                                        "Играй в Minecraft по-своему",
                                        "Make Minecraft yours",
                                    )),
                            )
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .text_color(rgb(palette.muted))
                                    .child(locale.text(
                                        "Моды, сборки и ресурспаки — в одном лаунчере.",
                                        "Mods, modpacks, and resource packs in one launcher.",
                                    )),
                            )
                            .child(
                                UiButton::new("home-browse-mods")
                                    .px_4()
                                    .py_3()
                                    .rounded(px(8.))
                                    .bg(rgb(palette.accent))
                                    .text_color(rgb(palette.accent_foreground))
                                    .hover(|style| style.opacity(0.9))
                                    .active(|style| style.opacity(0.78))
                                    .accessibility_label(
                                        locale.text("Открыть каталог модов", "Browse mods"),
                                    )
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.navigate_to(Page::Mods, window, cx);
                                    }))
                                    .child(
                                        div()
                                            .h_flex()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                locale.text("Открыть каталог модов", "Browse mods"),
                                            )
                                            .child(IconName::ArrowRight),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .w(px(112.))
                            .h(px(112.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(14.))
                            .bg(rgb(palette.accent))
                            .text_color(rgb(palette.accent_foreground))
                            .text_size(px(58.))
                            .child(IconName::Blocks),
                    ),
            )
            .child(
                div()
                    .h_flex()
                    .w_full()
                    .gap_3()
                    .children(destinations)
                    .child(self.add_version_card(palette, locale, cx)),
            )
            .when(!self.instances.profiles().is_empty(), |this| {
                this.child(self.instances_section(palette, locale, cx))
            })
            .child(
                div()
                    .v_flex()
                    .gap_3()
                    .child(
                        div()
                            .h_flex()
                            .items_center()
                            .gap_2()
                            .text_color(rgb(palette.foreground))
                            .font_weight(FontWeight::BOLD)
                            .child(IconName::Star)
                            .child(locale.text("Популярные моды", "Popular mods")),
                    )
                    .child(div().h_flex().w_full().gap_3().children(popular_cards)),
            )
            .into_any_element()
    }

    fn instances_section(
        &self,
        palette: Palette,
        locale: Locale,
        cx: &Context<Self>,
    ) -> AnyElement {
        let cards = self
            .instances
            .profiles()
            .iter()
            .map(|profile| self.instance_card(profile, palette, locale, cx))
            .collect::<Vec<_>>();

        div()
            .v_flex()
            .w_full()
            .gap_3()
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .gap_2()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(palette.foreground))
                    .child(IconName::Box)
                    .child(locale.text("Мои профили", "My profiles")),
            )
            .children(cards)
            .into_any_element()
    }

    fn instance_card(
        &self,
        profile: &InstanceProfile,
        palette: Palette,
        locale: Locale,
        cx: &Context<Self>,
    ) -> AnyElement {
        let runtime = self.instances.runtime(&profile.id);
        let is_busy = matches!(
            &runtime,
            InstanceRuntime::Installing { .. }
                | InstanceRuntime::Launching
                | InstanceRuntime::Running { .. }
                | InstanceRuntime::Removing
        );
        let is_installed = profile.installed_version_id.is_some();
        let (status_text, status_detail, action_text) = match &runtime {
            InstanceRuntime::NotInstalled => (
                locale.text("Не установлена", "Not installed").to_string(),
                locale
                    .text("Нажми, чтобы скачать игру.", "Click to download the game.")
                    .to_string(),
                locale.text("Скачать", "Download"),
            ),
            InstanceRuntime::Installing {
                stage,
                task,
                received,
                total,
                completed_tasks,
            } => {
                let detail = if let Some(total) = total.filter(|total| *total > 0) {
                    format!(
                        "{} · {} · {} / {}",
                        stage,
                        task.as_deref().unwrap_or(""),
                        format_bytes(*received),
                        format_bytes(total),
                    )
                } else {
                    format!(
                        "{} · {} · {} {}",
                        stage,
                        task.as_deref().unwrap_or(""),
                        completed_tasks,
                        locale.text("файлов готово", "files complete"),
                    )
                };
                (
                    locale.text("Скачивание", "Downloading").to_string(),
                    detail,
                    locale.text("Скачивание…", "Downloading…"),
                )
            }
            InstanceRuntime::Ready => (
                locale.text("Готова к запуску", "Ready to play").to_string(),
                locale
                    .text(
                        "Все игровые файлы установлены.",
                        "All game files are installed.",
                    )
                    .to_string(),
                locale.text("Играть", "Play"),
            ),
            InstanceRuntime::Launching => (
                locale.text("Запуск игры", "Starting game").to_string(),
                locale
                    .text("Подготавливаем процесс Java…", "Starting the Java process…")
                    .to_string(),
                locale.text("Запуск…", "Starting…"),
            ),
            InstanceRuntime::Running { process_id } => (
                locale.text("Игра запущена", "Game is running").to_string(),
                format!("{} {process_id}", locale.text("Процесс", "Process")),
                locale.text("Игра запущена", "Running"),
            ),
            InstanceRuntime::Removing => (
                locale
                    .text("Удаление сборки", "Deleting profile")
                    .to_string(),
                locale
                    .text("Удаляем локальные данные…", "Removing profile data…")
                    .to_string(),
                locale.text("Удаление…", "Deleting…"),
            ),
            InstanceRuntime::RemovalFailed { message } => (
                locale
                    .text("Не удалось удалить", "Could not delete")
                    .to_string(),
                message.clone(),
                if is_installed {
                    locale.text("Играть", "Play")
                } else {
                    locale.text("Повторить скачивание", "Retry download")
                },
            ),
            InstanceRuntime::Failed { message } => (
                locale
                    .text(
                        if is_installed {
                            "Не удалось запустить"
                        } else {
                            "Не удалось скачать"
                        },
                        if is_installed {
                            "Could not launch"
                        } else {
                            "Download failed"
                        },
                    )
                    .to_string(),
                message.clone(),
                if is_installed {
                    locale.text("Играть снова", "Play again")
                } else {
                    locale.text("Повторить скачивание", "Retry download")
                },
            ),
        };
        let loader = loader_title(profile.loader);
        let profile_version = profile.loader_version.as_deref().unwrap_or("");
        let profile_details = if profile.loader == LoaderChoice::Vanilla {
            format!("Minecraft {} · {loader}", profile.game_version)
        } else {
            format!(
                "Minecraft {} · {loader} {profile_version}",
                profile.game_version
            )
        };
        let profile_id = profile.id;

        div()
            .v_flex()
            .w_full()
            .gap_3()
            .p_4()
            .rounded(px(10.))
            .bg(rgb(palette.surface))
            .border_1()
            .border_color(rgb(palette.border))
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .v_flex()
                            .min_w_0()
                            .flex_1()
                            .gap_1()
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(palette.foreground))
                                    .child(profile.name.clone()),
                            )
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(rgb(palette.muted))
                                    .child(profile_details),
                            ),
                    )
                    .child(
                        div()
                            .px_2()
                            .py_1()
                            .rounded(px(6.))
                            .bg(rgb(palette.control))
                            .text_size(px(10.))
                            .text_color(rgb(if is_installed {
                                palette.accent
                            } else {
                                palette.muted
                            }))
                            .child(status_text),
                    ),
            )
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(rgb(palette.muted))
                    .child(status_detail),
            )
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .justify_end()
                    .gap_2()
                    .child(
                        UiButton::new(format!("play-instance-{profile_id}"))
                            .px_4()
                            .py_2()
                            .rounded(px(8.))
                            .bg(rgb(palette.accent))
                            .text_color(rgb(palette.accent_foreground))
                            .hover(|style| style.opacity(0.9))
                            .disabled(is_busy)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.start_profile_launch(profile_id, cx);
                            }))
                            .child(
                                div()
                                    .h_flex()
                                    .items_center()
                                    .gap_2()
                                    .child(if is_installed {
                                        IconName::Play
                                    } else {
                                        IconName::Download
                                    })
                                    .child(action_text),
                            ),
                    )
                    .child(
                        UiButton::new(format!("settings-instance-{profile_id}"))
                            .px_3()
                            .py_2()
                            .rounded(px(8.))
                            .bg(rgb(palette.control))
                            .text_color(rgb(palette.muted))
                            .hover(|style| style.bg(rgb(palette.hover)))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.open_profile_settings(profile_id, window, cx);
                            }))
                            .child(IconName::Settings),
                    )
                    .child(
                        UiButton::new(format!("delete-instance-{profile_id}"))
                            .px_3()
                            .py_2()
                            .rounded(px(8.))
                            .bg(rgb(palette.control))
                            .text_color(rgb(0xef6a65))
                            .hover(|style| style.bg(rgb(0x3b1b1b)))
                            .disabled(is_busy)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.open_delete_profile_confirmation(profile_id, window, cx);
                            }))
                            .child(IconName::Delete),
                    ),
            )
            .into_any_element()
    }

    fn destination_card(
        &self,
        page: Page,
        palette: Palette,
        locale: Locale,
        cx: &Context<Self>,
    ) -> AnyElement {
        let description = match page {
            Page::Modpacks => locale.text("Готовые сборки", "Curated collections"),
            Page::Mods => locale.text("Дополнения для игры", "Add-ons for your game"),
            Page::ResourcePacks => locale.text("Новый стиль мира", "A new look for your world"),
            _ => "",
        };

        UiButton::new(format!("home-{}", page.element_id()))
            .flex_1()
            .px_4()
            .py_4()
            .rounded(px(10.))
            .bg(rgb(palette.surface))
            .border_1()
            .border_color(rgb(palette.border))
            .text_color(rgb(palette.foreground))
            .hover(|style| style.bg(rgb(palette.hover)))
            .active(|style| style.bg(rgb(palette.pressed)))
            .accessibility_label(page.title(locale))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.navigate_to(page, window, cx);
            }))
            .child(
                div()
                    .v_flex()
                    .w_full()
                    .items_start()
                    .gap_3()
                    .child(
                        div()
                            .text_size(px(19.))
                            .text_color(rgb(palette.accent))
                            .child(page.icon()),
                    )
                    .child(
                        div()
                            .v_flex()
                            .items_start()
                            .gap_1()
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .font_weight(FontWeight::BOLD)
                                    .child(page.title(locale)),
                            )
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(rgb(palette.muted))
                                    .child(description),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn add_version_card(&self, palette: Palette, locale: Locale, cx: &Context<Self>) -> AnyElement {
        let detail = self.versions.selected_version().map_or_else(
            || {
                locale
                    .text("Добавить версию игры", "Add a game version")
                    .to_string()
            },
            str::to_owned,
        );

        UiButton::new("home-add-minecraft-version")
            .flex_1()
            .px_4()
            .py_4()
            .rounded(px(10.))
            .bg(rgb(palette.surface))
            .border_1()
            .border_color(rgb(palette.border))
            .text_color(rgb(palette.foreground))
            .hover(|style| style.bg(rgb(palette.hover)))
            .active(|style| style.bg(rgb(palette.pressed)))
            .accessibility_label(locale.text("Добавить Minecraft", "Add Minecraft"))
            .on_click(cx.listener(|this, _, window, cx| {
                this.open_version_picker(window, cx);
            }))
            .child(
                div()
                    .v_flex()
                    .w_full()
                    .items_start()
                    .gap_3()
                    .child(
                        div()
                            .text_size(px(19.))
                            .text_color(rgb(palette.accent))
                            .child(IconName::Plus),
                    )
                    .child(
                        div()
                            .v_flex()
                            .items_start()
                            .gap_1()
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .font_weight(FontWeight::BOLD)
                                    .child(locale.text("Добавить", "Add")),
                            )
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(rgb(palette.muted))
                                    .child(detail),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn home_mod_card(&self, sample: &ModSample, palette: Palette, locale: Locale) -> AnyElement {
        div()
            .v_flex()
            .flex_1()
            .min_w_0()
            .gap_3()
            .p_3()
            .rounded(px(10.))
            .bg(rgb(palette.surface))
            .border_1()
            .border_color(rgb(palette.border))
            .child(mod_thumbnail(sample, px(34.)))
            .child(
                div()
                    .v_flex()
                    .gap_1()
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(palette.foreground))
                            .child(sample.name),
                    )
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(rgb(palette.muted))
                            .child(locale.text("от ", "by "))
                            .child(sample.author),
                    ),
            )
            .into_any_element()
    }

    fn mods_page(&self, palette: Palette, locale: Locale, cx: &Context<Self>) -> AnyElement {
        let selected_category = self.mods.category();
        let selected_sort = self.mods.sort();
        let categories: Vec<_> = ModCategory::ALL
            .into_iter()
            .map(|category| {
                let is_selected = category == selected_category;
                UiButton::new(category.id())
                    .px_3()
                    .py_2()
                    .rounded(px(16.))
                    .bg(rgb(if is_selected {
                        palette.accent
                    } else {
                        palette.control
                    }))
                    .text_color(rgb(if is_selected {
                        palette.accent_foreground
                    } else {
                        palette.muted
                    }))
                    .hover(|style| style.bg(rgb(palette.hover)))
                    .active(|style| style.opacity(0.8))
                    .selected(is_selected)
                    .accessibility_label(category.title(locale))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.mods.set_category(category);
                        cx.notify();
                    }))
                    .child(category.title(locale))
                    .into_any_element()
            })
            .collect();

        let sort_owner = cx.entity().downgrade();
        let sort_menu = ComponentButton::new("mods-sort")
            .outline()
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .gap_2()
                    .child(IconName::ArrowDown)
                    .child(selected_sort.title(locale))
                    .child(IconName::ChevronDown),
            )
            .dropdown_menu(move |menu, _, _| {
                let sort_owner = sort_owner.clone();
                let sort_owner_downloads = sort_owner.clone();
                let sort_owner_name = sort_owner.clone();
                menu.item(
                    PopupMenuItem::new(ModSort::Popular.title(locale))
                        .checked(selected_sort == ModSort::Popular)
                        .on_click(move |_, _, cx| {
                            let _ = sort_owner.update(cx, |this, cx| {
                                this.mods.set_sort(ModSort::Popular);
                                cx.notify();
                            });
                        }),
                )
                .item(
                    PopupMenuItem::new(ModSort::Downloads.title(locale))
                        .checked(selected_sort == ModSort::Downloads)
                        .on_click(move |_, _, cx| {
                            let _ = sort_owner_downloads.update(cx, |this, cx| {
                                this.mods.set_sort(ModSort::Downloads);
                                cx.notify();
                            });
                        }),
                )
                .item(
                    PopupMenuItem::new(ModSort::Name.title(locale))
                        .checked(selected_sort == ModSort::Name)
                        .on_click(move |_, _, cx| {
                            let _ = sort_owner_name.update(cx, |this, cx| {
                                this.mods.set_sort(ModSort::Name);
                                cx.notify();
                            });
                        }),
                )
            });

        let cards: Vec<_> = self
            .mods
            .visible_mods()
            .into_iter()
            .map(|sample| self.mod_card(sample, palette, locale, cx))
            .collect();
        let cards_content = if cards.is_empty() {
            div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .text_color(rgb(palette.muted))
                .child(locale.text(
                    "В этой категории пока нет модов",
                    "No mods in this category yet",
                ))
                .into_any_element()
        } else {
            div()
                .v_flex()
                .flex_1()
                .min_h_0()
                .pr_4()
                .overflow_y_scrollbar()
                .gap_2()
                .children(cards)
                .into_any_element()
        };

        div()
            .v_flex()
            .w_full()
            .flex_1()
            .min_h_0()
            .gap_4()
            .child(self.search_bar(palette, locale))
            .child(
                div()
                    .h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(rgb(palette.muted))
                            .child(locale.text("Примеры модов", "Sample mods")),
                    )
                    .child(sort_menu),
            )
            .child(
                div()
                    .h_flex()
                    .w_full()
                    .h(px(38.))
                    .gap_2()
                    .overflow_x_scrollbar()
                    .children(categories),
            )
            .child(cards_content)
            .into_any_element()
    }

    fn mod_card(
        &self,
        sample: &ModSample,
        palette: Palette,
        locale: Locale,
        cx: &Context<Self>,
    ) -> AnyElement {
        let description = sample.description(locale);
        let sample_id = sample.id;
        let tags: Vec<_> = sample
            .categories
            .iter()
            .map(|category| {
                div()
                    .px_2()
                    .py_1()
                    .rounded(px(12.))
                    .bg(rgb(palette.control))
                    .text_size(px(10.))
                    .text_color(rgb(palette.muted))
                    .child(category.title(locale))
                    .into_any_element()
            })
            .collect();

        let details_button = UiButton::new(format!("open-mod-{}", sample.id.element_id()))
            .flex_1()
            .px_3()
            .py_2()
            .rounded(px(8.))
            .text_color(rgb(palette.foreground))
            .accessibility_label(locale.text("Открыть страницу мода", "Open mod details"))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.mods.select_mod(sample_id);
                this.navigate_to(Page::ModDetails, window, cx);
            }))
            .child(
                div()
                    .h_flex()
                    .w_full()
                    .items_center()
                    .gap_3()
                    .child(mod_thumbnail(sample, px(56.)))
                    .child(
                        div()
                            .v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_1()
                            .items_start()
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(palette.foreground))
                                    .child(sample.name),
                            )
                            .child(
                                div()
                                    .id(format!("mod-description-{}", sample.id.element_id()))
                                    .w_full()
                                    .text_size(px(11.))
                                    .text_color(rgb(palette.muted))
                                    .truncate()
                                    .tooltip(move |window, cx| {
                                        Tooltip::new(description).build(window, cx)
                                    })
                                    .child(description),
                            )
                            .child(
                                div()
                                    .h_flex()
                                    .items_center()
                                    .gap_3()
                                    .text_size(px(10.))
                                    .text_color(rgb(palette.muted))
                                    .child(
                                        div()
                                            .h_flex()
                                            .items_center()
                                            .gap_1()
                                            .child(IconName::User)
                                            .child(sample.author),
                                    )
                                    .child(
                                        div()
                                            .h_flex()
                                            .items_center()
                                            .gap_1()
                                            .child(IconName::ArrowDown)
                                            .child(sample.downloads_label),
                                    )
                                    .child(
                                        div()
                                            .h_flex()
                                            .items_center()
                                            .gap_1()
                                            .child(IconName::Heart)
                                            .child(sample.likes_label),
                                    )
                                    .children(tags),
                            ),
                    ),
            );

        div()
            .h_flex()
            .w_full()
            .items_center()
            .gap_3()
            .rounded(px(10.))
            .bg(rgb(palette.surface))
            .hover(|style| style.bg(rgb(palette.hover)))
            .border_1()
            .border_color(rgb(palette.border))
            .child(details_button)
            .child(
                div()
                    .v_flex()
                    .items_end()
                    .gap_2()
                    .pr_3()
                    .py_2()
                    .child(
                        div()
                            .px_2()
                            .py_1()
                            .rounded(px(6.))
                            .bg(rgb(palette.control))
                            .text_size(px(10.))
                            .text_color(rgb(palette.muted))
                            .child(sample.game_version),
                    )
                    .child(
                        ComponentButton::new(format!("install-{}", sample.name))
                            .primary()
                            .label(locale.text("Установить", "Install")),
                    ),
            )
            .into_any_element()
    }

    fn mod_details_page(&self, palette: Palette, locale: Locale, cx: &Context<Self>) -> AnyElement {
        let Some(sample) = self.mods.selected_mod() else {
            return self.empty_page(palette, locale);
        };
        let active_tab = self.mods.detail_tab();

        let back_button = UiButton::new("mod-details-back")
            .px_2()
            .py_2()
            .rounded(px(7.))
            .bg(rgb(palette.background))
            .text_color(rgb(palette.muted))
            .hover(|style| style.bg(rgb(palette.hover)))
            .accessibility_label(locale.text("Назад к модам", "Back to mods"))
            .on_click(cx.listener(|this, _, window, cx| {
                this.mods.clear_selected_mod();
                this.navigate_to(Page::Mods, window, cx);
            }))
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .gap_2()
                    .child(IconName::ArrowLeft)
                    .child(locale.text("Назад", "Back")),
            );

        let category_tags: Vec<_> = sample
            .categories
            .iter()
            .map(|category| {
                div()
                    .px_2()
                    .py_1()
                    .rounded(px(12.))
                    .bg(rgb(palette.control))
                    .text_size(px(10.))
                    .text_color(rgb(palette.muted))
                    .child(category.title(locale))
                    .into_any_element()
            })
            .collect();

        let hero = div()
            .h_flex()
            .w_full()
            .items_center()
            .gap_4()
            .p_4()
            .rounded(px(10.))
            .bg(rgb(palette.surface))
            .border_1()
            .border_color(rgb(palette.border))
            .child(mod_thumbnail(sample, px(76.)))
            .child(
                div()
                    .v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_2()
                    .child(
                        div()
                            .text_size(px(21.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(palette.foreground))
                            .child(sample.name),
                    )
                    .child(
                        div()
                            .h_flex()
                            .items_center()
                            .gap_3()
                            .text_size(px(11.))
                            .text_color(rgb(palette.muted))
                            .child(sample.author)
                            .child(
                                div()
                                    .h_flex()
                                    .items_center()
                                    .gap_1()
                                    .child(IconName::ArrowDown)
                                    .child(sample.downloads_label),
                            )
                            .child(
                                div()
                                    .h_flex()
                                    .items_center()
                                    .gap_1()
                                    .child(IconName::Heart)
                                    .child(sample.likes_label),
                            ),
                    )
                    .child(div().h_flex().gap_2().children(category_tags)),
            )
            .child(
                div()
                    .v_flex()
                    .items_end()
                    .gap_2()
                    .child(
                        div()
                            .px_2()
                            .py_1()
                            .rounded(px(6.))
                            .bg(rgb(palette.control))
                            .text_size(px(10.))
                            .text_color(rgb(palette.muted))
                            .child(sample.game_version),
                    )
                    .child(
                        UiButton::new(format!("details-install-{}", sample.id.element_id()))
                            .px_3()
                            .py_2()
                            .rounded(px(7.))
                            .bg(rgb(palette.accent))
                            .text_color(rgb(palette.accent_foreground))
                            .hover(|style| style.opacity(0.9))
                            .accessibility_label(locale.text("Установить мод", "Install mod"))
                            .child(locale.text("Установить", "Install")),
                    ),
            );

        let tabs: Vec<_> = ModDetailTab::ALL
            .into_iter()
            .map(|tab| {
                let is_selected = tab == active_tab;
                let title = if tab == ModDetailTab::Files {
                    format!("{} ({})", tab.title(locale), sample.files_count)
                } else {
                    tab.title(locale).to_owned()
                };

                UiButton::new(tab.id())
                    .px_3()
                    .py_2()
                    .rounded(px(7.))
                    .bg(rgb(if is_selected {
                        palette.selected
                    } else {
                        palette.background
                    }))
                    .text_color(rgb(if is_selected {
                        palette.accent
                    } else {
                        palette.muted
                    }))
                    .hover(|style| style.bg(rgb(palette.hover)))
                    .selected(is_selected)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.mods.set_detail_tab(tab);
                        cx.notify();
                    }))
                    .child(title)
                    .into_any_element()
            })
            .collect();

        let tab_content = self.mod_detail_content(sample, active_tab, palette, locale);
        let metadata = div()
            .v_flex()
            .w(px(204.))
            .gap_3()
            .p_4()
            .rounded(px(10.))
            .bg(rgb(palette.surface))
            .border_1()
            .border_color(rgb(palette.border))
            .child(detail_meta_row(
                IconName::Calendar,
                locale.text("Последнее обновление", "Last updated"),
                sample.updated,
                palette,
            ))
            .child(detail_meta_row(
                IconName::HardDrive,
                locale.text("Размер", "File size"),
                sample.file_size,
                palette,
            ))
            .child(detail_meta_row(
                IconName::FileText,
                locale.text("Лицензия", "License"),
                sample.license,
                palette,
            ))
            .child(detail_meta_row(
                IconName::Blocks,
                locale.text("Игровые версии", "Game versions"),
                sample.game_version,
                palette,
            ))
            .child(detail_meta_row(
                IconName::Cpu,
                locale.text("Загрузчики", "Loaders"),
                sample.loaders,
                palette,
            ));

        div()
            .v_flex()
            .w_full()
            .flex_1()
            .min_h_0()
            .gap_3()
            .child(back_button)
            .child(hero)
            .child(div().h_flex().w_full().gap_2().children(tabs))
            .child(
                div()
                    .h_flex()
                    .w_full()
                    .flex_1()
                    .min_h_0()
                    .gap_3()
                    .child(
                        div()
                            .v_flex()
                            .flex_1()
                            .min_h_0()
                            .pr_4()
                            .overflow_y_scrollbar()
                            .child(tab_content),
                    )
                    .child(metadata),
            )
            .into_any_element()
    }

    fn mod_detail_content(
        &self,
        sample: &ModSample,
        tab: ModDetailTab,
        palette: Palette,
        locale: Locale,
    ) -> AnyElement {
        match tab {
            ModDetailTab::Description => div()
                .v_flex()
                .gap_4()
                .child(
                    div()
                        .v_flex()
                        .gap_2()
                        .p_4()
                        .rounded(px(10.))
                        .bg(rgb(palette.surface))
                        .border_1()
                        .border_color(rgb(palette.border))
                        .child(
                            div()
                                .text_size(px(15.))
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgb(palette.foreground))
                                .child(locale.text("Описание", "Description")),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(rgb(palette.muted))
                                .child(sample.description(locale)),
                        ),
                )
                .child(self.mod_screenshots(sample, palette, locale))
                .into_any_element(),
            ModDetailTab::Files => {
                let files: Vec<_> = sample
                    .loaders
                    .split(", ")
                    .enumerate()
                    .map(|(index, loader)| {
                        div()
                            .h_flex()
                            .w_full()
                            .items_center()
                            .gap_3()
                            .p_3()
                            .rounded(px(8.))
                            .bg(rgb(palette.surface))
                            .border_1()
                            .border_color(rgb(palette.border))
                            .child(IconName::File)
                            .child(
                                div()
                                    .v_flex()
                                    .flex_1()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .text_color(rgb(palette.foreground))
                                            .child(format!(
                                                "{}-{}-{}.jar",
                                                sample.id.element_id(),
                                                sample.game_version,
                                                loader.to_lowercase()
                                            )),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(10.))
                                            .text_color(rgb(palette.muted))
                                            .child(format!(
                                                "{} · {} · {}",
                                                loader, sample.updated, sample.file_size
                                            )),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(10.))
                                    .text_color(rgb(palette.muted))
                                    .child(format!("v{}", index + 1)),
                            )
                            .into_any_element()
                    })
                    .collect();

                div().v_flex().gap_2().children(files).into_any_element()
            }
            ModDetailTab::Dependencies => div()
                .v_flex()
                .gap_3()
                .child(detail_meta_row(
                    IconName::Blocks,
                    locale.text("Игра", "Game"),
                    sample.game_version,
                    palette,
                ))
                .child(detail_meta_row(
                    IconName::Cpu,
                    locale.text("Поддерживаемые загрузчики", "Supported loaders"),
                    sample.loaders,
                    palette,
                ))
                .into_any_element(),
            ModDetailTab::Screenshots => self.mod_screenshots(sample, palette, locale),
            ModDetailTab::Similar => {
                let similar: Vec<_> = SAMPLE_MODS
                    .iter()
                    .filter(|candidate| candidate.id != sample.id)
                    .take(3)
                    .map(|candidate| self.home_mod_card(candidate, palette, locale))
                    .collect();
                div()
                    .h_flex()
                    .w_full()
                    .gap_3()
                    .children(similar)
                    .into_any_element()
            }
        }
    }

    fn mod_screenshots(&self, sample: &ModSample, palette: Palette, locale: Locale) -> AnyElement {
        let thumbnails: Vec<_> = [0, 1, 2]
            .into_iter()
            .map(|index| {
                if index == 0 && sample.image_url.is_some() {
                    mod_thumbnail(sample, px(112.))
                } else {
                    div()
                        .flex_1()
                        .h(px(112.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(8.))
                        .bg(rgb(if index == 0 {
                            sample.icon_background
                        } else if index == 1 {
                            palette.selected
                        } else {
                            palette.control
                        }))
                        .text_color(rgb(if index == 0 {
                            sample.icon_foreground
                        } else {
                            palette.accent
                        }))
                        .text_size(px(30.))
                        .child(sample.icon)
                        .into_any_element()
                }
            })
            .collect();

        div()
            .v_flex()
            .gap_3()
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .gap_2()
                    .text_size(px(14.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(palette.foreground))
                    .child(IconName::GalleryVerticalEnd)
                    .child(locale.text("Скриншоты", "Screenshots")),
            )
            .child(div().h_flex().w_full().gap_2().children(thumbnails))
            .into_any_element()
    }

    fn empty_page(&self, palette: Palette, locale: Locale) -> AnyElement {
        div()
            .text_size(px(13.))
            .text_color(rgb(palette.muted))
            .child(locale.text("Здесь пока пусто", "Nothing here yet"))
            .into_any_element()
    }

    fn search_empty_page(&self, palette: Palette, locale: Locale) -> AnyElement {
        let message = if self.search_query.trim().is_empty() {
            locale.text("Раздел пока пуст", "This section is empty for now")
        } else {
            locale.text("Ничего не найдено", "No results found")
        };

        div()
            .v_flex()
            .w_full()
            .flex_1()
            .min_h_0()
            .gap_4()
            .child(self.search_bar(palette, locale))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(rgb(palette.muted))
                    .child(message),
            )
            .into_any_element()
    }

    fn settings_page(&self, palette: Palette, cx: &mut Context<Self>) -> AnyElement {
        let locale = self.settings.locale();
        let theme = self.settings.theme();

        let russian_owner = cx.entity().downgrade();
        let english_owner = cx.entity().downgrade();
        let locale_dropdown = ComponentButton::new("settings-language")
            .outline()
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .gap_3()
                    .child(locale.name())
                    .child(IconName::ChevronDown),
            )
            .accessibility_label(locale.text("Язык интерфейса", "Interface language"))
            .dropdown_menu(move |menu, _, _| {
                let russian_owner = russian_owner.clone();
                let english_owner = english_owner.clone();
                menu.item(
                    PopupMenuItem::new(Locale::Ru.name())
                        .checked(locale == Locale::Ru)
                        .on_click(move |_, window, cx| {
                            let _ = russian_owner.update(cx, |this, cx| {
                                this.set_locale(Locale::Ru, window, cx);
                            });
                        }),
                )
                .item(
                    PopupMenuItem::new(Locale::En.name())
                        .checked(locale == Locale::En)
                        .on_click(move |_, window, cx| {
                            let _ = english_owner.update(cx, |this, cx| {
                                this.set_locale(Locale::En, window, cx);
                            });
                        }),
                )
            })
            .into_any_element();

        let download_directory = self
            .settings
            .download_directory()
            .to_string_lossy()
            .to_string();
        let download_directory_button = UiButton::new("settings-download-directory")
            .px_3()
            .py_2()
            .rounded(px(7.))
            .bg(rgb(palette.control))
            .text_color(rgb(palette.foreground))
            .hover(|style| style.bg(rgb(palette.hover)))
            .on_click(cx.listener(|this, _, _, cx| {
                this.choose_download_directory(cx);
            }))
            .child(locale.text("Выбрать…", "Choose…"));

        let hide_to_tray = Switch::new("settings-hide-to-tray")
            .checked(self.settings.hide_to_tray())
            .color(rgb(palette.accent))
            .accessibility_label(locale.text("Скрывать в трей", "Hide to tray"))
            .on_change(cx.listener(|this, checked, _, cx| {
                this.settings.set_hide_to_tray(*checked);
                cx.notify();
            }));

        let general_card = div()
            .v_flex()
            .gap_4()
            .p_4()
            .rounded(px(10.))
            .bg(rgb(palette.surface))
            .border_1()
            .border_color(rgb(palette.border))
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .gap_2()
                    .text_color(rgb(palette.muted))
                    .child(IconName::Settings2)
                    .child(
                        div()
                            .text_size(px(15.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(palette.foreground))
                            .child(locale.text("Общие", "General")),
                    ),
            )
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .h_flex()
                            .items_center()
                            .gap_3()
                            .text_color(rgb(palette.muted))
                            .child(IconName::Languages)
                            .child(
                                div()
                                    .v_flex()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_size(px(13.))
                                            .text_color(rgb(palette.foreground))
                                            .child(locale.text("Язык", "Language")),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.))
                                            .text_color(rgb(palette.muted))
                                            .child(locale.text(
                                                "Язык интерфейса лаунчера",
                                                "Launcher interface language",
                                            )),
                                    ),
                            ),
                    )
                    .child(locale_dropdown),
            )
            .child(div().h(px(1.)).w_full().bg(rgb(palette.border)))
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .h_flex()
                            .flex_1()
                            .min_w_0()
                            .items_center()
                            .gap_3()
                            .text_color(rgb(palette.muted))
                            .child(IconName::HardDrive)
                            .child(
                                div()
                                    .v_flex()
                                    .min_w_0()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_size(px(13.))
                                            .text_color(rgb(palette.foreground))
                                            .child(locale.text(
                                                "Папка загрузки сборок",
                                                "Build download folder",
                                            )),
                                    )
                                    .child(
                                        div()
                                            .w_full()
                                            .text_size(px(11.))
                                            .truncate()
                                            .child(download_directory.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(10.))
                                            .text_color(rgb(palette.muted))
                                            .child(locale.text(
                                                "Для новых сборок; существующие не перемещаются. Assets и библиотеки будут храниться рядом.",
                                                "Applies to new profiles; existing ones stay put. Shared assets and libraries are stored alongside.",
                                            )),
                                    ),
                            ),
                    )
                    .child(download_directory_button),
            )
            .child(div().h(px(1.)).w_full().bg(rgb(palette.border)))
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .h_flex()
                            .items_center()
                            .gap_3()
                            .text_color(rgb(palette.muted))
                            .child(IconName::PanelTopClose)
                            .child(
                                div()
                                    .v_flex()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_size(px(13.))
                                            .text_color(rgb(palette.foreground))
                                            .child(locale.text("Скрывать в трей", "Hide to tray")),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.))
                                            .text_color(rgb(palette.muted))
                                            .child(locale.text(
                                                "При закрытии окна оставить лаунчер в фоне",
                                                "Keep the launcher running when the window closes",
                                            )),
                                    ),
                            ),
                    )
                    .child(hide_to_tray),
            );

        let theme_card = div()
            .v_flex()
            .gap_4()
            .p_5()
            .rounded(px(10.))
            .bg(rgb(palette.surface))
            .border_1()
            .border_color(rgb(palette.border))
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .gap_3()
                    .text_color(rgb(palette.muted))
                    .child(IconName::Palette)
                    .child(
                        div()
                            .v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_size(px(15.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(palette.foreground))
                                    .child(locale.text("Внешний вид", "Appearance")),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(rgb(palette.muted))
                                    .child(locale.text(
                                        "Тема и цвет акцента интерфейса",
                                        "Interface theme and accent color",
                                    )),
                            ),
                    ),
            )
            .child(
                div()
                    .h_flex()
                    .gap_2()
                    .child(
                        UiButton::new("settings-theme-dark")
                            .flex_1()
                            .px_4()
                            .py_3()
                            .rounded(px(8.))
                            .bg(rgb(if theme == ThemeChoice::Dark {
                                palette.selected
                            } else {
                                palette.control
                            }))
                            .border_1()
                            .border_color(rgb(if theme == ThemeChoice::Dark {
                                palette.accent
                            } else {
                                palette.border
                            }))
                            .text_color(rgb(palette.foreground))
                            .hover(|style| {
                                style.bg(rgb(if theme == ThemeChoice::Dark {
                                    palette.selected_hover
                                } else {
                                    palette.hover
                                }))
                            })
                            .active(|style| style.bg(rgb(palette.pressed)))
                            .selected(theme == ThemeChoice::Dark)
                            .accessibility_label(locale.text("Тёмная", "Dark"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.settings.set_theme(ThemeChoice::Dark);
                                Theme::change(ThemeMode::Dark, None, cx);
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .h_flex()
                                    .items_center()
                                    .justify_center()
                                    .gap_2()
                                    .child(IconName::Moon)
                                    .child(locale.text("Тёмная", "Dark")),
                            ),
                    )
                    .child(
                        UiButton::new("settings-theme-light")
                            .flex_1()
                            .px_4()
                            .py_3()
                            .rounded(px(8.))
                            .bg(rgb(if theme == ThemeChoice::Light {
                                palette.selected
                            } else {
                                palette.control
                            }))
                            .border_1()
                            .border_color(rgb(if theme == ThemeChoice::Light {
                                palette.accent
                            } else {
                                palette.border
                            }))
                            .text_color(rgb(palette.foreground))
                            .hover(|style| {
                                style.bg(rgb(if theme == ThemeChoice::Light {
                                    palette.selected_hover
                                } else {
                                    palette.hover
                                }))
                            })
                            .active(|style| style.bg(rgb(palette.pressed)))
                            .selected(theme == ThemeChoice::Light)
                            .accessibility_label(locale.text("Светлая", "Light"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.settings.set_theme(ThemeChoice::Light);
                                Theme::change(ThemeMode::Light, None, cx);
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .h_flex()
                                    .items_center()
                                    .justify_center()
                                    .gap_2()
                                    .child(IconName::Sun)
                                    .child(locale.text("Светлая", "Light")),
                            ),
                    ),
            );

        let selected_accent = self.settings.accent();
        let accent_swatches: Vec<_> = AccentColor::ALL
            .into_iter()
            .map(|color| {
                let is_selected = selected_accent == color;
                let color_value = color.color(theme);
                let label = color.name(locale);
                let check = if is_selected {
                    div()
                        .w_full()
                        .h_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(rgb(color.foreground(theme)))
                        .child(IconName::Check)
                        .into_any_element()
                } else {
                    div().into_any_element()
                };

                UiButton::new(color.id())
                    .w(px(30.))
                    .h(px(30.))
                    .rounded(px(18.))
                    .bg(rgb(color_value))
                    .border_2()
                    .border_color(rgb(if is_selected {
                        palette.foreground
                    } else {
                        color_value
                    }))
                    .hover(|style| style.opacity(0.84))
                    .active(|style| style.opacity(0.72))
                    .selected(is_selected)
                    .accessibility_label(label)
                    .tooltip(move |window, cx| Tooltip::new(label).build(window, cx))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.settings.set_accent(color);
                        cx.notify();
                    }))
                    .child(check)
                    .into_any_element()
            })
            .collect();

        let appearance_card = theme_card
            .child(div().h(px(1.)).w_full().bg(rgb(palette.border)))
            .child(
                div()
                    .v_flex()
                    .gap_3()
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(rgb(palette.foreground))
                            .child(locale.text("Цвет акцента", "Accent color")),
                    )
                    .child(
                        div()
                            .h_flex()
                            .items_center()
                            .gap_2()
                            .children(accent_swatches),
                    ),
            );

        let preview = div()
            .v_flex()
            .w(px(232.))
            .gap_3()
            .p_4()
            .rounded(px(10.))
            .bg(rgb(palette.surface))
            .border_1()
            .border_color(rgb(palette.border))
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .gap_2()
                    .text_size(px(12.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(palette.foreground))
                    .child(IconName::PanelLeft)
                    .child(locale.text("Предварительный просмотр", "Preview")),
            )
            .child(
                div()
                    .v_flex()
                    .h(px(270.))
                    .gap_3()
                    .p_3()
                    .rounded(px(8.))
                    .bg(rgb(palette.background))
                    .border_1()
                    .border_color(rgb(palette.border))
                    .child(
                        div()
                            .h_flex()
                            .items_center()
                            .gap_2()
                            .child(brand_mark(palette, px(24.)))
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(palette.foreground))
                                    .child("Minecraft Launcher"),
                            ),
                    )
                    .child(
                        div()
                            .v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .h_flex()
                                    .w_full()
                                    .items_center()
                                    .gap_2()
                                    .px_2()
                                    .py_2()
                                    .rounded(px(6.))
                                    .bg(rgb(palette.selected))
                                    .text_color(rgb(palette.accent))
                                    .child(IconName::House)
                                    .child(locale.text("Главная", "Home")),
                            )
                            .child(
                                div()
                                    .h_flex()
                                    .items_center()
                                    .gap_2()
                                    .px_2()
                                    .py_2()
                                    .text_color(rgb(palette.muted))
                                    .child(IconName::Blocks)
                                    .child(locale.text("Модпаки", "Modpacks")),
                            ),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .h_flex()
                            .items_center()
                            .justify_center()
                            .gap_2()
                            .py_2()
                            .rounded(px(7.))
                            .bg(rgb(palette.accent))
                            .text_color(rgb(palette.accent_foreground))
                            .font_weight(FontWeight::BOLD)
                            .child(IconName::Play)
                            .child(locale.text("Играть", "Play")),
                    ),
            );

        div()
            .h_flex()
            .w_full()
            .gap_4()
            .child(
                div()
                    .v_flex()
                    .flex_1()
                    .gap_4()
                    .child(general_card)
                    .child(appearance_card),
            )
            .child(preview)
            .into_any_element()
    }
}

impl Render for LauncherApp {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let locale = self.settings.locale();
        let palette = Palette::for_settings(self.settings.theme(), self.settings.accent());
        let active_page = self.active_page;

        let navigation_items: Vec<_> = Page::LIBRARY
            .into_iter()
            .map(|page| self.navigation_button(page, palette, locale, cx))
            .collect();
        let settings_button = self.navigation_button(Page::Settings, palette, locale, cx);
        let collapse_button = self.sidebar_toggle_button(palette, locale, cx);
        let sidebar_header = if self.sidebar.is_collapsed() {
            div()
                .h_flex()
                .w_full()
                .items_center()
                .justify_center()
                .gap_1()
                .child(brand_mark(palette, px(20.)))
                .child(collapse_button)
                .into_any_element()
        } else {
            div()
                .h_flex()
                .w_full()
                .items_center()
                .gap_3()
                .child(brand_mark(palette, px(36.)))
                .child(
                    div()
                        .flex_1()
                        .text_size(px(14.))
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(palette.foreground))
                        .child("Minecraft Launcher"),
                )
                .child(collapse_button)
                .into_any_element()
        };
        let navigation = if self.sidebar.is_collapsed() {
            div().v_flex().gap_2()
        } else {
            div().v_flex().gap_2().child(
                div()
                    .px_3()
                    .text_size(px(10.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(palette.muted))
                    .child(locale.text("БИБЛИОТЕКА", "LIBRARY")),
            )
        }
        .children(navigation_items);
        let page_body = match active_page {
            Page::Home => self.home_page(palette, locale, cx),
            Page::Mods => self.mods_page(palette, locale, cx),
            Page::Modpacks | Page::ResourcePacks | Page::Shaders => {
                self.search_empty_page(palette, locale)
            }
            Page::Settings => self.settings_page(palette, cx),
            Page::ModDetails => self.mod_details_page(palette, locale, cx),
        };
        let page_subtitle = match active_page {
            Page::Home => locale.text(
                "Сборки, моды и ресурспаки для твоей игры",
                "Modpacks, mods, and resource packs for your game",
            ),
            Page::Mods => locale.text(
                "Найди дополнения для своей сборки",
                "Find add-ons for your setup",
            ),
            Page::Settings => locale.text(
                "Язык и оформление лаунчера",
                "Launcher language and appearance",
            ),
            Page::ModDetails => "",
            _ => locale.text("Здесь пока пусто", "Nothing here yet"),
        };
        let page_header = if active_page == Page::ModDetails {
            div().into_any_element()
        } else {
            div()
                .v_flex()
                .gap_2()
                .child(
                    div()
                        .text_size(px(26.))
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(palette.foreground))
                        .child(active_page.title(locale)),
                )
                .child(
                    div()
                        .text_size(px(13.))
                        .text_color(rgb(palette.muted))
                        .child(page_subtitle),
                )
                .into_any_element()
        };

        div()
            .flex()
            .size_full()
            .bg(rgb(palette.background))
            .child(
                div()
                    .v_flex()
                    .w(if self.sidebar.is_collapsed() {
                        px(76.)
                    } else {
                        px(252.)
                    })
                    .h_full()
                    .px_3()
                    .py_5()
                    .gap_6()
                    .bg(rgb(palette.sidebar))
                    .border_r_1()
                    .border_color(rgb(palette.border))
                    .child(sidebar_header)
                    .child(navigation)
                    .child(div().flex_1())
                    .child(div().h(px(1.)).w_full().bg(rgb(palette.border)))
                    .child(settings_button),
            )
            .child(
                div()
                    .v_flex()
                    .flex_1()
                    .h_full()
                    .p_8()
                    .gap_6()
                    .child(page_header)
                    .child(page_body),
            )
    }
}

fn mod_thumbnail(sample: &ModSample, size: Pixels) -> AnyElement {
    let icon = sample.icon;
    let background = sample.icon_background;
    let foreground = sample.icon_foreground;

    if let Some(url) = sample.image_url {
        div()
            .w(size)
            .h(size)
            .rounded(px(8.))
            .overflow_hidden()
            .bg(rgb(background))
            .child(
                img(url)
                    .size_full()
                    .object_fit(ObjectFit::Cover)
                    .with_loading(move || mod_icon_placeholder(icon, background, foreground, size))
                    .with_fallback(move || {
                        mod_icon_placeholder(icon, background, foreground, size)
                    }),
            )
            .into_any_element()
    } else {
        mod_icon_placeholder(icon, background, foreground, size)
    }
}

fn version_type_label(version_type: &str, locale: Locale) -> &'static str {
    match version_type {
        "release" => locale.text("Релиз", "Release"),
        "snapshot" => locale.text("Снапшот", "Snapshot"),
        "old_beta" => locale.text("Бета", "Beta"),
        "old_alpha" => locale.text("Альфа", "Alpha"),
        _ => locale.text("Другое", "Other"),
    }
}

fn field_label(label: &'static str) -> AnyElement {
    div()
        .text_size(px(13.))
        .font_weight(FontWeight::BOLD)
        .child(label)
        .into_any_element()
}

fn loader_title(loader: LoaderChoice) -> &'static str {
    match loader {
        LoaderChoice::Vanilla => "Vanilla",
        LoaderChoice::Fabric => "Fabric",
        LoaderChoice::NeoForge => "NeoForge",
        LoaderChoice::Forge => "Forge",
    }
}

fn format_bytes(bytes: u64) -> String {
    const MIB: u64 = 1024 * 1024;
    if bytes >= MIB {
        format!("{:.1} MiB", bytes as f64 / MIB as f64)
    } else if bytes >= 1024 {
        format!("{:.0} KiB", bytes as f64 / 1024.)
    } else {
        format!("{bytes} B")
    }
}

fn mod_icon_placeholder(
    icon: IconName,
    background: u32,
    foreground: u32,
    size: Pixels,
) -> AnyElement {
    div()
        .w(size)
        .h(size)
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(8.))
        .bg(rgb(background))
        .text_color(rgb(foreground))
        .text_size(px((size.as_f32() * 0.5).max(14.)))
        .child(icon)
        .into_any_element()
}

fn detail_meta_row(
    icon: IconName,
    label: &'static str,
    value: &'static str,
    palette: Palette,
) -> AnyElement {
    div()
        .h_flex()
        .items_start()
        .gap_2()
        .text_color(rgb(palette.muted))
        .child(icon)
        .child(
            div()
                .v_flex()
                .flex_1()
                .gap_1()
                .child(div().text_size(px(10.)).child(label))
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(rgb(palette.foreground))
                        .child(value),
                ),
        )
        .into_any_element()
}

fn brand_mark(palette: Palette, size: Pixels) -> AnyElement {
    div()
        .w(size)
        .h(size)
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(8.))
        .bg(rgb(palette.accent))
        .text_color(rgb(palette.accent_foreground))
        .text_size(px(if size.as_f32() < 28. { 14. } else { 20. }))
        .child(IconName::Blocks)
        .into_any_element()
}
