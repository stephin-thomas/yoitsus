use std::sync::Arc;

use super::common::join_n_get_voice_channel_handler;
use super::helpers::get_yt_sources;
use crate::Context;
use crate::Error;
use crate::HttpClient;
use crate::HttpKey;
use anyhow::Context as AnyhowContext;
use anyhow::Result;
use anyhow::anyhow;
use poise;
use poise::serenity_prelude as serenity;
use poise::serenity_prelude::ActivityData;
use poise::serenity_prelude::CreateEmbed;
use poise::serenity_prelude::CreateMessage;
use poise::serenity_prelude::Timestamp;
use songbird::Event;
use songbird::EventContext;
use songbird::EventHandler as VoiceEventHandler;
use songbird::TrackEvent;
use songbird::input::AuxMetadata;
use songbird::input::Compose;
use songbird::input::YoutubeDl;
use songbird::tracks::PlayMode;
use songbird::tracks::TrackHandle;
use tokio::sync::Mutex;
use tracing::info;
#[derive(Clone)]
struct SongEndNotifier {
    chan_id: serenity::ChannelId,
    guild_id: serenity::GuildId,
    mgr: Arc<songbird::Songbird>,
    http: Arc<serenity::Http>,
    next: Arc<Mutex<Vec<YoutubeDl<'static>>>>,
}

fn check_msg(result: serenity::Result<serenity::Message>) {
    if let Err(why) = result {
        println!("Error sending message: {:?}", why);
    }
}

#[serenity::async_trait]
impl<'a> VoiceEventHandler for SongEndNotifier {
    async fn act(&self, _ctx: &EventContext<'_>) -> Option<Event> {
        let next_song = self.next.lock().await.pop();
        let queue_len = self.next.lock().await.len();
        info!("Event Queue length is {:?}", queue_len);
        if let Some(mut next_song) = next_song {
            check_msg(
                self.chan_id
                    .say(
                        &self.http,
                        format!(
                            "Playing next song {}",
                            &next_song.aux_metadata().await.unwrap().title.unwrap()
                        ),
                    )
                    .await,
            );

            info!("Adding Next song, Next song is {:?}", next_song);
            let handler = self
                .mgr
                .get(self.guild_id)
                .ok_or(anyhow::anyhow!("Error getting the handler for the guild"))
                .unwrap();

            let track_handle = handler.lock().await.enqueue_input(next_song.into()).await;
            let _ = track_handle.add_event(
                Event::Track(TrackEvent::End),
                self.clone(),
                // SongEndNotifier {
                //     chan_id:self.chan_id,
                //     guild_id:self.guild_id,
                //     mgr:self.mgr.clone(),
                //     http: self.http.clone(),
                //     next: self.next.clone(),
                // },
            );
        } else {
            self.chan_id
                .say(&self.http, "No more songs found, ending the queue")
                .await
                .ok();
        }
        //        let track=self.next.pop();

        None
    }
}

async fn get_http_client(ctx: &serenity::Context) -> HttpClient {
    let data = ctx.data.read().await;
    data.get::<HttpKey>()
        .cloned()
        .expect("Guaranteed to exist in the typemap.")
}

async fn autocomplete_search<'a>(
    ctx: Context<'_>,
    partial: &'a str,
) -> impl Iterator<Item = String> {
    if partial.len() < 3 {
        return vec!["music".to_string()].into_iter();
    }
    let http_client = get_http_client(ctx.serenity_context()).await;
    let sources = YoutubeDl::new_search(http_client, partial)
        .search(Some(5))
        .await;
    match sources {
        Ok(suggestions) => suggestions
            .into_iter()
            .map(|x| x.title.unwrap_or("No suggestion".to_owned()))
            .collect::<Vec<String>>()
            .into_iter(),

        Err(err) => {
            tracing::info!("Error getting search suggestions: {:?}", err);
            let default = vec!["music".to_string()];
            default.into_iter()
        }
    }
    //let mut sources = get_yt_sources(http_client, url).await.unwrap();
}

#[poise::command(slash_command, prefix_command, guild_only)]
pub async fn play(
    ctx: Context<'_>,
    #[autocomplete = autocomplete_search]
    #[description = "Play / queue a song from a YouTube URL"]
    url: String,
) -> Result<(), Error> {
    let mut track_handle: Option<TrackHandle> = None;
    {
        let handler_lock = join_n_get_voice_channel_handler(&ctx).await?;
        let http_client = get_http_client(ctx.serenity_context()).await;
        let _msg = ctx
            .channel_id()
            .send_message(&ctx.http(), CreateMessage::new().content("Searching"))
            .await?;

        let mut sources = get_yt_sources(http_client, url).await?;
        sources.reverse();
        let playlist_len = if sources.len() > 1 {
            Some(sources.len())
        } else {
            None
        };

        let mut handler = handler_lock.lock().await;
        let metadata = sources[0]
            .aux_metadata()
            .await
            .context("Error getting metadata from the input")?;
        info!("Playing song: {:?}", &metadata.title);
        ctx.serenity_context().set_presence(
            Some(ActivityData::playing(metadata.title.as_ref().unwrap())),
            serenity::OnlineStatus::Online,
        );
        let embed = create_song_embed(metadata, playlist_len, handler.queue().len()).await;
        let builder = CreateMessage::new().content("Music!").embed(embed);
        // let handle= handler.play_input(src.clone().into());
        let _msg = ctx.channel_id().send_message(&ctx.http(), builder).await?;
        handler.queue().stop();
        //let mut data=ctx.invocation_data::<Data>().await.unwrap();
        //let mut data = ctx.data().queue.lock().await;

        let chan_id = ctx.channel_id();
        let guild_id = ctx.guild_id().unwrap();
        //add first song to the queue
        if let Some(track_url) = sources.pop() {
            let playing_track_handle = handler.enqueue_input(track_url.into()).await;
            let mgr = songbird::get(ctx.serenity_context())
                .await
                .ok_or(anyhow::anyhow!(
                    "Songbird Voice client placed in at initialisation."
                ))
                .unwrap();
            //let local_queue=Arc::new(Mutex::new(sources)) ;
            let data = ctx.data();
            {
                let mut queue = data.queue.lock().await;
                queue.clear();
                queue.append(&mut sources);
            }
            let _ = playing_track_handle.add_event(
                Event::Track(TrackEvent::End),
                SongEndNotifier {
                    chan_id,
                    guild_id,
                    mgr,
                    http: ctx.serenity_context().http.clone(),
                    next: data.queue.clone(),
                },
            );

            track_handle = Some(
                handler
                    .queue()
                    .current()
                    .ok_or(anyhow!("Error getting currently playing track"))?,
            );
            //     drop(handler);
            //     drop(handler_lock);
        }
    }
    if let Some(track_handle) = track_handle {
        if track_handle.get_info().await?.playing == PlayMode::Play {
        } else {
            track_handle.play()?;
            drop(track_handle)
        }
    }

    return Ok(());
}

#[poise::command(slash_command, prefix_command, guild_only)]
pub async fn queue(
    ctx: Context<'_>,
    #[description = "Add to queue a song from a YouTube URL"] url: String,
) -> Result<(), Error> {
    let handler_lock = join_n_get_voice_channel_handler(&ctx).await?;
    let http_client = get_http_client(ctx.serenity_context()).await;
    let sources = get_yt_sources(http_client, url).await?;
    let playlist_len = if sources.len() > 1 {
        Some(sources.len())
    } else {
        None
    };
    let mut handler = handler_lock.lock().await;
    let len = handler.queue().len() + playlist_len.unwrap_or(0);
    ctx.say(format!(
        "Adding to queue...\nCurrent Len :- {}\n Final Len:-{}",
        handler.queue().len(),
        len
    ))
    .await?;
    for input in sources.into_iter() {
        let _track_handle = handler.enqueue_input(input.into()).await;
    }
    let track_handle = handler
        .queue()
        .current()
        .ok_or(anyhow!("Error getting currently playing track"))?;
    drop(handler);
    drop(handler_lock);
    if track_handle.get_info().await?.playing == PlayMode::Play {
        return Ok(());
    } else {
        track_handle.play()?;
        return Ok(());
    }
}

async fn create_song_embed(
    metadata: AuxMetadata,
    added: Option<usize>,
    queue_len: usize,
) -> CreateEmbed {
    let playtime = metadata.duration.unwrap_or_default();
    let title;
    if let Some(added) = added {
        title = format!(":notes: Playlist added to the queue!- {} songs", added);
    } else {
        title = format!(":notes: Song added to the queue!");
    }

    let embed = CreateEmbed::new()
        .colour(0xffffff)
        .title(title)
        .thumbnail(metadata.thumbnail.unwrap_or_else(|| {
            String::from(
                "https://images.unsplash.com/photo-1611162616475-46b635cb6868?ixlib=rb-4.0.3",
            )
        }))
        .description(format!(
            "{} - {}",
            metadata.title.clone().unwrap(),
            metadata.artist.clone().unwrap()
        ))
        .fields(vec![
            (
                "Songs queued",
                format!("{}", queue_len + added.unwrap_or(0)),
                true,
            ),
            ("Total playtime", playtime.as_secs().to_string(), true),
        ])
        .timestamp(Timestamp::now());
    return embed;
}
