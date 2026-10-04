use crate::features::settings::Locale;
use gpui_kit::assets::IconName;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ModCategory {
    #[default]
    All,
    Adventure,
    Technology,
    Decoration,
    Optimization,
    Libraries,
    Magic,
    Rpg,
}

impl ModCategory {
    pub const ALL: [Self; 8] = [
        Self::All,
        Self::Adventure,
        Self::Technology,
        Self::Decoration,
        Self::Optimization,
        Self::Libraries,
        Self::Magic,
        Self::Rpg,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Self::All => "mods-filter-all",
            Self::Adventure => "mods-filter-adventure",
            Self::Technology => "mods-filter-technology",
            Self::Decoration => "mods-filter-decoration",
            Self::Optimization => "mods-filter-optimization",
            Self::Libraries => "mods-filter-libraries",
            Self::Magic => "mods-filter-magic",
            Self::Rpg => "mods-filter-rpg",
        }
    }

    pub fn title(self, locale: Locale) -> &'static str {
        match self {
            Self::All => locale.text("Все", "All"),
            Self::Adventure => locale.text("Приключения", "Adventure"),
            Self::Technology => locale.text("Технологии", "Technology"),
            Self::Decoration => locale.text("Декорации", "Decoration"),
            Self::Optimization => locale.text("Оптимизация", "Optimization"),
            Self::Libraries => locale.text("Библиотеки", "Libraries"),
            Self::Magic => locale.text("Магия", "Magic"),
            Self::Rpg => "RPG",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ModSort {
    #[default]
    Popular,
    Downloads,
    Name,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModId {
    Jei,
    JourneyMap,
    Create,
    Sodium,
    FarmersDelight,
    Lithium,
    XaerosMinimap,
    Botania,
}

impl ModId {
    pub fn element_id(self) -> &'static str {
        match self {
            Self::Jei => "jei",
            Self::JourneyMap => "journeymap",
            Self::Create => "create",
            Self::Sodium => "sodium",
            Self::FarmersDelight => "farmers-delight",
            Self::Lithium => "lithium",
            Self::XaerosMinimap => "xaeros-minimap",
            Self::Botania => "botania",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ModDetailTab {
    #[default]
    Description,
    Files,
    Dependencies,
    Screenshots,
    Similar,
}

impl ModDetailTab {
    pub const ALL: [Self; 5] = [
        Self::Description,
        Self::Files,
        Self::Dependencies,
        Self::Screenshots,
        Self::Similar,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Self::Description => "mod-detail-description",
            Self::Files => "mod-detail-files",
            Self::Dependencies => "mod-detail-dependencies",
            Self::Screenshots => "mod-detail-screenshots",
            Self::Similar => "mod-detail-similar",
        }
    }

    pub fn title(self, locale: Locale) -> &'static str {
        match self {
            Self::Description => locale.text("Описание", "Description"),
            Self::Files => locale.text("Файлы", "Files"),
            Self::Dependencies => locale.text("Зависимости", "Dependencies"),
            Self::Screenshots => locale.text("Скриншоты", "Screenshots"),
            Self::Similar => locale.text("Похожие", "Similar"),
        }
    }
}

impl ModSort {
    pub fn title(self, locale: Locale) -> &'static str {
        match self {
            Self::Popular => locale.text("Популярные", "Popular"),
            Self::Downloads => locale.text("По загрузкам", "Most downloaded"),
            Self::Name => locale.text("По названию", "Name"),
        }
    }
}

#[derive(Clone, Copy)]
pub struct ModSample {
    pub id: ModId,
    pub name: &'static str,
    pub description_ru: &'static str,
    pub description_en: &'static str,
    pub author: &'static str,
    pub game_version: &'static str,
    pub downloads_label: &'static str,
    pub downloads: u64,
    pub likes_label: &'static str,
    pub likes: u64,
    pub updated: &'static str,
    pub file_size: &'static str,
    pub license: &'static str,
    pub loaders: &'static str,
    pub files_count: u32,
    pub categories: &'static [ModCategory],
    pub icon: IconName,
    pub icon_background: u32,
    pub icon_foreground: u32,
    pub image_url: Option<&'static str>,
}

impl ModSample {
    pub fn description(self, locale: Locale) -> &'static str {
        locale.text(self.description_ru, self.description_en)
    }

    pub fn matches(self, category: ModCategory) -> bool {
        category == ModCategory::All || self.categories.contains(&category)
    }
}

pub const SAMPLE_MODS: &[ModSample] = &[
    ModSample {
        id: ModId::Jei,
        name: "JEI (Just Enough Items)",
        description_ru: "Просмотр рецептов и использование предметов.",
        description_en: "View recipes and item usage.",
        author: "mezz",
        game_version: "1.20.1",
        downloads_label: "349.3M",
        downloads: 349_300_000,
        likes_label: "403.2K",
        likes: 403_200,
        updated: "2025-11-18",
        file_size: "4.2 MB",
        license: "MIT",
        loaders: "Forge, NeoForge, Fabric",
        files_count: 83,
        categories: &[ModCategory::Libraries],
        icon: IconName::BookOpen,
        icon_background: 0x32834a,
        icon_foreground: 0xffffff,
        image_url: Some(
            "https://cdn.modrinth.com/data/u6dRKJwZ/4a3f18ac0d096c9f8e9176984c44be4e58f94c89_96.webp",
        ),
    },
    ModSample {
        id: ModId::JourneyMap,
        name: "JourneyMap",
        description_ru: "Миникарта и карта мира в реальном времени.",
        description_en: "A live minimap and world map.",
        author: "techbrew",
        game_version: "1.20.1",
        downloads_label: "212.8M",
        downloads: 212_800_000,
        likes_label: "176.1K",
        likes: 176_100,
        updated: "2025-09-22",
        file_size: "9.1 MB",
        license: "All Rights Reserved",
        loaders: "Forge, Fabric, NeoForge",
        files_count: 54,
        categories: &[ModCategory::Adventure, ModCategory::Rpg],
        icon: IconName::Map,
        icon_background: 0x173d36,
        icon_foreground: 0xb7f4d4,
        image_url: None,
    },
    ModSample {
        id: ModId::Create,
        name: "Create",
        description_ru: "Механизмы, транспорт и автоматизация.",
        description_en: "Machines, transportation, and automation.",
        author: "simibubi",
        game_version: "1.20.1",
        downloads_label: "161.6M",
        downloads: 161_600_000,
        likes_label: "211.5K",
        likes: 211_500,
        updated: "2025-12-04",
        file_size: "16.8 MB",
        license: "MIT",
        loaders: "Forge, Fabric",
        files_count: 126,
        categories: &[ModCategory::Technology, ModCategory::Decoration],
        icon: IconName::Blocks,
        icon_background: 0xb07a37,
        icon_foreground: 0x24180f,
        image_url: None,
    },
    ModSample {
        id: ModId::Sodium,
        name: "Sodium",
        description_ru: "Оптимизация графики и повышение FPS.",
        description_en: "Rendering optimization and higher FPS.",
        author: "JellySquid",
        game_version: "1.20.1",
        downloads_label: "145.2M",
        downloads: 145_200_000,
        likes_label: "185.7K",
        likes: 185_700,
        updated: "2025-10-11",
        file_size: "1.2 MB",
        license: "LGPL-3.0",
        loaders: "Fabric, NeoForge",
        files_count: 39,
        categories: &[ModCategory::Optimization],
        icon: IconName::Cpu,
        icon_background: 0x39a849,
        icon_foreground: 0xffffff,
        image_url: None,
    },
    ModSample {
        id: ModId::FarmersDelight,
        name: "Farmer's Delight",
        description_ru: "Новые блюда, инструменты и фермерские блоки.",
        description_en: "New meals, tools, and farming blocks.",
        author: "vectorwing",
        game_version: "1.20.1",
        downloads_label: "49.8M",
        downloads: 49_800_000,
        likes_label: "58.4K",
        likes: 58_400,
        updated: "2025-08-15",
        file_size: "3.4 MB",
        license: "MIT",
        loaders: "Forge, Fabric",
        files_count: 46,
        categories: &[ModCategory::Decoration, ModCategory::Adventure],
        icon: IconName::BookOpen,
        icon_background: 0x9e6841,
        icon_foreground: 0xfff1d6,
        image_url: None,
    },
    ModSample {
        id: ModId::Lithium,
        name: "Lithium",
        description_ru: "Ускоряет игровую логику без изменения механик.",
        description_en: "Speeds up game logic without changing mechanics.",
        author: "CaffeineMC",
        game_version: "1.20.1",
        downloads_label: "108.0M",
        downloads: 108_000_000,
        likes_label: "81.0K",
        likes: 81_000,
        updated: "2025-11-02",
        file_size: "0.7 MB",
        license: "LGPL-3.0",
        loaders: "Fabric, NeoForge",
        files_count: 28,
        categories: &[ModCategory::Optimization],
        icon: IconName::Cpu,
        icon_background: 0x447b67,
        icon_foreground: 0xe2fff2,
        image_url: None,
    },
    ModSample {
        id: ModId::XaerosMinimap,
        name: "Xaero's Minimap",
        description_ru: "Миникарта с метками и точками интереса.",
        description_en: "A minimap with waypoints and points of interest.",
        author: "xaero96",
        game_version: "1.20.1",
        downloads_label: "73.2M",
        downloads: 73_200_000,
        likes_label: "64.8K",
        likes: 64_800,
        updated: "2025-10-26",
        file_size: "2.1 MB",
        license: "All Rights Reserved",
        loaders: "Forge, Fabric, NeoForge",
        files_count: 61,
        categories: &[ModCategory::Adventure, ModCategory::Rpg],
        icon: IconName::Map,
        icon_background: 0x65549a,
        icon_foreground: 0xf1edff,
        image_url: None,
    },
    ModSample {
        id: ModId::Botania,
        name: "Botania",
        description_ru: "Магическая ботаника и автоматизация.",
        description_en: "Magical botany and automation.",
        author: "Vazkii",
        game_version: "1.20.1",
        downloads_label: "68.1M",
        downloads: 68_100_000,
        likes_label: "37.6K",
        likes: 37_600,
        updated: "2025-06-12",
        file_size: "8.3 MB",
        license: "MIT",
        loaders: "Forge, Fabric",
        files_count: 44,
        categories: &[ModCategory::Magic, ModCategory::Decoration],
        icon: IconName::Asterisk,
        icon_background: 0x406d3e,
        icon_foreground: 0xe5ffd3,
        image_url: None,
    },
];

#[derive(Debug, Default)]
pub struct ModsState {
    category: ModCategory,
    sort: ModSort,
    query: String,
    selected_mod: Option<ModId>,
    detail_tab: ModDetailTab,
}

impl ModsState {
    pub fn category(&self) -> ModCategory {
        self.category
    }

    pub fn sort(&self) -> ModSort {
        self.sort
    }

    pub fn set_category(&mut self, category: ModCategory) {
        self.category = category;
    }

    pub fn set_sort(&mut self, sort: ModSort) {
        self.sort = sort;
    }

    pub fn set_query(&mut self, query: String) {
        self.query = query;
    }

    pub fn select_mod(&mut self, id: ModId) {
        self.selected_mod = Some(id);
        self.detail_tab = ModDetailTab::Description;
    }

    pub fn selected_mod(&self) -> Option<&'static ModSample> {
        self.selected_mod
            .and_then(|id| SAMPLE_MODS.iter().find(|sample| sample.id == id))
    }

    pub fn clear_selected_mod(&mut self) {
        self.selected_mod = None;
    }

    pub fn detail_tab(&self) -> ModDetailTab {
        self.detail_tab
    }

    pub fn set_detail_tab(&mut self, tab: ModDetailTab) {
        self.detail_tab = tab;
    }

    pub fn visible_mods(&self) -> Vec<&'static ModSample> {
        let query = self.query.trim().to_lowercase();
        let mut mods: Vec<_> = SAMPLE_MODS
            .iter()
            .filter(|sample| {
                sample.matches(self.category)
                    && (query.is_empty()
                        || sample.name.to_lowercase().contains(&query)
                        || sample.author.to_lowercase().contains(&query)
                        || sample.description_ru.to_lowercase().contains(&query)
                        || sample.description_en.to_lowercase().contains(&query))
            })
            .collect();

        match self.sort {
            ModSort::Popular => mods.sort_by_key(|sample| std::cmp::Reverse(sample.likes)),
            ModSort::Downloads => mods.sort_by_key(|sample| std::cmp::Reverse(sample.downloads)),
            ModSort::Name => mods.sort_by_key(|sample| sample.name),
        }

        mods
    }
}
