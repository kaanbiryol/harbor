use gpui::{AssetSource, Result, SharedString};
use rust_embed::RustEmbed;
use std::borrow::Cow;

#[derive(RustEmbed)]
#[folder = "../ui/assets"]
#[include = "icons/**/*.svg"]
#[include = "fonts/ibm-plex-sans/*"]
#[include = "fonts/lilex/*"]
struct HarborUiAssets;

pub(crate) struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if path.is_empty() {
            return Ok(None);
        }

        if let Some(asset) = HarborUiAssets::get(path) {
            return Ok(Some(asset.data));
        }

        gpui_component_assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut assets: Vec<SharedString> = HarborUiAssets::iter()
            .filter_map(|asset_path| {
                asset_path
                    .starts_with(path)
                    .then(|| SharedString::from(asset_path.into_owned()))
            })
            .collect();
        assets.extend(gpui_component_assets::Assets.list(path)?);
        Ok(assets)
    }
}
