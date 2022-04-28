use crate::chapter::{ Chapter, ChapterError, ChapterDownloadError, ChapterMetadata };
use crate::manga::MangaMetadata;
use crate::requester::{ RateLimitedRequester, RequesterError };
use crate::types::{ ChapterDataResponse, MangaDataResponse };

use std::path::Path;

use pbr::ProgressBar;
use regex::Regex;
use thiserror::Error;

fn get_id(url:&str) -> Option<String> {
    lazy_static! {
        static ref ID_RE:Regex = Regex::new(r"https?://mangadex\.org/title/((?:[0-9a-fA-F]+-?)+)/?.*").unwrap();
    }

    let id = ID_RE.captures(url)?.get(1)?.as_str().to_string();
    Some(id)
}

#[derive(Debug, Error)]
pub enum APIError {
    #[error("error making request: {0}")]
    Requester(#[from] RequesterError),
    #[error("error decoding response body: {0}")]
    Decoding(#[from] reqwest::Error),
    #[error("manga id could not be parsed from the given url")]
    NoID,
    #[error("error retrieving chapter information: {0}")]
    Chapter(#[from] ChapterError),
    #[error("error downloading chapter images: {0}")]
    ChapterDownload(#[from] ChapterDownloadError),
}

pub struct API {
    requester: RateLimitedRequester,
}
impl API {
    pub fn new() -> Self {
        Self {
            requester: RateLimitedRequester::new_with_defaults(),
        }
    }

    pub async fn get_manga_metadata(&mut self, url:&str) -> Result<MangaMetadata, APIError> {
        let id = get_id(url).ok_or(APIError::NoID)?;
        let raw_manga_data = self.requester.request("main", &format!("/manga/{}", id))
            .await?
            .json::<MangaDataResponse>()
            .await?;

        Ok(MangaMetadata::from_response(id, raw_manga_data))
    }

    pub async fn get_chapter_metadata(&mut self, manga_metadata:&MangaMetadata, quiet:bool) -> Result<Vec<ChapterMetadata>, APIError> {
        let res = self.requester.request("main", &format!("/manga/{}/feed?offset={}", &manga_metadata.id, 0))
            .await?
            .json::<ChapterDataResponse>()
            .await?;

        let mut chapters = ChapterMetadata::from_response(res.data);
        let total = res.total;
        let mut i = res.offset + res.limit;

        let mut pb = match quiet {
            false => Some(ProgressBar::new(total)),
            true => None,
        };

        while i < total {
            let res = self.requester.request("main", &format!("/manga/{}/feed?offset={}", &manga_metadata.id, i))
                .await?
                .json::<ChapterDataResponse>()
                .await?;

            let mut new_chapters = ChapterMetadata::from_response(res.data);
            if let Some(pb) = &mut pb {
                pb.add(new_chapters.len() as u64);
            }
            chapters.append(&mut new_chapters);

            i += res.limit;
        }

        if let Some(pb) = &mut pb {
            pb.finish_print("Chapter metadata downloaded.");
            println!("");
        }

        Ok(chapters)
    }

    pub async fn get_chapters(&mut self, chapter_metadata:&[ChapterMetadata], quiet:bool) -> Result<Vec<Chapter>, APIError> {
        let mut pb = match quiet {
            false => Some(ProgressBar::new(chapter_metadata.len() as u64)),
            true => None,
        };

        let mut chapters = vec![];
        let mut iter = chapter_metadata.iter();
        while let Some(metadata) = iter.next() {
            let chapter = Chapter::new(&mut self.requester, &metadata).await?;
            chapters.push(chapter);

            if let Some(pb) = &mut pb {
                pb.inc();
            }
        }

        if let Some(pb) = &mut pb {
            pb.finish_print("Chapter download data downloaded.");
            println!("");
        }

        Ok(chapters)
    }

    pub async fn download_chapters(&mut self, chapters:&[Chapter], master_directory:&Path, quiet:bool) -> Result<(), APIError> {
        let mut iter = chapters.iter();
        while let Some(chapter) = iter.next() {
            chapter.download_to_folder(&mut self.requester, master_directory, quiet).await?;
        }

        Ok(())
    }
}
