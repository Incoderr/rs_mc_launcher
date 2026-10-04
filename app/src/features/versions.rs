use std::sync::Arc;
use std::time::Duration;

use gpui_kit::http_client::{AsyncBody, HttpClient, HttpRequestExt, Request};
use rs_mc_launcher_core::minecraft::manifest::{VERSION_MANIFEST_URL, VersionManifest};

#[derive(Debug, Default)]
pub struct VersionCatalogState {
    loading: bool,
    manifest: Option<VersionManifest>,
    error: Option<String>,
    selected_version: Option<String>,
}

impl VersionCatalogState {
    pub fn is_loading(&self) -> bool {
        self.loading
    }

    pub fn manifest(&self) -> Option<&VersionManifest> {
        self.manifest.as_ref()
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn selected_version(&self) -> Option<&str> {
        self.selected_version.as_deref()
    }

    pub fn begin_loading(&mut self) -> bool {
        if self.loading || self.manifest.is_some() {
            return false;
        }

        self.loading = true;
        self.error = None;
        true
    }

    pub fn finish_loading(&mut self, result: Result<VersionManifest, String>) {
        self.loading = false;
        match result {
            Ok(manifest) => {
                self.manifest = Some(manifest);
                self.error = None;
            }
            Err(error) => self.error = Some(error),
        }
    }

    pub fn select(&mut self, version: String) {
        self.selected_version = Some(version);
    }
}

pub async fn fetch_manifest(client: Arc<dyn HttpClient>) -> Result<VersionManifest, String> {
    let request = Request::builder()
        .uri(VERSION_MANIFEST_URL)
        .timeout(Duration::from_secs(20))
        .body(AsyncBody::empty())
        .map_err(|error| format!("Не удалось подготовить запрос манифеста: {error}"))?;

    let response = client
        .send(request)
        .await
        .map_err(|error| format!("Не удалось загрузить манифест Minecraft: {error}"))?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("Сервер манифеста вернул HTTP {status}"));
    }

    let mut body = response.into_body();
    let mut bytes = Vec::new();
    futures::io::AsyncReadExt::read_to_end(&mut body, &mut bytes)
        .await
        .map_err(|error| format!("Не удалось прочитать манифест Minecraft: {error}"))?;

    VersionManifest::parse(&bytes)
        .map_err(|error| format!("Не удалось разобрать манифест Minecraft: {error}"))
}
