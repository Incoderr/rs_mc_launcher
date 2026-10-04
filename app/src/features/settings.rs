#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Locale {
    #[default]
    Ru,
    En,
}

impl Locale {
    pub fn tag(self) -> &'static str {
        match self {
            Self::Ru => "ru",
            Self::En => "en",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Ru => "Русский",
            Self::En => "English",
        }
    }

    pub fn text(self, russian: &'static str, english: &'static str) -> &'static str {
        match self {
            Self::Ru => russian,
            Self::En => english,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AccentColor {
    #[default]
    Orange,
    Red,
    Blue,
    Green,
    Purple,
    Pink,
}

impl AccentColor {
    pub const ALL: [Self; 6] = [
        Self::Orange,
        Self::Red,
        Self::Blue,
        Self::Green,
        Self::Purple,
        Self::Pink,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Self::Orange => "accent-orange",
            Self::Red => "accent-red",
            Self::Blue => "accent-blue",
            Self::Green => "accent-green",
            Self::Purple => "accent-purple",
            Self::Pink => "accent-pink",
        }
    }

    pub fn name(self, locale: Locale) -> &'static str {
        match self {
            Self::Orange => locale.text("Оранжевый", "Orange"),
            Self::Red => locale.text("Красный", "Red"),
            Self::Blue => locale.text("Синий", "Blue"),
            Self::Green => locale.text("Зелёный", "Green"),
            Self::Purple => locale.text("Фиолетовый", "Purple"),
            Self::Pink => locale.text("Розовый", "Pink"),
        }
    }

    pub fn color(self, theme: ThemeChoice) -> u32 {
        match (self, theme) {
            (Self::Orange, ThemeChoice::Dark) => 0xff7a1a,
            (Self::Orange, ThemeChoice::Light) => 0xe96a12,
            (Self::Red, ThemeChoice::Dark) => 0xff515b,
            (Self::Red, ThemeChoice::Light) => 0xdc3545,
            (Self::Blue, ThemeChoice::Dark) => 0x448aff,
            (Self::Blue, ThemeChoice::Light) => 0x2563eb,
            (Self::Green, ThemeChoice::Dark) => 0x39c975,
            (Self::Green, ThemeChoice::Light) => 0x168a4a,
            (Self::Purple, ThemeChoice::Dark) => 0xa78bfa,
            (Self::Purple, ThemeChoice::Light) => 0x7c3aed,
            (Self::Pink, ThemeChoice::Dark) => 0xf472b6,
            (Self::Pink, ThemeChoice::Light) => 0xdb2777,
        }
    }

    pub fn foreground(self, theme: ThemeChoice) -> u32 {
        match (self, theme) {
            (Self::Orange, _) | (_, ThemeChoice::Dark) => 0x17120e,
            (_, ThemeChoice::Light) => 0xffffff,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThemeChoice {
    #[default]
    Dark,
    Light,
}

#[derive(Debug, Default)]
pub struct SettingsState {
    locale: Locale,
    theme: ThemeChoice,
    accent: AccentColor,
    hide_to_tray: bool,
    download_directory: PathBuf,
}

impl SettingsState {
    pub fn locale(&self) -> Locale {
        self.locale
    }

    pub fn theme(&self) -> ThemeChoice {
        self.theme
    }

    pub fn accent(&self) -> AccentColor {
        self.accent
    }

    pub fn hide_to_tray(&self) -> bool {
        self.hide_to_tray
    }

    pub fn download_directory(&self) -> &PathBuf {
        &self.download_directory
    }

    pub fn set_locale(&mut self, locale: Locale) {
        self.locale = locale;
    }

    pub fn set_theme(&mut self, theme: ThemeChoice) {
        self.theme = theme;
    }

    pub fn set_accent(&mut self, accent: AccentColor) {
        self.accent = accent;
    }

    pub fn set_hide_to_tray(&mut self, hide_to_tray: bool) {
        self.hide_to_tray = hide_to_tray;
    }

    pub fn set_download_directory(&mut self, path: PathBuf) {
        self.download_directory = path;
    }
}
use std::path::PathBuf;
