use poise;
use poise::serenity_prelude as serenity;
use reqwest::Client as HttpClient;
use serenity::all::ActivityData;
use serenity::async_trait;
use serenity::model::event::ResumedEvent;
use serenity::model::gateway::Ready;
use serenity::model::prelude::OnlineStatus;
use serenity::model::prelude::*;
use serenity::prelude::*;
use songbird::SerenityInit;
use songbird::input::YoutubeDl;
use std::env;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, instrument};
struct HttpKey;

impl TypeMapKey for HttpKey {
    type Value = HttpClient;
}

mod commands;
use commands::help::help;
use commands::music::funts::*;
use commands::music::play;

// Types used by all command functions
type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;
struct Data {
    //cur_song:Arc<Mutex<Option<AuxMetadata>>>,
    queue: Arc<Mutex<Vec<YoutubeDl<'static>>>>,
    now_playing_msg: Arc<Mutex<Option<serenity::Message>>>,
}

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: serenity::Context, ready: Ready) {
        info!(
            "Connected as --> {} [id: {}]",
            ready.user.name, ready.user.id
        );
        let status =
            env::var("DISCORD_STATUS").expect("Set your DISCORD_STATUS environment variable!");
        ctx.set_presence(Some(ActivityData::playing(&status)), OnlineStatus::Online);
    }

    #[instrument(skip(self, _ctx))]
    async fn resume(&self, _ctx: serenity::Context, resume: ResumedEvent) {
        debug!("Resumed; trace: {:?}", resume);
    }
}

fn create_invite_link(client_id: &str, permissions: usize) -> String {
    let mut url =
        url::Url::parse("https://discord.com/oauth2/authorize").expect("Failed to parse URL");
    url.query_pairs_mut()
        .append_pair("client_id", client_id)
        .append_pair("permissions", &permissions.to_string())
        .append_pair("scope", "bot applications.commands");
    return url.to_string();
}

async fn on_error<'a>(error: poise::FrameworkError<'_, Data, Error>) {
    // This is our custom error handler
    // They are many errors that can occur, so we only handle the ones we want to customize
    // and forward the rest to the default handler
    match error {
        poise::FrameworkError::Setup { error, .. } => panic!("Failed to start bot: {:?}", error),
        poise::FrameworkError::Command { error, ctx, .. } => {
            println!("Error in command `{}`: {:?}", ctx.command().name, error,);
        }
        error => {
            if let Err(e) = poise::builtins::on_error(error).await {
                println!("Error while handling error: {}", e)
            }
        }
    }
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().expect("Failed to load .env file.");

    let token = env::var("DISCORD_TOKEN").expect("Set your DISCORD_TOKEN environment variable!");
    let prefix = env::var("PREFIX").expect("Set your PREFIX environment variable!");
    let client_id =
        env::var("DISCORD_CLIENT_ID").expect("Set your CLIENT_ID environment variable!");
    let invite_link = create_invite_link(&client_id, 36700160);
    println!("Invite link: {}", invite_link);
    // Initialise error tracing
    tracing_subscriber::fmt::init();

    let options = poise::FrameworkOptions {
        commands: vec![
            help(),
            play(),
            next(),
            pause(),
            resume(),
            shuffle(),
            disconnect(),
            loop_toggle(),
            //nowplaying(),
            playlist(),
        ],
        prefix_options: poise::PrefixFrameworkOptions {
            prefix: Some(prefix),
            edit_tracker: Some(Arc::new(poise::EditTracker::for_timespan(
                Duration::from_secs(3600),
            ))),
            additional_prefixes: vec![
                poise::Prefix::Literal("^"),
                // poise::Prefix::Literal("hey bot"),
            ],
            ..Default::default()
        },

        // The global error handler for all error cases that may occur
        on_error: |error| Box::pin(on_error(error)),
        // This code is run before every command
        pre_command: |ctx| {
            Box::pin(async move {
                println!("Executing command {}...", ctx.command().qualified_name);
            })
        },
        // This code is run after a command if it was successful (returned Ok)
        post_command: |ctx| {
            Box::pin(async move {
                println!("Executed command {}!", ctx.command().qualified_name);
            })
        },
        // Every command invocation must pass this check to continue execution
        command_check: Some(|ctx| {
            Box::pin(async move {
                if ctx.author().id == 123456789 {
                    return Ok(false);
                }
                Ok(true)
            })
        }),

        // Enforce command checks even for owners (enforced by default)
        // Set to true to bypass checks, which is useful for testing
        skip_checks_for_owners: false,
        event_handler: |_ctx, event, _framework, _data| {
            Box::pin(async move {
                println!(
                    "Got an event in event handler: {:?}",
                    event.snake_case_name()
                );
                Ok(())
            })
        },
        ..Default::default()
    };

    let framework = poise::Framework::builder()
        .setup(move |ctx, _ready, framework| {
            Box::pin(async move {
                println!("Logged in as {}", _ready.user.name);
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(Data {
                    queue: Arc::new(Mutex::new(vec![])),
                    // cur_song:Arc::new(Mutex::new(None)),
                    now_playing_msg: Arc::new(Mutex::new(None)),
                })
            })
        })
        .options(options)
        .build();

    let intents = GatewayIntents::non_privileged()
        | GatewayIntents::MESSAGE_CONTENT
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::GUILD_VOICE_STATES;

    let mut client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .register_songbird()
        .event_handler(Handler)
        .type_map_insert::<HttpKey>(HttpClient::new())
        .await
        .expect("Err creating client");

    client.start().await.unwrap();
}
