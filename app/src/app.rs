use crate::{
    features::mods::{ModCategory, ModDetailTab, ModSample, ModSort, ModsState, SAMPLE_MODS},
    features::settings::{AccentColor, Locale, SettingsState, ThemeChoice},
    features::versions::{VersionCatalogState, fetch_manifest},
    router::Page,
};
use gpui_kit::assets::IconName;
use gpui_kit::base::{Button as UiButton, StyledExt};
use gpui_kit::component::button::{Button as ComponentButton, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::scroll::ScrollableElement;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{Theme, ThemeMode, WindowExt, tooltip::Tooltip};
use gpui_kit::*;

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
    sidebar: SidebarState,
    search_input: Entity<InputState>,
    search_query: String,
    _search_subscription: Subscription,
}

impl LauncherApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Поиск по названию, описанию или автору")
        });
        let search_subscription =
            cx.subscribe(&search_input, |this, input, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    let query = input.read(cx).value().to_string();
                    this.search_query = query.clone();
                    this.mods.set_query(query);
                    cx.notify();
                }
            });

        Self {
            active_page: Page::Home,
            settings: SettingsState::default(),
            mods: ModsState::default(),
            versions: VersionCatalogState::default(),
            sidebar: SidebarState::default(),
            search_input,
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
        cx.notify();
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

    fn load_version_manifest(&mut self, cx: &mut Context<Self>) {
        if !self.versions.begin_loading() {
            return;
        }

        let client = cx.http_client();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = fetch_manifest(client).await;
            let _ = this.update(cx, |this, cx| {
                this.versions.finish_loading(result);
                cx.notify();
            });
        })
        .detach();
    }

    fn open_version_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.load_version_manifest(cx);
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
                    content.child(app.version_picker_body(owner.downgrade(), palette, locale))
                })
        });
    }

    fn version_picker_body(
        &self,
        owner: WeakEntity<Self>,
        palette: Palette,
        locale: Locale,
    ) -> AnyElement {
        let versions = if let Some(manifest) = self.versions.manifest() {
            let release = manifest.latest.release.clone();
            let selected = self.versions.selected_version();
            let entries: Vec<_> = manifest
                .versions
                .iter()
                .map(|version| {
                    let id = version.id.clone();
                    let is_selected = selected == Some(id.as_str());
                    let owner = owner.clone();
                    let selected_id = id.clone();
                    UiButton::new(format!("select-minecraft-version-{id}"))
                        .w_full()
                        .px_3()
                        .py_2()
                        .rounded(px(8.))
                        .bg(rgb(if is_selected {
                            palette.selected_hover
                        } else {
                            palette.surface
                        }))
                        .text_color(rgb(palette.foreground))
                        .hover(|style| style.bg(rgb(palette.hover)))
                        .on_click(move |_, window, cx| {
                            if let Some(owner) = owner.upgrade() {
                                let selected_id = selected_id.clone();
                                let _ = owner.update(cx, |this, cx| {
                                    this.versions.select(selected_id);
                                    cx.notify();
                                });
                                window.close_dialog(cx);
                            }
                        })
                        .child(
                            div()
                                .h_flex()
                                .w_full()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .h_flex()
                                        .items_center()
                                        .gap_2()
                                        .child(IconName::Box)
                                        .child(id.clone()),
                                )
                                .child(
                                    div()
                                        .px_2()
                                        .py_1()
                                        .rounded(px(6.))
                                        .bg(rgb(palette.control))
                                        .text_size(px(10.))
                                        .text_color(rgb(if id == release {
                                            palette.accent
                                        } else {
                                            palette.muted
                                        }))
                                        .child(if id == release {
                                            locale.text("Последний релиз", "Latest release")
                                        } else {
                                            version_type_label(&version.version_type, locale)
                                        }),
                                ),
                        )
                        .into_any_element()
                })
                .collect();

            div()
                .v_flex()
                .w_full()
                .gap_3()
                .child(
                    div()
                        .h_flex()
                        .items_center()
                        .gap_2()
                        .text_size(px(12.))
                        .text_color(rgb(palette.muted))
                        .child(IconName::Info)
                        .child(locale.text(
                            "Выбери версию из официального манифеста Minecraft.",
                            "Choose a version from the official Minecraft manifest.",
                        )),
                )
                .child(
                    div()
                        .v_flex()
                        .w_full()
                        .h(px(390.))
                        .min_h_0()
                        .gap_2()
                        .pr_2()
                        .overflow_y_scrollbar()
                        .children(entries),
                )
                .into_any_element()
        } else if self.versions.is_loading() {
            div()
                .h(px(180.))
                .flex()
                .items_center()
                .justify_center()
                .gap_2()
                .text_color(rgb(palette.muted))
                .child(IconName::LoaderCircle)
                .child(locale.text("Загружаем версии Minecraft…", "Loading Minecraft versions…"))
                .into_any_element()
        } else if let Some(error) = self.versions.error() {
            let retry_owner = owner.clone();
            div()
                .v_flex()
                .items_center()
                .justify_center()
                .gap_3()
                .h(px(180.))
                .child(div().text_color(rgb(palette.muted)).child(locale.text(
                    "Не удалось получить список версий.",
                    "Could not load the version list.",
                )))
                .child(
                    div()
                        .max_w(px(480.))
                        .text_size(px(10.))
                        .text_color(rgb(palette.muted))
                        .child(error.to_string()),
                )
                .child(
                    UiButton::new("retry-minecraft-manifest")
                        .px_3()
                        .py_2()
                        .rounded(px(7.))
                        .bg(rgb(palette.accent))
                        .text_color(rgb(palette.accent_foreground))
                        .on_click(move |_, _, cx| {
                            if let Some(owner) = retry_owner.upgrade() {
                                let _ = owner.update(cx, |this, cx| {
                                    this.load_version_manifest(cx);
                                });
                            }
                        })
                        .child(locale.text("Повторить", "Retry")),
                )
                .into_any_element()
        } else {
            div()
                .h(px(180.))
                .flex()
                .items_center()
                .justify_center()
                .text_color(rgb(palette.muted))
                .child(locale.text("Список версий пуст.", "The version list is empty."))
                .into_any_element()
        };

        div()
            .v_flex()
            .w_full()
            .gap_3()
            .text_color(rgb(palette.foreground))
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .gap_2()
                    .text_size(px(16.))
                    .font_weight(FontWeight::BOLD)
                    .child(IconName::Box)
                    .child(locale.text("Версии игры", "Game versions")),
            )
            .child(versions)
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
