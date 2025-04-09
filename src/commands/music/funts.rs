use std::time::Duration;

use super::common::get_current_voice_chan_handler;
use crate::Context;
use crate::Error;
use crate::commands::music::common::join_n_get_voice_channel_handler;
use anyhow::Result;
use anyhow::anyhow;
use poise;
use poise::CreateReply;
use poise::serenity_prelude as serenity;
use rand::seq::SliceRandom;
use serenity::CreateEmbed;

use songbird::input::AuxMetadata;
use songbird::input::Compose;
use songbird::tracks::LoopState;
use songbird::tracks::Queued;
use songbird::tracks::{PlayMode, TrackState};

async fn show_n_delete_msg(ctx: Context<'_>, msg: &str) -> anyhow::Result<()> {
    let msg = ctx.say(msg).await?;
    tokio::time::sleep(Duration::from_secs(5)).await;
    let _ = msg.delete(ctx).await?;
    Ok(())
}

#[poise::command(slash_command, prefix_command, guild_only, ephemeral)]
/// "Skip currently playing song in the queue"
pub async fn next(ctx: Context<'_>) -> Result<(), Error> {
    let _ = ctx.defer().await?;
    {
        let handler_lock = get_current_voice_chan_handler(&ctx).await?;
        let handler = handler_lock.lock().await;
        handler.queue().skip()?;
    }
    show_n_delete_msg(ctx, "song skipped").await?;
    //ctx.say("song skipped").await?;
    Ok(())
}
#[poise::command(slash_command, prefix_command, guild_only, ephemeral)]
/// "Pause song in the queue"
pub async fn pause(ctx: Context<'_>) -> Result<(), Error> {
    let _ = ctx.defer().await?;
    {
        let handler_lock = get_current_voice_chan_handler(&ctx).await?;
        let handler = handler_lock.lock().await;
        handler.queue().pause()?;
    }
    show_n_delete_msg(ctx, "song paused").await?;

    Ok(())
}
#[poise::command(slash_command, prefix_command, guild_only, ephemeral)]
///"Resume song in the queue"
pub async fn resume(ctx: Context<'_>) -> Result<(), Error> {
    let _ = ctx.defer().await?;
    {
        let handler_lock = get_current_voice_chan_handler(&ctx).await?;
        let handler = handler_lock.lock().await;
        handler.queue().resume()?;
    }
    show_n_delete_msg(ctx, "song resumed").await?;
    Ok(())
}

#[poise::command(slash_command, prefix_command, guild_only, ephemeral)]
///"Shuffle the queue"
pub async fn shuffle(ctx: Context<'_>) -> Result<(), Error> {
    let _ = ctx.defer().await?;
    {
        let handler_lock = get_current_voice_chan_handler(&ctx).await?;
        let handler = handler_lock.lock().await;

        handler.queue().modify_queue(|q| {
            let mut rng = rand::rng();
            let mut vec: Vec<Queued> = q.drain(..).collect();
            vec.shuffle(&mut rng);
            q.extend(vec);
        });
    }
    let data = ctx.data();
    {
        //this order is important as rng does not carry across await points
        let mut queue_lock = data.queue.lock().await;
        let mut rng = rand::rng();
        queue_lock.shuffle(&mut rng);
    }
    show_n_delete_msg(ctx, "queue shuffled").await?;
    Ok(())
}
#[poise::command(slash_command, prefix_command, guild_only, ephemeral)]
///"join a voice channel"
pub async fn join(ctx: Context<'_>) -> Result<(), Error> {
    let _ = ctx.defer().await?;
    let _ = join_n_get_voice_channel_handler(&ctx).await?;
    ctx.say("Joined voice channel").await?;
    Ok(())
}

#[poise::command(slash_command, prefix_command, guild_only, ephemeral)]
///"Disconnect and leave"
pub async fn disconnect(ctx: Context<'_>) -> Result<(), Error> {
    let _ = ctx.defer().await?;
    let handler_lock = get_current_voice_chan_handler(&ctx).await?;
    let mut handler = handler_lock.lock().await;
    handler.leave().await?;
    ctx.say("left voice channel").await?;
    Ok(())
}
#[poise::command(slash_command, prefix_command, guild_only, ephemeral)]
///"loop the current queue"
pub async fn loop_toggle(
    ctx: Context<'_>,
    #[description = "Loop count"] count: Option<usize>,
) -> Result<(), Error> {
    let _ = ctx.defer().await?;
    {
        let handler_lock = get_current_voice_chan_handler(&ctx).await?;
        let handler = handler_lock.lock().await;
        let cur_track_handle = handler
            .queue()
            .current()
            .ok_or(anyhow!("Error getting current track"))?;
        if let Some(count) = count {
            cur_track_handle.loop_for(count)?;
            let loop_msg = format!("song looped for {} times", count);
            show_n_delete_msg(ctx, loop_msg.as_str()).await?;
        } else {
            if cur_track_handle.get_info().await?.loops == LoopState::Finite(0) {
                cur_track_handle.enable_loop()?;
                show_n_delete_msg(ctx, "song looped").await?;
            } else {
                show_n_delete_msg(ctx, "song loop disabled").await?;
                cur_track_handle.disable_loop()?;
            }
        }
        //msg = ctx.say("song resumed").await?;
    }
    ////let msg = ctx.say("song resumed").await?;
    Ok(())
}

// #[poise::command(slash_command, prefix_command, guild_only, track_edits)]
// /// Show now playing status
// pub async fn nowplaying(
//     ctx: Context<'_>,
//     #[description = "Current Playing Track"] _param: Option<String>,
// ) -> Result<(), Error> {
//     let handler_lock = get_current_voice_chan_handler(&ctx).await?;
//     let handler = handler_lock.lock().await;
//     if let Some(mut cur_song) = ctx.data().cur_song.lock().await.clone() {
//     if let Some(current_track_state) = handler
//         .queue()
//         .current()
//         .ok_or(anyhow::anyhow!("Error getting currently playing track"))?
//         .get_info()
//         .await?;
//     if let Some(metadata)=cur_song.lock().await{
//     let embed = create_now_playing_embed(&metadata, &current_track_state).await;
//     let builder = CreateMessage::new().embed(embed);
//     let _msg = ctx.channel_id().send_message(&ctx.http(), builder).await?;
//     Ok(())
// }
// }

#[poise::command(slash_command, prefix_command, guild_only, ephemeral)]
/// This command shows the current playlist of songs in the queue
pub async fn playlist(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer().await?;
    {
        let mut data = ctx.data().queue.lock().await;
        let mut embed = CreateEmbed::new()
            .title("Current Playlist")
            .description("Remaining songs to play");

        for track in data.iter_mut().rev() {
            embed = embed.field(
                "Track",
                track
                    .aux_metadata()
                    .await?
                    .title
                    .ok_or(anyhow!("Error getting title from metadata"))?,
                false,
            );
        }
        let builder = CreateReply::default().embed(embed);
        ctx.send(builder).await?;
        //let _msg = ctx.channel_id().send_message(&ctx.http(), builder).await?;
    }
    Ok(())
}

pub(crate) async fn create_now_playing_embed(
    metadata: &AuxMetadata,   // Pass by reference to avoid unnecessary clones
    track_state: &TrackState, // Pass by reference
) -> CreateEmbed {
    let total_duration = metadata.duration.unwrap_or_default();
    let current_position = track_state.position;

    // --- Helper to format Duration into MM:SS ---
    let format_duration = |d: Duration| {
        let total_secs = d.as_secs();
        let minutes = total_secs / 60;
        let seconds = total_secs % 60;
        format!("{:02}:{:02}", minutes, seconds)
    };

    let current_time_str = format_duration(current_position);
    let total_time_str = format_duration(total_duration);

    // --- Calculate Progress Bar ---
    // Creates a text-based progress bar like: `[▬▬▬▬🔘─────]`
    let progress_bar = {
        let percentage = if total_duration.as_secs() > 0 {
            (current_position.as_secs_f64() / total_duration.as_secs_f64())
                .max(0.0) // Ensure percentage doesn't go below 0
                .min(1.0) // Ensure percentage doesn't exceed 1
        } else {
            0.0 // Avoid division by zero if duration is 0
        };

        const BAR_LENGTH: usize = 15; // Total characters for the bar (adjust as needed)
        let filled_len = (percentage * BAR_LENGTH as f64).round() as usize;
        let empty_len = BAR_LENGTH.saturating_sub(filled_len); // Use saturating_sub to prevent underflow

        // Use Unicode block elements for a nicer look
        let filled_char = "▬"; // You can also use '█'
        let empty_char = "─"; // You can also use '░'
        let indicator_char = "🔘"; // Represents the current position head

        // Construct the bar string with the indicator
        if filled_len == 0 {
            format!(
                "`{}{}{}`",
                indicator_char,
                empty_char.repeat(empty_len),
                " ".repeat(filled_char.len())
            ) // Indicator at start
        } else if filled_len >= BAR_LENGTH {
            format!(
                "`{}{}{}`",
                filled_char.repeat(filled_len),
                indicator_char,
                "".repeat(empty_char.len())
            ) // Indicator at end
        } else {
            format!(
                "`{}{}{}{}`",
                filled_char.repeat(filled_len),
                indicator_char,
                empty_char.repeat(empty_len),
                // Pad with spaces to ensure consistent width if indicator pushes characters
                " ".repeat(filled_char.len().saturating_sub(1))
            )
        }
    };

    // --- Determine Play Status Icon ---
    let status_icon = match &track_state.playing {
        PlayMode::Play => "▶️",  // Play icon emoji
        PlayMode::Pause => "⏸️", // Pause icon emoji
        PlayMode::Stop => "⏹️",  // Stop icon emoji
        PlayMode::End => "🏁",   // Ended icon emoji
        PlayMode::Errored(err) => &{
            format!("❓{}", err) // Return the formatted string directly
        }, // Unknown status icon emoji
        _ => "⏹ state unknown",  // Stop or Ended icon emoji
    };

    // --- Build the Embed ---
    let embed_title = format!("{} Now Playing", status_icon);
    let embed_description = format!(
        "**{}**\n{}", // Title bold, artist on new line
        metadata.title.as_deref().unwrap_or("Unknown Title"),
        metadata.artist.as_deref().unwrap_or("Unknown Artist")
    );

    CreateEmbed::new()
        .colour(0x1DB954) // Spotify Green, or choose your preferred color
        .title(embed_title)
        .thumbnail(metadata.thumbnail.clone().unwrap_or_else(|| {
            // Provide a default thumbnail if none is available
            String::from(
                "https://images.unsplash.com/photo-1611162616475-46b635cb6868?ixlib=rb-4.0.3",
            )
        }))
        .description(embed_description)
        .field(
            "Progress",
            // Display the progress bar and the time signature MM:SS / MM:SS
            format!(
                "{} `{} / {}`",
                progress_bar, current_time_str, total_time_str
            ),
            false, // Make the field take the full width
        )
        // Optionally add more fields if needed, e.g., requested by, volume
        .field(
            "Volume",
            format!("{}%", (track_state.volume * 100.0) as u32),
            true,
        )
        .field("Looping", format!("{:?}", track_state.loops), true)
        .timestamp(serenity::Timestamp::now()) // Show when the embed was generated
}
