mod convert;
mod errors;
mod message;
mod types;

use message::*;
use reqwest::{StatusCode, Url};
use std::{convert::Infallible, env, net::SocketAddr};
use teloxide::{
    dispatching::{
        stop_token::AsyncStopToken,
        update_listeners::{self, StatefulListener},
    },
    dispatching2::UpdateFilterExt,
    dptree,
    prelude2::*,
};
use tokio_stream::wrappers::UnboundedReceiverStream;
use warp::Filter;

#[tokio::main]
async fn main() {
    run().await;
}

async fn webhook(bot: AutoSend<Bot>) -> impl update_listeners::UpdateListener<Infallible> {
    // Heroku auto defines a port value
    let teloxide_token = env::var("TELOXIDE_TOKEN").expect("TELOXIDE_TOKEN is set");
    let port: u16 = env::var("PORT")
        .expect("PORT is set")
        .parse()
        .expect("PORT is u16");
    let host = env::var("HOST").expect("HOST is set");
    let path = format!("bot{}", teloxide_token);
    let url = Url::parse(&format!("https://{}/{}", host, path)).unwrap();

    bot.set_webhook(url).await.expect("setup the webhook");
    log::info!("Bot webhook set.");

    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

    async fn handle_rejection(error: warp::Rejection) -> Result<impl warp::Reply, Infallible> {
        log::error!("Cannot process the request due to: {:?}", error);
        Ok(StatusCode::INTERNAL_SERVER_ERROR)
    }

    let server = warp::post()
        .and(warp::path(path))
        .and(warp::body::json())
        .map(move |update: Update| {
            tx.send(Ok(update))
                .expect("send an incoming update from the webhook");

            StatusCode::OK
        })
        .recover(handle_rejection);

    let (stop_token, stop_flag) = AsyncStopToken::new_pair();

    let addr = format!("0.0.0.0:{}", port).parse::<SocketAddr>().unwrap();
    let server = warp::serve(server);
    let (_addr, fut) = server.bind_with_graceful_shutdown(addr, stop_flag);

    // You might want to use serve.key_path/serve.cert_path methods here to
    // setup a self-signed TLS certificate.

    tokio::spawn(fut);
    let stream = UnboundedReceiverStream::new(rx);

    fn streamf<S, T>(state: &mut (S, T)) -> &mut S {
        &mut state.0
    }

    StatefulListener::new(
        (stream, stop_token),
        streamf,
        |state: &mut (_, AsyncStopToken)| state.1.clone(),
    )
}

async fn run() {
    teloxide::enable_logging!();
    log::info!("Starting bot...");

    let bot = Bot::from_env().auto_send();

    let handler = dptree::entry()
        .branch(
            Update::filter_message()
                .filter_command::<types::Command>()
                .endpoint(command_handler),
        )
        .branch(Update::filter_message().endpoint(message_handler))
        .branch(Update::filter_callback_query().endpoint(callback_handler));

    let mut dispatcher = Dispatcher::builder(bot.clone(), handler).build();
    dispatcher.setup_ctrlc_handler();
    if env::var("TELOXIDE_USE_WEBHOOK").is_ok() {
        dispatcher
            .dispatch_with_listener(webhook(bot).await, LoggingErrorHandler::new())
            .await;
    } else {
        dispatcher.dispatch().await;
    }

    log::info!("Closing bot...");
}
