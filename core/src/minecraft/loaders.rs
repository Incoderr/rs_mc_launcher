use std::collections::BTreeMap;

use serde::Deserialize;

pub const FORGE_PROMOTIONS_URL: &str =
    "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json";
pub const FABRIC_LOADER_MANIFEST_URL: &str = "https://meta.fabricmc.net/v2/versions/loader";
pub const NEOFORGE_MAVEN_METADATA_URL: &str =
    "https://maven.neoforged.net/releases/net/neoforged/neoforge/maven-metadata.xml";

#[derive(Clone, Debug)]
pub struct ForgeManifest {
    pub promotions: Vec<ForgePromotion>,
}

impl ForgeManifest {
    pub fn parse(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        let raw: ForgeManifestData = serde_json::from_slice(bytes)?;
        let promotions = raw
            .promos
            .into_iter()
            .filter_map(|(key, version)| {
                let (minecraft_version, channel) = key
                    .strip_suffix("-recommended")
                    .map(|version| (version, ForgeChannel::Recommended))
                    .or_else(|| {
                        key.strip_suffix("-latest")
                            .map(|version| (version, ForgeChannel::Latest))
                    })?;

                Some(ForgePromotion {
                    minecraft_version: minecraft_version.to_owned(),
                    channel,
                    version,
                })
            })
            .collect();

        Ok(Self { promotions })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ForgeChannel {
    Recommended,
    Latest,
}

#[derive(Clone, Debug)]
pub struct ForgePromotion {
    pub minecraft_version: String,
    pub channel: ForgeChannel,
    pub version: String,
}

#[derive(Deserialize)]
struct ForgeManifestData {
    #[serde(default)]
    promos: BTreeMap<String, String>,
}

#[derive(Clone, Debug)]
pub struct FabricLoaderManifest {
    pub versions: Vec<FabricLoaderVersion>,
}

impl FabricLoaderManifest {
    pub fn parse(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        let versions = serde_json::from_slice(bytes)?;
        Ok(Self { versions })
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct FabricLoaderVersion {
    pub version: String,
    pub stable: bool,
    pub maven: String,
}

#[derive(Clone, Debug)]
pub struct FabricGameLoaderManifest {
    pub versions: Vec<FabricLoaderVersion>,
}

impl FabricGameLoaderManifest {
    pub fn parse(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        let compatible_loaders: Vec<FabricCompatibleLoader> = serde_json::from_slice(bytes)?;
        let versions = compatible_loaders
            .into_iter()
            .map(|entry| entry.loader)
            .collect();
        Ok(Self { versions })
    }
}

#[derive(Deserialize)]
struct FabricCompatibleLoader {
    loader: FabricLoaderVersion,
}

#[derive(Clone, Debug)]
pub struct NeoForgeManifest {
    pub versions: Vec<NeoForgeVersion>,
}

impl NeoForgeManifest {
    pub fn parse(xml: &str) -> Result<Self, String> {
        let metadata: MavenMetadata = quick_xml::de::from_str(xml)
            .map_err(|error| format!("Не удалось разобрать Maven metadata NeoForge: {error}"))?;
        let versions = metadata
            .versioning
            .versions
            .version
            .into_iter()
            .filter_map(|version| {
                Some(NeoForgeVersion {
                    minecraft_version: minecraft_version_for_neoforge(&version)?,
                    version,
                })
            })
            .collect();

        Ok(Self { versions })
    }
}

#[derive(Clone, Debug)]
pub struct NeoForgeVersion {
    pub minecraft_version: String,
    pub version: String,
}

#[derive(Deserialize)]
struct MavenMetadata {
    versioning: MavenVersioning,
}

#[derive(Deserialize)]
struct MavenVersioning {
    versions: MavenVersions,
}

#[derive(Deserialize)]
struct MavenVersions {
    #[serde(rename = "version", default)]
    version: Vec<String>,
}

fn minecraft_version_for_neoforge(version: &str) -> Option<String> {
    let release = version.split('-').next()?;
    let parts: Vec<_> = release.split('.').collect();
    let major: u32 = parts.first()?.parse().ok()?;
    let minor: u32 = parts.get(1)?.parse().ok()?;

    if major == 1 {
        let patch: u32 = parts.get(2)?.parse().ok()?;
        Some(format!("1.{minor}.{patch}"))
    } else if (20..=25).contains(&major) {
        Some(format!("1.{major}.{minor}"))
    } else if major >= 26 {
        Some(format!("{major}.{minor}"))
    } else {
        None
    }
}
