use super::funts::create_now_playing_embed;
use super::{common::join_n_get_voice_channel_handler, helpers::get_yt_sources};
use crate::{Context, Error, HttpClient, HttpKey};
use anyhow::{Context as AnyhowContext, Result, anyhow};
use poise::serenity_prelude::{ActivityData, EditMessage, Message};
use poise::{self, CreateReply, serenity_prelude as serenity};
use songbird::{
    Event, EventContext, EventHandler as VoiceEventHandler, TrackEvent,
    input::{AuxMetadata, Compose, YoutubeDl},
    tracks::{PlayMode, TrackHandle},
};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::{info, warn};

#[poise::command(slash_command, prefix_command, guild_only, track_edits)]
pub async fn play(
    ctx: Context<'_>,
    #[autocomplete = autocomplete_search]
    #[description = "Play / queue a song from a YouTube URL"]
    url: String,
) -> Result<(), Error> {
    let _ = ctx.defer().await?;
    let track_handle: TrackHandle = add_songs(ctx, url, false).await?;

    if track_handle.get_info().await?.playing == PlayMode::Play {
    } else {
        track_handle.play()?;
    }

    return Ok(());
}

#[poise::command(slash_command, prefix_command, guild_only, track_edits)]
pub async fn add_to_queue(
    ctx: Context<'_>,
    #[autocomplete = autocomplete_search]
    #[description = "Add to queue a song from a YouTube URL"]
    url: String,
) -> Result<(), Error> {
    //let contxt = Arc::new(ctx.clone());
    let _ = ctx.defer().await?;
    let track_handle: TrackHandle = add_songs(ctx, url, true).await?;

    if track_handle.get_info().await?.playing == PlayMode::Play {
    } else {
        track_handle.play()?;
    }

    return Ok(());
}

#[derive(Clone)]
struct AudioProgressNotifier {
    //ctx: Arc<crate::Context<'a>>,
    msg: Arc<Mutex<Option<Message>>>,
    cur_song: Arc<Mutex<Option<AuxMetadata>>>,
    guild_id: serenity::GuildId,
    mgr: Arc<songbird::Songbird>,
    http: Arc<serenity::Http>,
}

#[serenity::async_trait]
impl<'a> VoiceEventHandler for AudioProgressNotifier {
    async fn act(&self, _ctx: &EventContext<'_>) -> Option<Event> {
        //info!("Periodic event triggered");
        if self.msg.lock().await.is_none() || self.cur_song.lock().await.is_none() {
            return Some(Event::Cancel);
        }
        if let Some(msg) = self.msg.lock().await.as_mut() {
            let handler = self
                .mgr
                .get(self.guild_id)
                .ok_or(anyhow::anyhow!("Error getting the handler for the guild"))
                .unwrap();
            if let Some(metadata) = self.cur_song.lock().await.as_ref() {
                let track_state = handler
                    .lock()
                    .await
                    .queue()
                    .current()
                    .unwrap()
                    .get_info()
                    .await
                    .unwrap();
                if track_state.playing == PlayMode::Stop
                    || track_state.playing == PlayMode::Pause
                    || track_state.playing == PlayMode::End
                {
                    let embed = create_now_playing_embed(metadata, &track_state).await;
                    let edit_builder = EditMessage::default().embed(embed);
                    msg.edit(&self.http, edit_builder).await.ok();
                    return Some(Event::Cancel);
                }
                let embed = create_now_playing_embed(metadata, &track_state).await;
                let edit_builder = EditMessage::default().embed(embed);
                msg.edit(&self.http, edit_builder).await.ok();
            }
        }
        None
    }
}

#[derive(Clone)]
struct SongEndNotifier {
    //ctx: Arc<crate::Context<'a>>,
    chan_id: serenity::ChannelId,
    guild_id: serenity::GuildId,
    mgr: Arc<songbird::Songbird>,
    http: Arc<serenity::Http>,
    next: Arc<Mutex<Vec<YoutubeDl<'static>>>>,
    msg: Arc<Mutex<Option<Message>>>,
    cur_song: Arc<Mutex<Option<AuxMetadata>>>,
}

// fn check_msg(result: serenity::Result<serenity::Message>) {
//     if let Err(why) = result {
//         println!("Error sending message: {:?}", why);
//     }
// }

#[serenity::async_trait]
impl VoiceEventHandler for SongEndNotifier {
    async fn act(&self, _ctx: &EventContext<'_>) -> Option<Event> {
        let next_song = self.next.lock().await.pop();
        let queue_len = self.next.lock().await.len();
        info!("Event Queue length is {:?}", queue_len);
        if let Some(mut next_song) = next_song {
            info!("Adding Next song, Next song is {:?}", &next_song);
            if let Ok(metadata) = next_song
                .aux_metadata()
                .await
                .context("Error getting metadata from the input")
            {
                self.add_track(&metadata, next_song).await;
            } else {
                info!("Error getting metadata from the input");
                self.act(_ctx).await;
            };
        } else {
            //     self.cha
            //         .say(&self.http, "No more songs found, ending the queue")
            //         .await
            //         .ok();
        }

        None
    }
}
impl<'a> SongEndNotifier {
    async fn add_track(&self, metadata: &AuxMetadata, input: YoutubeDl<'static>) {
        let handler = self
            .mgr
            .get(self.guild_id)
            .ok_or(anyhow::anyhow!("Error getting the handler for the guild"))
            .unwrap();
        let track_handle = handler.lock().await.enqueue_input(input.into()).await;
        let track_state = handler
            .lock()
            .await
            .queue()
            .current()
            .unwrap()
            .get_info()
            .await
            .unwrap();
        let embed = create_now_playing_embed(&metadata, &track_state).await;
        let edit_builder = EditMessage::default().embed(embed);
        //let edit_builder = EditMessage::new().embed(embed);
        let mut now_playing_msg = self.msg.lock().await;
        now_playing_msg
            .as_mut()
            .unwrap()
            .edit(&self.http, edit_builder)
            .await
            .ok();
        self.cur_song.lock().await.replace(metadata.clone());

        let _ = track_handle
            .add_event(
                Event::Track(TrackEvent::End),
                // self.clone()
                SongEndNotifier {
                    chan_id: self.chan_id,
                    guild_id: self.guild_id,
                    mgr: self.mgr.clone(),
                    //ctx: Arc::clone(&self.ctx),
                    http: self.http.clone(),
                    next: self.next.clone(),
                    msg: self.msg.clone(),
                    cur_song: self.cur_song.clone(),
                },
            )
            .map_err(|err| warn!("Error adding track end event: {:?}", err));
        let _ = track_handle
            .add_event(
                Event::Periodic(Duration::from_secs(1), None),
                AudioProgressNotifier {
                    guild_id: self.guild_id,
                    mgr: self.mgr.clone(),
                    //ctx:ctx,
                    http: self.http.clone(),
                    msg: self.msg.clone(),
                    cur_song: self.cur_song.clone(),
                },
            )
            .map_err(|err| warn!("Error adding periodic event: {:?}", err));
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

async fn add_songs(
    ctx: Context<'_>,
    url: String,
    add_to_queue: bool,
) -> Result<TrackHandle, Error> {
    let handler_lock = join_n_get_voice_channel_handler(&ctx).await?;
    let http_client = get_http_client(ctx.serenity_context()).await;
    // let _msg = ctx
    //     .channel_id()
    //     .send_message(&ctx.http(), CreateMessage::new().content("Searching"))
    //     .await?;

    let mut sources = get_yt_sources(http_client, url).await?;
    sources.reverse();
    // let playlist_len = if sources.len() > 1 {
    //     Some(sources.len())
    // } else {
    //     None
    // };

    let mut handler = handler_lock.lock().await;

    if !add_to_queue {
        handler.queue().stop();
    }

    let chan_id = ctx.channel_id();
    let guild_id = ctx.guild_id().unwrap();
    //add first song to the queue
    let metadata: AuxMetadata;
    let mut track_url: YoutubeDl<'static>;
    //if let Some(mut track_url) = sources.pop() {
    loop {
        let track_url_res = sources.pop();
        if track_url_res.is_none() {
            return Err(anyhow::anyhow!("No tracks found").into());
        }
        track_url = track_url_res.unwrap();
        let metadata_res = track_url
            .aux_metadata()
            .await
            .context("Error getting metadata from the input");
        if metadata_res.is_err() {
            let err = metadata_res.as_ref().err().unwrap();
            warn!("Error getting metadata from the input: {:?} skipping", err);
            continue;
        } else {
            metadata = metadata_res?;
            break;
        }
    }
    info!("Playing song: {:?}", &metadata.title);

    let playing_track_handle = handler.enqueue_input(track_url.clone().into()).await;
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
        if !add_to_queue {
            queue.clear();
        }
        queue.append(&mut sources);
        // let mut cur_song = data.cur_song.lock().await;
        // cur_song.replace(track_url.clone());
        let mut now_playing_msg = data.now_playing_msg.lock().await;

        if now_playing_msg.is_none() {
            ctx.serenity_context().set_presence(
                Some(ActivityData::playing(metadata.title.as_ref().unwrap())),
                serenity::OnlineStatus::Online,
            );
            let msg_string = format!(
                "Now playing: {} - {}",
                metadata.title.as_ref().unwrap(),
                metadata.artist.as_ref().unwrap()
            );
            let poise_builder = CreateReply::default().content(msg_string).ephemeral(false);

            //            let poise_builder = CreateReply::default().embed(embed.clone()).ephemeral(false);
            let poise_reply_msg = poise::send_reply(ctx, poise_builder).await?;
            let poise_msg = poise_reply_msg.into_message().await?;

            // let builder = CreateMessage::new().content("Music!").embed(embed);
            //let builder = CreateMessage::new().embed(embed);
            //let msg = ctx.channel_id().send_message(&ctx.http(), builder).await?;
            //let msg_obj =Arc::new(Mutex::new(msg.into_message().await?));

            now_playing_msg.replace(poise_msg);
        }
        let now_playing_embed = create_now_playing_embed(
            &metadata,
            &handler.queue().current().unwrap().get_info().await.unwrap(),
        )
        .await;
        let now_playing_builder = EditMessage::default().embed(now_playing_embed);

        now_playing_msg
            .as_mut()
            .unwrap()
            .edit(ctx, now_playing_builder)
            .await
            .ok();
        let cur_song = Arc::new(Mutex::new(Some(metadata)));

        let _ = playing_track_handle.add_event(
            Event::Track(TrackEvent::End),
            SongEndNotifier {
                //ctx: ctx.clone(),
                chan_id,
                guild_id,
                mgr: mgr.clone(),
                http: ctx.serenity_context().http.clone(),
                next: data.queue.clone(),
                msg: data.now_playing_msg.clone(),
                cur_song: cur_song.clone(),
            },
        )?;
        //handler.remove_all_global_events();
        let _ = playing_track_handle.add_event(
            Event::Periodic(Duration::from_secs(1), None),
            AudioProgressNotifier {
                //ctx: ctx.clone(),
                http: ctx.serenity_context().http.clone(),
                msg: data.now_playing_msg.clone(),
                cur_song: cur_song,
                guild_id,
                mgr,
            },
        )?;
    }

    let track_handle = handler
        .queue()
        .current()
        .ok_or(anyhow!("Error getting currently playing track").into());
    return track_handle;
    //     drop(handler);
    //     drop(handler_lock);
}

// async fn create_song_embed(
//     metadata: &AuxMetadata,
//     added: Option<usize>,
//     queue_len: usize,
// ) -> CreateEmbed {
//     let playtime = metadata.duration.unwrap_or_default();
//     let title;
//     if let Some(added) = added {
//         title = format!(":notes: Playlist added to the queue!- {} songs", added);
//     } else {
//         title = format!(":notes: Song added to the queue!");
//     }
//     let def_thumbnail =
//         "https://images.unsplash.com/photo-1611162616475-46b635cb6868?ixlib=rb-4.0.3".to_owned();

//     let embed = CreateEmbed::new()
//         .colour(0xffffff)
//         .title(title)
//         .thumbnail(
//             metadata
//                 .thumbnail
//                 .as_ref()
//                 .unwrap_or_else(|| &def_thumbnail),
//         )
//         .description(format!(
//             "{} - {}",
//             metadata.title.clone().unwrap(),
//             metadata.artist.clone().unwrap()
//         ))
//         .fields(vec![
//             (
//                 "Songs queued",
//                 format!("{}", queue_len + added.unwrap_or(0)),
//                 true,
//             ),
//             //playtime as minutes and seconds.
//             (
//                 "Total playtime",
//                 format!("{}m {}s", playtime.as_secs() / 60, playtime.as_secs() % 60),
//                 true,
//             ),
//         ])
//         .timestamp(Timestamp::now());
//     return embed;
// }
