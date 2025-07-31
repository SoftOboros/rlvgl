//! Minimal Lottie animation utilities.
use dotlottie_rs::fms::{DotLottieError, DotLottieManager, Manifest};

/// Extract the animation manifest from a Lottie archive in memory.
pub fn manifest_from_bytes(data: &[u8]) -> Result<Manifest, DotLottieError> {
    let manager = DotLottieManager::new(data)?;
    Ok(manager.manifest().clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;

    const SIMPLE_LOTTIE: &str = "UEsDBBQAAAAIAGit/VrKjHKDLgAAADEAAAANAAAAbWFuaWZlc3QuanNvbqtWSszLzE0syczPK1ayUoiuVspMAdJKGak5OflKtbE6CkplqUXFQGmQqKFSLQBQSwMEFAAAAAgAaK39Wn3redEyAAAAOQAAABUAAABhbmltYXRpb25zL2hlbGxvLmpzb26rVipTslIy1TNX0lFKK1KyMjbQUcosULICUvkQqlzJylBHKQNM5iRWphYVK1lFx9YCAFBLAQIUAxQAAAAIAGit/VrKjHKDLgAAADEAAAANAAAAAAAAAAAAAACAAQAAAABtYW5pZmVzdC5qc29uUEsBAhQDFAAAAAgAaK39Wn3redEyAAAAOQAAABUAAAAAAAAAAAAAAIABWQAAAGFuaW1hdGlvbnMvaGVsbG8uanNvblBLBQYAAAAAAgACAH4AAAC+AAAAAAA=";

    #[test]
    fn parse_manifest() {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(SIMPLE_LOTTIE)
            .unwrap();
        let manifest = manifest_from_bytes(&bytes).unwrap();
        assert_eq!(manifest.animations.len(), 1);
        assert_eq!(manifest.animations[0].id, "hello");
    }
}
