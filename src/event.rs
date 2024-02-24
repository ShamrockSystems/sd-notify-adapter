use std::{str::FromStr, sync::Arc};

use tokio::sync::mpsc::{Receiver, Sender};
use tokio_util::sync::CancellationToken;
use tracing::{info, instrument};
use valuable::Valuable;

use crate::{
    config::SharedConfiguration,
    error::Error,
    status::{Change, ChangeOperation},
    timer::watchdog::Message,
};

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Valuable)]
pub enum Event {
    Ready,
    Reloading,
    Stopping,
    ErrorNumber,
    BusError,
    Watchdog,
    WatchdogTrigger,
    WatchdogTimeout,
    StartTimeout,
}

impl FromStr for Event {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "ready" => Ok(Self::Ready),
            "reloading" => Ok(Self::Reloading),
            "stopping" => Ok(Self::Stopping),
            "errno" => Ok(Self::ErrorNumber),
            "buserror" => Ok(Self::BusError),
            "watchdog" => Ok(Self::Watchdog),
            "watchdog_trigger" => Ok(Self::WatchdogTrigger),
            "watchdog_timeout" => Ok(Self::WatchdogTimeout),
            "start_timeout" => Ok(Self::StartTimeout),
            _ => Err(Error::ParseEvent(s.into())),
        }
    }
}

#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Valuable)]
pub struct EventList(Arc<[Event]>);

impl FromStr for EventList {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            Ok(EventList(Arc::from([])))
        } else {
            let events: Result<Arc<[_]>, _> = s.split(',').map(str::parse).collect();
            Ok(EventList(events?))
        }
    }
}

impl EventList {
    fn contains(&self, event: &Event) -> bool {
        self.0.contains(event)
    }
}

#[instrument(name = "Event listener", skip_all)]
pub async fn event_listener(
    token: CancellationToken,
    config: SharedConfiguration,
    mut event_receiver: Receiver<Event>,
    ready_sender: Sender<()>,
    watchdog_sender: Sender<Message>,
    status_sender: Sender<Change>,
) -> Result<(), Error> {
    // Event lists should not change during runtime
    let config_lock = config.read().await;
    let status_livez_true = config_lock.status_livez_true.clone();
    let status_livez_false = config_lock.status_livez_false.clone();
    let status_readyz_true = config_lock.status_readyz_true.clone();
    let status_readyz_false = config_lock.status_readyz_false.clone();
    let status_shutdown = config_lock.status_shutdown.clone();

    info!("Event listener ready");
    ready_sender
        .send(())
        .await
        .map_err(Error::ReadyChannelSend)?;

    loop {
        let event = tokio::select! {
            () = token.cancelled() => break,
            result = event_receiver.recv() => result,
        }
        .ok_or(Error::EventChannelClosed)?;

        info!(event = event.as_value(), "Processing event");

        macro_rules! send_watchdog {
            ($message: expr) => {
                watchdog_sender
                    .send($message)
                    .await
                    .map_err(Error::WatchdogChannelSend)
            };
        }
        match event {
            Event::Watchdog => send_watchdog!(Message::KeepAlive)?,
            Event::WatchdogTrigger => send_watchdog!(Message::Trigger)?,
            Event::WatchdogTimeout => send_watchdog!(Message::NewTimeout)?,
            _ => {}
        }

        let healthz_operation = ChangeOperation::Keep;
        let mut livez_operation = ChangeOperation::Keep;
        let mut readyz_operation = ChangeOperation::Keep;

        if status_shutdown.contains(&event) {
            return Err(Error::EventShutdown(event));
        }

        if status_livez_true.contains(&event) {
            livez_operation = ChangeOperation::Set(true);
        }
        if status_livez_false.contains(&event) {
            livez_operation = ChangeOperation::Set(false);
        }
        if status_readyz_true.contains(&event) {
            readyz_operation = ChangeOperation::Set(true);
        }
        if status_readyz_false.contains(&event) {
            readyz_operation = ChangeOperation::Set(false);
        }

        status_sender
            .send(Change {
                healthz: healthz_operation,
                livez: livez_operation,
                readyz: readyz_operation,
            })
            .await
            .map_err(Error::StatusChannelSend)?;
    }

    info!("Shutting down event listener");

    Ok(())
}
