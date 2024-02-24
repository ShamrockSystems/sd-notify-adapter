#![warn(clippy::pedantic)]
#![warn(clippy::unwrap_used)]
#![warn(clippy::cargo)]

use std::{io, panic, process::exit, sync::Arc};

use const_format::concatcp;
use envconfig::Envconfig;
use tokio::{
    runtime::{self, UnhandledPanic},
    signal::{self, unix::SignalKind},
    sync::{mpsc, RwLock},
    task::JoinSet,
};
use tokio_util::sync::CancellationToken;
use tracing::{error, info};
use valuable::Valuable;

use crate::{
    config::{Configuration, SharedConfiguration},
    error::Error,
    server::{http, uds},
    status::{Change, SharedStatus, Status},
    timer::{startup, watchdog},
};

mod config;
mod error;
mod event;
mod message;
mod status;
mod server {
    pub mod http;
    pub mod uds;
}
mod timer {
    pub mod startup;
    pub mod watchdog;
}

fn main() {
    if let Err(error) = adapter() {
        error!("{}", error);
        exit(1);
    }
}

#[allow(clippy::too_many_lines)]
fn adapter() -> Result<(), Error> {
    let body = async {
        let config = Configuration::init_from_env().map_err(Error::Config)?;
        let status = Status::from_config(&config);

        if config.log {
            let subscriber = tracing_subscriber::FmtSubscriber::builder()
                .json()
                .flatten_event(true)
                .with_writer(io::stderr)
                .finish();
            tracing::subscriber::set_global_default(subscriber).map_err(Error::TraceSubscribe)?;
        }

        tracing::info!(config = config.as_value(), "Initial configuration");

        let (ready_sender, mut ready_receiver) = mpsc::channel(config.channel_size);
        let (shutdown_sender, mut shutdown_receiver) = mpsc::channel(config.channel_size);
        let (watchdog_sender, watchdog_receiver) = mpsc::channel(config.channel_size);
        let (event_sender, event_receiver) = mpsc::channel(config.channel_size);
        let (config_sender, config_receiver) = mpsc::channel(config.channel_size);
        let (status_sender, status_receiver) = mpsc::channel(config.channel_size);

        let token = CancellationToken::new();

        let config: SharedConfiguration = Arc::new(RwLock::new(config));
        let status: SharedStatus = Arc::new(RwLock::new(status));

        let mut handles = JoinSet::new();
        macro_rules! spawn_task {
            ($task: expr, $name: expr, $shutdown: expr) => {
                handles.spawn(async move {
                    let result = $task.await;
                    if let Err(error) = result {
                        $shutdown
                            .send(error)
                            .await
                            .expect(concatcp!("Could not send shutdown error for ", $name));
                    }
                });
            };
        }

        let token_clone = token.clone();
        let config_clone = config.clone();
        let ready_sender_clone = ready_sender.clone();
        let config_sender_clone = config_sender.clone();
        let event_sender_clone = event_sender.clone();
        let shutdown_sender_clone = shutdown_sender.clone();
        spawn_task!(
            uds::server(
                token_clone,
                config_clone,
                ready_sender_clone,
                config_sender_clone,
                event_sender_clone,
            ),
            "UDS server",
            shutdown_sender_clone
        );

        let token_clone = token.clone();
        let config_clone = config.clone();
        let status_clone = status.clone();
        let ready_sender_clone = ready_sender.clone();
        let shutdown_sender_clone = shutdown_sender.clone();
        spawn_task!(
            http::server(token_clone, config_clone, status_clone, ready_sender_clone),
            "HTTP server",
            shutdown_sender_clone
        );

        let token_clone = token.clone();
        let config_clone = config.clone();
        let ready_sender_clone = ready_sender.clone();
        let watchdog_sender_clone = watchdog_sender.clone();
        let status_sender_clone = status_sender.clone();
        let shutdown_sender_clone = shutdown_sender.clone();
        spawn_task!(
            event::event_listener(
                token_clone,
                config_clone,
                event_receiver,
                ready_sender_clone,
                watchdog_sender_clone,
                status_sender_clone,
            ),
            "event listener",
            shutdown_sender_clone
        );

        let token_clone = token.clone();
        let config_clone = config.clone();
        let status_clone = status.clone();
        let event_sender_clone = event_sender.clone();
        let ready_sender_clone = ready_sender.clone();
        let shutdown_sender_clone = shutdown_sender.clone();
        spawn_task!(
            startup::timer(
                token_clone,
                config_clone,
                status_clone,
                ready_sender_clone,
                event_sender_clone
            ),
            "startup timer",
            shutdown_sender_clone
        );

        let token_clone = token.clone();
        let config_clone = config.clone();
        let ready_sender_clone = ready_sender.clone();
        let event_sender_clone = event_sender.clone();
        let shutdown_sender_clone = shutdown_sender.clone();
        spawn_task!(
            watchdog::timer(
                token_clone,
                config_clone,
                watchdog_receiver,
                ready_sender_clone,
                event_sender_clone
            ),
            "watchdog timer",
            shutdown_sender_clone
        );

        let token_clone = token.clone();
        let status_clone = status.clone();
        let ready_sender_clone = ready_sender.clone();
        let shutdown_sender_clone = shutdown_sender.clone();
        spawn_task!(
            status::status_writer(
                token_clone,
                status_clone,
                status_receiver,
                ready_sender_clone
            ),
            "status writer",
            shutdown_sender_clone
        );

        let token_clone = token.clone();
        let config_clone = config.clone();
        let ready_sender_clone = ready_sender.clone();
        let shutdown_sender_clone = shutdown_sender.clone();
        spawn_task!(
            config::config_writer(
                token_clone,
                config_clone,
                config_receiver,
                ready_sender_clone
            ),
            "config writer",
            shutdown_sender_clone
        );

        let num_handles = handles.len();
        let status_sender_clone = status_sender.clone();
        tokio::spawn(async move {
            for _ in 0..num_handles {
                ready_receiver.recv().await;
            }
            info!("Adapter ready");
            status_sender_clone
                .send(Change {
                    healthz: status::ChangeOperation::Set(true),
                    livez: status::ChangeOperation::Keep,
                    readyz: status::ChangeOperation::Keep,
                })
                .await
                .map_err(Error::StatusChannelSend)
                .expect("Could not send ready status change");
        });

        macro_rules! unix_signal (($e: expr) => {signal::unix::signal($e).map_err(Error::Signal)?});

        let mut alarm = unix_signal!(SignalKind::alarm());
        let mut hangup = unix_signal!(SignalKind::hangup());
        let mut interrupt = unix_signal!(SignalKind::interrupt());
        let mut pipe = unix_signal!(SignalKind::pipe());
        let mut quit = unix_signal!(SignalKind::quit());
        let mut terminate = unix_signal!(SignalKind::terminate());
        let mut user_defined1 = unix_signal!(SignalKind::user_defined1());
        let mut user_defined2 = unix_signal!(SignalKind::user_defined2());

        let result = tokio::select! {
            _ = alarm.recv() => Ok(()),
            _ = hangup.recv() => Ok(()),
            _ = interrupt.recv() => Ok(()),
            _ = pipe.recv() => Ok(()),
            _ = quit.recv() => Ok(()),
            _ = terminate.recv() => Ok(()),
            _ = user_defined1.recv() => Ok(()),
            _ = user_defined2.recv() => Ok(()),
            error = shutdown_receiver.recv() => {
                match error {
                    Some(error) => Err(error),
                    None => Err(Error::ShutdownChannelClosed),
                }
            },
        };

        token.cancel();

        while let Some(result) = handles.join_next().await {
            result.map_err(Error::Join)?;
        }

        result
    };

    runtime::Builder::new_current_thread()
        .enable_all()
        .unhandled_panic(UnhandledPanic::ShutdownRuntime)
        .build()
        .expect("Failed building the Runtime")
        .block_on(body)
}
