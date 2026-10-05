use std::{sync::Arc, time::Duration};

use gpui_kit::http_client::{AsyncBody, HttpClient, HttpRequestExt, Request};
use serde::Deserialize;

use super::{ModCategory, ModSort};
use crate::features::settings::Locale;
use gpui_kit::assets::IconName;

const MODRINTH_SEARCH_URL: &str = "https://api.modrinth.com/v2/search";
const CURSEFORGE_SEARCH_URL: &str = "https://api.curseforge.com/v1/mods/search";
const CURSEFORGE_MINECRAFT_GAME_ID: u32 = 432;
const CURSEFORGE_MODS_CLASS_ID: u32 = 6;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ModSource {
    #[default]
    All,
    Modrinth,
    CurseForge,
}

impl ModSource {
    pub const FILTERS: [Self; 3] = [Self::All, Self::Modrinth, Self::CurseForge];

    pub fn title(self, locale: Locale) -> &'static str {
        match self {
            Self::All => locale.text("Все источники", "All sources"),
            Self::Modrinth => "Modrinth",
            Self::CurseForge => "CurseForge",
        }
    }

    fn api_name(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Modrinth => "Modrinth",
            Self::CurseForge => "CurseForge",
        }
    }
}

#[derive(Clone, Debug)]
pub struct ModProject {
    pub id: String,
    pub source: ModSource,
    pub name: String,
    pub description: String,
    pub author: String,
    pub game_version: String,
    pub downloads_label: String,
    pub downloads: u64,
    pub likes_label: String,
    pub updated: String,
    pub file_size: String,
    pub license: String,
    pub loaders: String,
    pub files_count: u32,
    pub categories: Vec<ModCategory>,
    pub icon: IconName,
    pub icon_background: u32,
    pub icon_foreground: u32,
    pub image_url: Option<String>,
    pub page_url: String,
}

#[derive(Clone, Debug)]
pub struct CatalogProviderError {
    pub provider: ModSource,
    pub message: String,
    pub missing_api_key: bool,
}

#[derive(Debug, Default)]
pub struct ModCatalogResult {
    pub projects: Vec<ModProject>,
    pub errors: Vec<CatalogProviderError>,
}

pub async fn search_catalog(
    client: Arc<dyn HttpClient>,
    query: String,
    sort: ModSort,
    source: ModSource,
    curseforge_api_key: Option<String>,
) -> ModCatalogResult {
    let (modrinth, curseforge) = match source {
        ModSource::All => {
            futures::join!(
                search_modrinth(client.clone(), &query, sort),
                search_curseforge(client, &query, sort, curseforge_api_key),
            )
        }
        ModSource::Modrinth => (search_modrinth(client, &query, sort).await, Ok(Vec::new())),
        ModSource::CurseForge => (
            Ok(Vec::new()),
            search_curseforge(client, &query, sort, curseforge_api_key).await,
        ),
    };

    let mut result = ModCatalogResult::default();
    let modrinth_projects = collect_provider_results(&mut result, ModSource::Modrinth, modrinth);
    let curseforge_projects =
        collect_provider_results(&mut result, ModSource::CurseForge, curseforge);

    if source == ModSource::All && sort == ModSort::Popular {
        let count = modrinth_projects.len().max(curseforge_projects.len());
        for index in 0..count {
            if let Some(project) = modrinth_projects.get(index) {
                result.projects.push(project.clone());
            }
            if let Some(project) = curseforge_projects.get(index) {
                result.projects.push(project.clone());
            }
        }
    } else {
        result.projects.extend(modrinth_projects);
        result.projects.extend(curseforge_projects);
    }
    result
}

fn collect_provider_results(
    result: &mut ModCatalogResult,
    provider: ModSource,
    response: Result<Vec<ModProject>, CatalogProviderError>,
) -> Vec<ModProject> {
    match response {
        Ok(projects) => projects,
        Err(error) => {
            tracing::warn!(provider = provider.api_name(), message = %error.message, "mod catalog search failed");
            result.errors.push(error);
            Vec::new()
        }
    }
}

async fn search_modrinth(
    client: Arc<dyn HttpClient>,
    query: &str,
    sort: ModSort,
) -> Result<Vec<ModProject>, CatalogProviderError> {
    let index = match sort {
        ModSort::Popular => "follows",
        ModSort::Downloads => "downloads",
        ModSort::Name => "relevance",
    };
    let mut url = format!(
        "{MODRINTH_SEARCH_URL}?facets=%5B%5B%22project_type%3Amod%22%5D%5D&index={index}&limit=100"
    );
    if !query.trim().is_empty() {
        url.push_str("&query=");
        url.push_str(&encode_query_component(query.trim()));
    }

    let body = request_body(client, &url, None, ModSource::Modrinth).await?;
    let response: ModrinthSearchResponse = serde_json::from_str(&body).map_err(|error| {
        provider_error(
            ModSource::Modrinth,
            format!("Invalid search response: {error}"),
        )
    })?;

    Ok(response
        .hits
        .into_iter()
        .map(ModProject::from_modrinth)
        .collect())
}

async fn search_curseforge(
    client: Arc<dyn HttpClient>,
    query: &str,
    sort: ModSort,
    api_key: Option<String>,
) -> Result<Vec<ModProject>, CatalogProviderError> {
    let Some(api_key) = api_key.filter(|key| !key.trim().is_empty()) else {
        return Err(CatalogProviderError {
            provider: ModSource::CurseForge,
            message: "Set the CURSEFORGE_API_KEY environment variable to enable CurseForge.".into(),
            missing_api_key: true,
        });
    };

    let sort_field = match sort {
        ModSort::Popular => 2,
        ModSort::Downloads => 6,
        ModSort::Name => 4,
    };
    let mut url = format!(
        "{CURSEFORGE_SEARCH_URL}?gameId={CURSEFORGE_MINECRAFT_GAME_ID}&classId={CURSEFORGE_MODS_CLASS_ID}&pageSize=50&index=0&sortField={sort_field}&sortOrder=desc"
    );
    if !query.trim().is_empty() {
        url.push_str("&searchFilter=");
        url.push_str(&encode_query_component(query.trim()));
    }

    let body = request_body(client, &url, Some(&api_key), ModSource::CurseForge).await?;
    let response: CurseForgeSearchResponse = serde_json::from_str(&body).map_err(|error| {
        provider_error(
            ModSource::CurseForge,
            format!("Invalid search response: {error}"),
        )
    })?;

    Ok(response
        .data
        .into_iter()
        .map(ModProject::from_curseforge)
        .collect())
}

async fn request_body(
    client: Arc<dyn HttpClient>,
    url: &str,
    api_key: Option<&str>,
    provider: ModSource,
) -> Result<String, CatalogProviderError> {
    let mut request = Request::builder().uri(url).timeout(Duration::from_secs(20));
    if let Some(api_key) = api_key {
        request = request.header("x-api-key", api_key);
    }
    let request = request
        .body(AsyncBody::empty())
        .map_err(|error| provider_error(provider, format!("Could not prepare request: {error}")))?;
    let response = client
        .send(request)
        .await
        .map_err(|error| provider_error(provider, format!("Request failed: {error}")))?;
    let status = response.status();
    if !status.is_success() {
        return Err(provider_error(
            provider,
            format!("Server returned HTTP {status}"),
        ));
    }

    let mut body = response.into_body();
    let mut bytes = Vec::new();
    futures::io::AsyncReadExt::read_to_end(&mut body, &mut bytes)
        .await
        .map_err(|error| provider_error(provider, format!("Could not read response: {error}")))?;
    String::from_utf8(bytes)
        .map_err(|error| provider_error(provider, format!("Response is not valid UTF-8: {error}")))
}

fn provider_error(provider: ModSource, message: String) -> CatalogProviderError {
    CatalogProviderError {
        provider,
        message,
        missing_api_key: false,
    }
}

fn encode_query_component(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(char::from(byte));
        } else {
            use std::fmt::Write as _;
            let _ = write!(encoded, "%{byte:02X}");
        }
    }
    encoded
}

#[derive(Deserialize)]
struct ModrinthSearchResponse {
    #[serde(default)]
    hits: Vec<ModrinthHit>,
}

#[derive(Deserialize)]
struct ModrinthHit {
    project_id: String,
    title: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    author: String,
    #[serde(default)]
    categories: Vec<String>,
    #[serde(default)]
    versions: Vec<String>,
    #[serde(default)]
    downloads: u64,
    #[serde(default)]
    follows: u64,
    date_modified: Option<String>,
    license: Option<String>,
    icon_url: Option<String>,
    slug: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurseForgeSearchResponse {
    #[serde(default)]
    data: Vec<CurseForgeMod>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurseForgeMod {
    id: u64,
    name: String,
    slug: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    download_count: u64,
    thumbs_up_count: u64,
    date_modified: Option<String>,
    logo: Option<CurseForgeAsset>,
    #[serde(default)]
    authors: Vec<CurseForgeAuthor>,
    #[serde(default)]
    categories: Vec<CurseForgeCategory>,
    #[serde(default)]
    latest_files_indexes: Vec<CurseForgeFileIndex>,
}

#[derive(Deserialize)]
struct CurseForgeAsset {
    url: Option<String>,
}

#[derive(Deserialize)]
struct CurseForgeAuthor {
    name: String,
}

#[derive(Deserialize)]
struct CurseForgeCategory {
    name: String,
    slug: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurseForgeFileIndex {
    game_version: String,
    mod_loader: Option<u32>,
}

impl ModProject {
    fn from_modrinth(hit: ModrinthHit) -> Self {
        let categories = map_categories(hit.categories.iter().map(String::as_str));
        let loaders = hit
            .categories
            .iter()
            .filter(|tag| is_loader(tag))
            .map(|tag| display_loader(tag))
            .collect::<Vec<_>>();
        let slug = hit.slug.as_deref().unwrap_or(&hit.project_id);
        let page_url = format!("https://modrinth.com/mod/{slug}");
        Self {
            id: format!("modrinth:{}", hit.project_id),
            source: ModSource::Modrinth,
            name: hit.title,
            description: hit.description,
            author: hit.author,
            game_version: hit
                .versions
                .first()
                .cloned()
                .unwrap_or_else(|| "Any version".into()),
            downloads_label: format_count(hit.downloads),
            downloads: hit.downloads,
            likes_label: format_count(hit.follows),
            updated: date_label(hit.date_modified.as_deref()),
            file_size: "—".into(),
            license: hit.license.unwrap_or_else(|| "—".into()),
            loaders: loader_label(loaders),
            files_count: 0,
            icon: icon_for_categories(&categories),
            icon_background: 0x2f6f4e,
            icon_foreground: 0xffffff,
            image_url: hit.icon_url,
            categories,
            page_url,
        }
    }

    fn from_curseforge(project: CurseForgeMod) -> Self {
        let raw_categories = project
            .categories
            .iter()
            .map(|category| format!("{} {}", category.slug, category.name))
            .collect::<Vec<_>>();
        let categories = map_categories(raw_categories.iter().map(String::as_str));
        let loaders = project
            .latest_files_indexes
            .iter()
            .filter_map(|index| index.mod_loader.map(mod_loader_name))
            .collect::<Vec<_>>();
        let game_version = project
            .latest_files_indexes
            .first()
            .map(|index| index.game_version.clone())
            .unwrap_or_else(|| "Any version".into());
        let author = project
            .authors
            .first()
            .map(|author| author.name.clone())
            .unwrap_or_else(|| "Unknown".into());
        let page_url = format!(
            "https://www.curseforge.com/minecraft/mc-mods/{}",
            project.slug
        );
        Self {
            id: format!("curseforge:{}", project.id),
            source: ModSource::CurseForge,
            name: project.name,
            description: project.summary,
            author,
            game_version,
            downloads_label: format_count(project.download_count),
            downloads: project.download_count,
            likes_label: format_count(u64::from(project.thumbs_up_count)),
            updated: date_label(project.date_modified.as_deref()),
            file_size: "—".into(),
            license: "—".into(),
            loaders: loader_label(loaders),
            files_count: 0,
            icon: icon_for_categories(&categories),
            icon_background: 0x775d32,
            icon_foreground: 0xffffff,
            image_url: project.logo.and_then(|asset| asset.url),
            categories,
            page_url,
        }
    }
}

fn map_categories<'a>(tags: impl Iterator<Item = &'a str>) -> Vec<ModCategory> {
    let mut categories = Vec::new();
    for tag in tags.map(str::to_ascii_lowercase) {
        let category = if tag.contains("adventure") {
            Some(ModCategory::Adventure)
        } else if tag.contains("technology") || tag.contains("tech") {
            Some(ModCategory::Technology)
        } else if tag.contains("decoration") || tag.contains("decor") {
            Some(ModCategory::Decoration)
        } else if tag.contains("optimization") || tag.contains("performance") {
            Some(ModCategory::Optimization)
        } else if tag.contains("librar") || tag == "library" {
            Some(ModCategory::Libraries)
        } else if tag.contains("magic") {
            Some(ModCategory::Magic)
        } else if tag.contains("rpg") || tag.contains("roleplay") {
            Some(ModCategory::Rpg)
        } else {
            None
        };
        if let Some(category) = category.filter(|category| !categories.contains(category)) {
            categories.push(category);
        }
    }
    categories
}

fn is_loader(tag: &str) -> bool {
    matches!(
        tag.to_ascii_lowercase().as_str(),
        "fabric" | "forge" | "neoforge" | "quilt"
    )
}

fn display_loader(loader: &str) -> String {
    match loader.to_ascii_lowercase().as_str() {
        "fabric" => "Fabric".into(),
        "forge" => "Forge".into(),
        "neoforge" => "NeoForge".into(),
        "quilt" => "Quilt".into(),
        _ => loader.to_owned(),
    }
}

fn mod_loader_name(loader: u32) -> String {
    display_loader(match loader {
        1 => "forge",
        4 => "fabric",
        5 => "quilt",
        6 => "neoforge",
        _ => "unknown",
    })
}

fn loader_label(loaders: Vec<String>) -> String {
    let mut unique = Vec::new();
    for loader in loaders {
        if !loader.eq_ignore_ascii_case("unknown") && !unique.contains(&loader) {
            unique.push(loader);
        }
    }
    if unique.is_empty() {
        "—".into()
    } else {
        unique.join(", ")
    }
}

fn icon_for_categories(categories: &[ModCategory]) -> IconName {
    if categories.contains(&ModCategory::Adventure) || categories.contains(&ModCategory::Rpg) {
        IconName::Map
    } else if categories.contains(&ModCategory::Optimization) {
        IconName::Cpu
    } else if categories.contains(&ModCategory::Technology) {
        IconName::Blocks
    } else if categories.contains(&ModCategory::Magic) {
        IconName::Asterisk
    } else {
        IconName::Puzzle
    }
}

fn format_count(count: u64) -> String {
    if count >= 1_000_000_000 {
        format!("{:.1}B", count as f64 / 1_000_000_000.0)
    } else if count >= 1_000_000 {
        format!("{:.1}M", count as f64 / 1_000_000.0)
    } else if count >= 1_000 {
        format!("{:.1}K", count as f64 / 1_000.0)
    } else {
        count.to_string()
    }
}

fn date_label(date: Option<&str>) -> String {
    date.and_then(|date| date.get(..10))
        .unwrap_or("—")
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::{CurseForgeMod, ModProject, ModSource, ModrinthHit, encode_query_component};
    use crate::features::mods::ModCategory;

    #[test]
    fn maps_modrinth_search_hit_into_catalog_project() {
        let hit: ModrinthHit = serde_json::from_str(
            r#"{
                "project_id":"abc123",
                "slug":"example-mod",
                "title":"Example Mod",
                "description":"A useful mod",
                "author":"Example Author",
                "categories":["technology","fabric"],
                "versions":["1.21.1"],
                "downloads":1200,
                "follows":25,
                "date_modified":"2026-01-02T03:04:05Z",
                "license":"MIT",
                "icon_url":"https://cdn.example.org/icon.png"
            }"#,
        )
        .expect("valid Modrinth search hit");

        let project = ModProject::from_modrinth(hit);

        assert_eq!(project.source, ModSource::Modrinth);
        assert_eq!(project.name, "Example Mod");
        assert_eq!(project.game_version, "1.21.1");
        assert_eq!(project.downloads_label, "1.2K");
        assert_eq!(project.updated, "2026-01-02");
        assert_eq!(project.loaders, "Fabric");
        assert!(project.categories.contains(&ModCategory::Technology));
        assert_eq!(project.page_url, "https://modrinth.com/mod/example-mod");
    }

    #[test]
    fn maps_curseforge_camel_case_search_hit_into_catalog_project() {
        let hit: CurseForgeMod = serde_json::from_str(
            r#"{
                "id":42,
                "name":"Example Mod",
                "slug":"example-mod",
                "summary":"A useful mod",
                "downloadCount":9876,
                "thumbsUpCount":31,
                "dateModified":"2026-02-03T04:05:06Z",
                "logo":{"url":"https://media.example.org/icon.png"},
                "authors":[{"name":"Example Author"}],
                "categories":[{"name":"Technology","slug":"technology"}],
                "latestFilesIndexes":[{"gameVersion":"1.20.1","modLoader":4}]
            }"#,
        )
        .expect("valid CurseForge search hit");

        let project = ModProject::from_curseforge(hit);

        assert_eq!(project.source, ModSource::CurseForge);
        assert_eq!(project.author, "Example Author");
        assert_eq!(project.game_version, "1.20.1");
        assert_eq!(project.loaders, "Fabric");
        assert!(project.categories.contains(&ModCategory::Technology));
        assert_eq!(
            project.page_url,
            "https://www.curseforge.com/minecraft/mc-mods/example-mod"
        );
    }

    #[test]
    fn query_encoding_handles_spaces_symbols_and_unicode() {
        assert_eq!(
            encode_query_component("Sodium + 日本語"),
            "Sodium%20%2B%20%E6%97%A5%E6%9C%AC%E8%AA%9E"
        );
    }
}
