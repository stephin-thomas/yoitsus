use crate::Context;
use anyhow::Context as AnyhowContext;
use songbird::Call;
use std::sync::Arc;

pub async fn join_n_get_voice_channel_handler(
    ctx: &Context<'_>,
) -> anyhow::Result<Arc<tokio::sync::Mutex<Call>>, anyhow::Error> {
    let manager = songbird::get(ctx.serenity_context())
        .await
        .ok_or(anyhow::anyhow!(
            "Songbird Voice client placed in at initialisation."
        ))?;
    let guild_id = ctx
        .guild_id()
        .ok_or_else(|| anyhow::anyhow!("Guild ID not found"))?;
    let channel_id = ctx
        .guild()
        .expect("None value when getting guild")
        .voice_states
        .get(&ctx.author().id)
        .expect("Errro getting voice states based on author id")
        .channel_id
        .ok_or(anyhow::anyhow!(
            "No channel Id found for voice channel 
    are you connected to voice channel or the guild has one?
        "
        ))?;
    let _joined_voice_channel = manager
        .join(guild_id, channel_id.clone())
        .await
        .context("Error joining the channel")?;
    manager.get(guild_id).ok_or(anyhow::anyhow!(
        "Error getting the handler for the guild, check if you are in a voice channel"
    ))
}

pub async fn get_voice_handler(
    ctx: &Context<'_>,
) -> anyhow::Result<Arc<tokio::sync::Mutex<Call>>, anyhow::Error> {
    let manager = songbird::get(ctx.serenity_context())
        .await
        .ok_or(anyhow::anyhow!(
            "Songbird Voice client placed in at initialisation."
        ))?;
    let guild_id = ctx
        .guild_id()
        .ok_or_else(|| anyhow::anyhow!("Guild ID not found"))?;
    let handler = manager
        .get(guild_id)
        .ok_or(anyhow::anyhow!("Error getting the handler for the guild"))?;
    Ok(handler)
}
