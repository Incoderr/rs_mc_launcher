use crate::features::settings::Locale;
use gpui_kit::assets::IconName;

pub mod catalog;

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
    pub name: &'static str,
    pub author: &'static str,
    pub icon: IconName,
    pub icon_background: u32,
    pub icon_foreground: u32,
    pub image_url: Option<&'static str>,
}

pub const SAMPLE_MODS: &[ModSample] = &[
    ModSample {
        name: "JEI (Just Enough Items)",
        author: "mezz",
        icon: IconName::BookOpen,
        icon_background: 0x32834a,
        icon_foreground: 0xffffff,
        image_url: Some(
            "https://cdn.modrinth.com/data/u6dRKJwZ/4a3f18ac0d096c9f8e9176984c44be4e58f94c89_96.webp",
        ),
    },
    ModSample {
        name: "JourneyMap",
        author: "techbrew",
        icon: IconName::Map,
        icon_background: 0x173d36,
        icon_foreground: 0xb7f4d4,
        image_url: None,
    },
    ModSample {
        name: "Create",
        author: "simibubi",
        icon: IconName::Blocks,
        icon_background: 0xb07a37,
        icon_foreground: 0x24180f,
        image_url: None,
    },
    ModSample {
        name: "Sodium",
        author: "JellySquid",
        icon: IconName::Cpu,
        icon_background: 0x39a849,
        icon_foreground: 0xffffff,
        image_url: None,
    },
    ModSample {
        name: "Farmer's Delight",
        author: "vectorwing",
        icon: IconName::BookOpen,
        icon_background: 0x9e6841,
        icon_foreground: 0xfff1d6,
        image_url: None,
    },
    ModSample {
        name: "Lithium",
        author: "CaffeineMC",
        icon: IconName::Cpu,
        icon_background: 0x447b67,
        icon_foreground: 0xe2fff2,
        image_url: None,
    },
    ModSample {
        name: "Xaero's Minimap",
        author: "xaero96",
        icon: IconName::Map,
        icon_background: 0x65549a,
        icon_foreground: 0xf1edff,
        image_url: None,
    },
    ModSample {
        name: "Botania",
        author: "Vazkii",
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
    source: catalog::ModSource,
    query: String,
    projects: Vec<catalog::ModProject>,
    errors: Vec<catalog::CatalogProviderError>,
    loading: bool,
    request_id: u64,
    selected_mod: Option<catalog::ModProject>,
    detail_tab: ModDetailTab,
}

impl ModsState {
    pub fn category(&self) -> ModCategory {
        self.category
    }

    pub fn sort(&self) -> ModSort {
        self.sort
    }

    pub fn source(&self) -> catalog::ModSource {
        self.source
    }

    pub fn set_source(&mut self, source: catalog::ModSource) {
        if self.source != source {
            self.source = source;
            self.invalidate_search();
        }
    }

    pub fn set_category(&mut self, category: ModCategory) {
        self.category = category;
    }

    pub fn set_sort(&mut self, sort: ModSort) {
        self.sort = sort;
    }

    pub fn set_query(&mut self, query: String) {
        if self.query != query {
            self.query = query;
            self.invalidate_search();
        }
    }

    pub fn select_mod(&mut self, project: catalog::ModProject) {
        self.selected_mod = Some(project);
        self.detail_tab = ModDetailTab::Description;
    }

    pub fn selected_mod(&self) -> Option<&catalog::ModProject> {
        self.selected_mod.as_ref()
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

    pub fn is_loading(&self) -> bool {
        self.loading
    }

    pub fn errors(&self) -> &[catalog::CatalogProviderError] {
        &self.errors
    }

    pub fn begin_search(&mut self) -> (u64, String, ModSort, catalog::ModSource) {
        self.request_id = self.request_id.wrapping_add(1);
        self.loading = true;
        self.errors.clear();
        (self.request_id, self.query.clone(), self.sort, self.source)
    }

    pub fn is_current_request(&self, request_id: u64) -> bool {
        self.request_id == request_id
    }

    pub fn finish_search(&mut self, request_id: u64, result: catalog::ModCatalogResult) -> bool {
        if !self.is_current_request(request_id) {
            return false;
        }
        self.loading = false;
        self.projects = result.projects;
        self.errors = result.errors;
        true
    }

    pub fn visible_mods(&self) -> Vec<&catalog::ModProject> {
        let query = self.query.trim().to_lowercase();
        let mut mods: Vec<_> = self
            .projects
            .iter()
            .filter(|project| {
                (self.source == catalog::ModSource::All || project.source == self.source)
                    && (self.category == ModCategory::All
                        || project.categories.contains(&self.category))
                    && (query.is_empty()
                        || project.name.to_lowercase().contains(&query)
                        || project.author.to_lowercase().contains(&query)
                        || project.description.to_lowercase().contains(&query))
            })
            .collect();

        match self.sort {
            // Provider popularity metrics differ (follows on Modrinth and
            // popularity rank on CurseForge), so retain each provider's rank.
            ModSort::Popular => {}
            ModSort::Downloads => mods.sort_by_key(|project| std::cmp::Reverse(project.downloads)),
            ModSort::Name => {
                mods.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
            }
        }

        mods
    }

    fn invalidate_search(&mut self) {
        self.request_id = self.request_id.wrapping_add(1);
        self.loading = false;
        self.errors.clear();
    }
}
