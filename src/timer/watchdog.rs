use std::{
    future::{self, Future},
    pin::Pin,
    task::{Context, Poll},
    time::{Duration, Instant},
};

use tokio::{
    sync::mpsc::{Receiver, Sender},
    time::sleep,
};
use tokio_util::sync::CancellationToken;
use tracing::{info, instrument};

use crate::{config::SharedConfiguration, error::Error, event::Event};

#[instrument(name = "Watchdog timer", skip_all)]
pub async fn timer(
    token: CancellationToken,
    config: SharedConfiguration,
    mut watchdog_receiver: Receiver<Message>,
    ready_sender: Sender<()>,
    event_sender: Sender<Event>,
) -> Result<(), Error> {
    let mut duration: Duration = config.read().await.unit_watchdog_sec.into();
    let mut last_timestamp = Instant::now();

    info!("Watchdog timer ready");
    ready_sender
        .send(())
        .await
        .map_err(Error::ReadyChannelSend)?;

    loop {
        let timeout = async {
            if duration.is_zero() {
                future::pending::<()>().await;
            } else {
                sleep(duration).await;
            }
        };
        let message = tokio::select! {
            () = token.cancelled() => break,
            message = watchdog_receiver.recv() => message.ok_or(Error::WatchdogChannelClosed)?,
            () = timeout => Message::Wake,
        };
        match message {
            Message::Wake => {
                let current_timestamp = Instant::now();
                let change = current_timestamp - last_timestamp;
                if change > duration {
                    event_sender
                        .send(Event::WatchdogTimeout)
                        .await
                        .map_err(Error::EventChannelSend)?;
                }
            }
            Message::KeepAlive | Message::Trigger => {
                last_timestamp = Instant::now();
            } // Status change handled in event listener
            Message::NewTimeout => {
                duration = config.read().await.unit_watchdog_sec.into();
            }
        }
    }

    info!("Shutting down watchdog timer");

    Ok(())
}

pub enum Message {
    Wake,
    KeepAlive,
    Trigger,
    NewTimeout,
}

struct NeverYield;

impl Future for NeverYield {
    type Output = ();

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Always return Pending, never ready to make progress
        Poll::Pending
    }
}
