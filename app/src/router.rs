use crate::features::settings::Locale;
use gpui_kit::assets::IconName;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Home,
    Modpacks,
    Mods,
    ResourcePacks,
    Shaders,
    Settings,
    ModDetails,
}

impl Page {
    pub const LIBRARY: [Self; 5] = [
        Self::Home,
        Self::Modpacks,
        Self::Mods,
        Self::ResourcePacks,
        Self::Shaders,
    ];

    pub fn title(self, locale: Locale) -> &'static str {
        match self {
            Self::Home => locale.text("Главная", "Home"),
            Self::Modpacks => locale.text("Модпаки", "Modpacks"),
            Self::Mods => locale.text("Моды", "Mods"),
            Self::ResourcePacks => locale.text("Ресурспаки", "Resource packs"),
            Self::Shaders => locale.text("Шейдеры", "Shaders"),
            Self::Settings => locale.text("Настройки", "Settings"),
            Self::ModDetails => locale.text("Мод", "Mod"),
        }
    }

    pub fn element_id(self) -> &'static str {
        match self {
            Self::Home => "nav-home",
            Self::Modpacks => "nav-modpacks",
            Self::Mods => "nav-mods",
            Self::ResourcePacks => "nav-resource-packs",
            Self::Shaders => "nav-shaders",
            Self::Settings => "nav-settings",
            Self::ModDetails => "nav-mod-details",
        }
    }

    pub fn icon(self) -> IconName {
        match self {
            Self::Home => IconName::House,
            Self::Modpacks => IconName::Blocks,
            Self::Mods => IconName::Puzzle,
            Self::ResourcePacks => IconName::Paintbrush,
            Self::Shaders => IconName::Sun,
            Self::Settings => IconName::Settings,
            Self::ModDetails => IconName::BookOpen,
        }
    }
}
