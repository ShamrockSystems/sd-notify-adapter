use std::{f64::INFINITY, time::Duration};

use tokio::{sync::mpsc::Sender, time::sleep};
use tokio_util::sync::CancellationToken;
use tracing::{info, instrument};

use crate::{
    config::{Seconds, SharedConfiguration},
    error::Error,
    event::Event,
    status::SharedStatus,
};

#[instrument(name = "Startup timer", skip_all)]
pub async fn timer(
    token: CancellationToken,
    config: SharedConfiguration,
    status: SharedStatus,
    ready_sender: Sender<()>,
    event_sender: Sender<Event>,
) -> Result<(), Error> {
    let mut timeout = config.read().await.unit_timeout_start_sec;

    info!("Startup timer ready");
    ready_sender
        .send(())
        .await
        .map_err(Error::ReadyChannelSend)?;

    if timeout == Seconds(INFINITY) {
        return Ok(());
    }

    let mut sleep_duration = timeout.into();
    loop {
        tokio::select! {
            () = token.cancelled() => return Ok(()),
            () = sleep(sleep_duration) => {}
        }

        let new_timeout = config.read().await.unit_timeout_start_sec;
        if timeout == new_timeout {
            break;
        }
        sleep_duration = Duration::from(new_timeout) - timeout.into();
        timeout = new_timeout;
    }

    let ready = status.read().await.readyz;
    if !ready {
        event_sender
            .send(Event::StartTimeout)
            .await
            .map_err(Error::EventChannelSend)?;
    }

    info!("Shutting down startup timer");

    Ok(())
}
