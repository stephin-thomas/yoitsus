use super::common::get_voice_handler;
use crate::Context;
use crate::Error;
use anyhow::Result;
use anyhow::anyhow;
use poise;
use poise::serenity_prelude as serenity;
use serenity::CreateEmbed;
use serenity::CreateMessage;
use rand::seq::SliceRandom;

use songbird::input::Compose;
use songbird::tracks::Queued;

#[poise::command(slash_command, prefix_command, guild_only)]
pub async fn next(
    ctx: Context<'_>,
    #[description = "Skip a song in the queue"] _param: Option<String>,
) -> Result<(), Error> {
    let handler_lock = get_voice_handler(&ctx).await?;
    let handler = handler_lock.lock().await;
    handler.queue().skip()?;
    ctx.say("song skipped").await?;
    Ok(())
}
#[poise::command(slash_command, prefix_command, guild_only)]
pub async fn pause(
    ctx: Context<'_>,
    #[description = "pause song in the queue"] _param: Option<String>,
) -> Result<(), Error> {
    let handler_lock = get_voice_handler(&ctx).await?;
    let handler = handler_lock.lock().await;
    handler.queue().pause()?;
    ctx.say("song paused").await?;
    Ok(())
}
#[poise::command(slash_command, prefix_command, guild_only)]
pub async fn resume(
    ctx: Context<'_>,
    #[description = "Resume song in the queue"] _param: Option<String>,
) -> Result<(), Error> {
    let handler_lock = get_voice_handler(&ctx).await?;
    let handler = handler_lock.lock().await;
    handler.queue().resume()?;
    ctx.say("song resumed").await?;
    Ok(())
}

#[poise::command(slash_command, prefix_command, guild_only)]
pub async fn shuffle(
    ctx: Context<'_>,
    #[description = "Resume song in the queue"] _param: Option<String>,
) -> Result<(), Error> {
    let handler_lock = get_voice_handler(&ctx).await?;
    let handler = handler_lock.lock().await;

    handler.queue().modify_queue(|q| {
        let mut rng = rand::rng();
        let mut vec: Vec<Queued> = q.drain(..).collect();
        vec.shuffle(&mut rng);
        q.extend(vec);
    });
    let data = ctx.data();
    {
        //this order is important as rng does not carry across await points
        let mut queue_lock = data.queue.lock().await;
        let mut rng = rand::rng();
        queue_lock.shuffle(&mut rng);
    }

    ctx.say("songs shuffled").await?;
    Ok(())
}

#[poise::command(slash_command, prefix_command, guild_only)]
pub async fn disconnect(
    ctx: Context<'_>,
    #[description = "Disconnect and leave"] _param: Option<String>,
) -> Result<(), Error> {
    let handler_lock = get_voice_handler(&ctx).await?;
    let mut handler = handler_lock.lock().await;
    handler.leave().await?;
    ctx.say("left voice channel").await?;
    Ok(())
}
#[poise::command(slash_command, prefix_command, guild_only)]
pub async fn nowplaying(
    ctx: Context<'_>,
    #[description = "Current Playing Track"] _param: Option<String>,
) -> Result<(), Error> {
    let handler_lock = get_voice_handler(&ctx).await?;
    let handler = handler_lock.lock().await;
    let current_track_info = handler
        .queue()
        .current()
        .ok_or(anyhow::anyhow!("Error getting currently playing track"))?
        .get_info()
        .await?;
    ctx.say(format!("Current Track\n{:#?}", current_track_info))
        .await?;
    Ok(())
}

#[poise::command(slash_command, prefix_command, guild_only)]
pub async fn playlist(
    ctx: Context<'_>,
    #[description = "Show current playlist"] _param: Option<String>,
) -> Result<(), Error> {

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
    let builder = CreateMessage::new().content("Music!").embed(embed);
    let _msg = ctx.channel_id().send_message(&ctx.http(), builder).await?;

    Ok(())
}
