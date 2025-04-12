use std::io::ErrorKind;

use anyhow::Context;
use songbird::input::{AudioStreamError, AuxMetadata, YoutubeDl};
use tokio::process::Command;
use tracing::info;
use url::Url;

#[derive(PartialEq, Debug)]
pub enum ParseYtLink {
    Song,
    Playlist,
    NotYoutube,
    Channel,
    User,
    Live,
    Search,
    Shorts,
}

pub(crate) fn is_youtube_link(url_string: &str) -> ParseYtLink {
    match Url::parse(url_string) {
        Ok(url) => {
            if let Some(host) = url.host_str() {
                if host.ends_with("youtube.com") || host == "youtu.be" {
                    let path = url.path();
                    if host.ends_with("youtube.com") {
                        if path.starts_with("/watch")
                            && url.query_pairs().any(|(key, _)| key == "v")
                        {
                            return ParseYtLink::Song;
                        } else if path.starts_with("/watch")
                            && url.query_pairs().any(|(key, _)| key == "list")
                        {
                            return ParseYtLink::Playlist;
                        } else if path.starts_with("/playlist")
                            && url.query_pairs().any(|(key, _)| key == "list")
                        {
                            return ParseYtLink::Playlist;
                        } else if path.starts_with("/channel/") {
                            return ParseYtLink::Channel;
                        } else if path.starts_with("/user/") {
                            return ParseYtLink::User;
                        } else if path.starts_with("/c/") {
                            return ParseYtLink::Channel;
                        } else if path.starts_with("/live/") {
                            return ParseYtLink::Live;
                        } else if path == "/shorts/{}" && path.split('/').count() == 3 {
                            // Basic check for shorts, might need refinement
                            return ParseYtLink::Shorts;
                        }
                    } else if host == "youtu.be" {
                        if path.starts_with("/") && path.split('/').count() == 2 {
                            return ParseYtLink::Song;
                        } else if path.starts_with("/live/") {
                            return ParseYtLink::Live;
                        } else if path == "/shorts/{}" && path.split('/').count() == 3 {
                            // Basic check for shorts, might need refinement
                            return ParseYtLink::Shorts;
                        }
                    }
                    return ParseYtLink::Song;
                }
                return ParseYtLink::NotYoutube;
            } else {
                return ParseYtLink::Search;
            }
        }

        Err(_) => return ParseYtLink::Search,
    }
}

use serde::{Deserialize, Serialize};
use serde_json;

// #[derive(Serialize, Deserialize, Debug, PartialEq)]
// pub struct Thumbnail {
//     url: String,
//     height: u32,
//     width: u32,
// }

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct YoutubeVideo {
    //pub id: String, // Or u32/u64 based on your actual data type
    pub title: String,
    //pub description: Option<String>,
    // pub thumbnails: Vec<Thumbnail>,
    //pub view_count: u64,
    pub url: String,
    // Add other relevant fields based on the JSON structure
    // This is just a basic structure to show how to map fields
}

pub trait YoutubeDlExt<'a> {
    async fn search_playlist(q: &str) -> anyhow::Result<Vec<YoutubeVideo>>;
}
impl<'a> YoutubeDlExt<'a> for YoutubeDl<'a> {
    async fn search_playlist(q: &str) -> anyhow::Result<Vec<YoutubeVideo>> {
        let ytdl_args = ["-j", q, "--flat-playlist"];
        let mut command = Command::new("yt-dlp");
        let cmd = command.args(ytdl_args);

        let output = cmd.output().await.map_err(|e| {
            AudioStreamError::Fail(if e.kind() == ErrorKind::NotFound {
                format!("could not find executable '{}' on path", "yt-dlp").into()
            } else {
                Box::new(e)
            })
        })?;

        if !output.status.success() {
            print!("{:?}", cmd);
            print!(
                "{} failed with non-zero status code: {}",
                "yt-dlp",
                std::str::from_utf8(&output.stderr[..]).unwrap_or("<no error message>")
            );
            return Err(anyhow::anyhow!("Unsuceessful getting ouput"));
        }
        //split output at new line
        let output = output
            .stdout
            .split(|&b| b == b'\n')
            .filter(|&x| (!x.is_empty()))
            .map(serde_json::from_slice)
            .map(|x| x.context("Errr serializing youtube video entry").unwrap())
            .collect::<Vec<YoutubeVideo>>();
        return Ok(output);
    }
}

pub async fn get_yt_sources(
    http_client: reqwest::Client,
    url: String,
) -> anyhow::Result<Vec<YoutubeDl<'static>>> {
    info!("Play command called with URL: {}", url);

    let url_type = is_youtube_link(&url);

    println!("Parsed URL: {:?}", url_type);
    let mut sources = Vec::new();
    let gen_search_res = |aux_data: AuxMetadata| -> YoutubeDl<'_> {
        info!("Found playlist file url as {:?}", aux_data);
        YoutubeDl::new(
            http_client.clone(),
            aux_data
                .source_url
                .expect("Error getting source url from search aux data"),
        )
    };

    match url_type {
        ParseYtLink::Search => {
            sources = YoutubeDl::new_search(http_client.clone(), url)
                .search(Some(5))
                .await
                .context("Error searching for the song")?
                .map(gen_search_res)
                .collect()
        }
        ParseYtLink::Playlist => {
            let playlist = YoutubeDl::search_playlist(&url)
                .await
                .context("Error getting playlist")?;
            sources = playlist
                .into_iter()
                .map(|video| YoutubeDl::new(http_client.clone(), video.url))
                .collect();

            //      sources=YoutubeDl::new(http_client.clone(), url).search(Some(5)).await.context("Error searching youtube playlist")?.into_iter().map(gen_search_res).collect();
        }

        ParseYtLink::Song => {
            sources.push(YoutubeDl::new(http_client.clone(), url));
        }
        _ => {
            return Err(anyhow::anyhow!(
                "Error parsing the url to download playlist"
            ));
        }
    }

    return Ok(sources);
}
