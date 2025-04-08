use crate::Context;
use crate::Error;
use poise::samples::HelpConfiguration;

// Custom help menu
#[poise::command(prefix_command, track_edits, slash_command)]
pub async fn help(
    ctx: Context<'_>,
    #[description = "Specific command to show help about"]
    #[autocomplete = "poise::builtins::autocomplete_command"]
    mut command: Option<String>,
) -> Result<(), Error> {
    //let prefix = env::var("PREFIX").expect("Set your PREFIX environment variable!");

    // let menu_choice_str: String = match args.single::<String>() {
    //     Ok(menu_choice) => menu_choice,
    //     Err(_) => "default".to_string(),
    // };

    // let menu_choice: &str = &menu_choice_str;

    // msg.channel_id.send_message(&ctx.http, |m| {
    //     m.embed(|e| e
    //         .colour(0xffffff)
    //         .thumbnail("https://i.imgur.com/eWUNoYz.png")
    //         .title("**- -【 Ｈｅｌｐ 】- -**")
    //         .description(format!("Yo. I'm Yoitsus. A :rocket: blazing fast :rocket: rust :rocket: discord bot powered by :rocket: Serenity, Songbird and ChatGPT! :rocket: My prefix is `{}`", prefix))
    //         .fields(
    //             match menu_choice {

    //                 "general" => {
    //                     vec![
    //                         ("help", "Displays this help menu", true),
    //                         ("roll", "Selects a random number from a given range", true),
    //                         ("askgpt", "Ask ChatGPT a question", true),
    //                     ]
    //                 },

    //                 "music" => {
    //                     vec![
    //                         ("join", "Joins a voice channel", true),
    //                         ("leave", "Leaves a music channel", true),
    //                         ("play", "Play / queue a song from a YouTube URL", true),
    //                         ("stop", "Stops current playlist", true),
    //                         ("skip", "Skips the current song", true),
    //                         ("pause", "Pauses the current song", true),
    //                         ("resume", "Resumes the current song", true),
    //                         ("nowplaying", "Shows info about current song", true),
    //                         ("queue", "Show the current queue", true),
    //                         ("shuffle", "Shuffles the current playlist", true),
    //                         ("clear", "Clear the queue", true),
    //                     ]
    //                 },

    //                 _ => {
    //                     vec![
    //                         ("help", "Displays this help menu", false),
    //                         ("help music", "Show music commands", false),
    //                         ("help general", "Show general commands", false),
    //                     ]
    //                 },
    //             }
    //         )
    //         .footer(|f| f.text("Made by Forendes"))
    //         .timestamp(Timestamp::now())
    //     )
    // }).await?;
    if ctx.invoked_command_name() != "help" {
        command = match command {
            Some(c) => Some(format!("{} {}", ctx.invoked_command_name(), c)),
            None => Some(ctx.invoked_command_name().to_string()),
        };
    }
    let extra_text_at_bottom = "\
Type `?help command` for more info on a command.";

    let config = HelpConfiguration {
        show_subcommands: true,
        show_context_menu_commands: true,
        ephemeral: true,
        extra_text_at_bottom,

        ..Default::default()
    };
    poise::builtins::help(ctx, command.as_deref(), config).await?;
    Ok(())
}
