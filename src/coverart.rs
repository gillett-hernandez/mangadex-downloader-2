use crate::chapter::ImageDownloadError;
use crate::requester::RateLimitedRequester;
use crate::types::CoverArtData;
use crate::utils;

use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct CoverArt {
    pub volume: String,
    pub url: String,
}
impl CoverArt {
    pub fn from_data(id: &str, data: CoverArtData) -> Option<Self> {
        Some(Self {
            volume: data.attributes.volume?,
            url: format!("{}/{}", id, data.attributes.file_name),
        })
    }

    pub fn from_response(id: &str, mut raw: Vec<CoverArtData>) -> Vec<Self> {
        raw.drain(..)
            .map(|r| Self::from_data(id, r))
            .filter(|ca| ca.is_some())
            .map(|ca| ca.unwrap())
            .collect::<Vec<Self>>()
    }

    pub fn get_volume(&self) -> String {
        match self.volume.parse::<f64>() {
            Ok(v) => format!("Volume {}", v),
            Err(_) => self.volume.clone(),
        }
    }

    pub async fn download(
        &self,
        requester: &mut RateLimitedRequester,
        master_directory: &Path,
    ) -> Result<(), ImageDownloadError> {
        let master_path = master_directory.join(Path::new(&utils::escape_path(&self.get_volume())));
        fs::create_dir_all(&master_path)?;

        let res = requester
            .request("content", &format!("/covers/{}", &self.url))
            .await?;
        let content_type = res
            .headers()
            .get("Content-Type")
            .ok_or(ImageDownloadError::NoContentType)?
            .to_str()?;

        let Some(extension) = mime2ext::mime2ext(content_type) else {
                return Err(ImageDownloadError::Mime);
            };

        let body = res.bytes().await?;

        // I'm too lazy to do async file io
        let path = master_path.join(Path::new(&format!("cover.{}", extension)));
        let mut file = File::create(path)?;
        let _ = file.write_all(&body)?;

        Ok(())
    }
}
