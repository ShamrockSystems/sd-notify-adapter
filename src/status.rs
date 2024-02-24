use std::sync::Arc;

use tokio::sync::{
    mpsc::{Receiver, Sender},
    RwLock,
};
use tokio_util::sync::CancellationToken;
use tracing::{info, instrument};

use crate::{config::Configuration, error::Error};

pub struct Status {
    pub healthz: bool,
    pub livez: bool,
    pub readyz: bool,
}

impl Status {
    pub fn from_config(config: &Configuration) -> Self {
        Status {
            healthz: false,
            livez: config.initial_livez,
            readyz: config.initial_readyz,
        }
    }
}

#[allow(clippy::module_name_repetitions)]
pub type SharedStatus = Arc<RwLock<Status>>;

#[instrument(name = "Status writer", skip_all)]
pub async fn status_writer(
    token: CancellationToken,
    status: SharedStatus,
    mut status_receiver: Receiver<Change>,
    ready_sender: Sender<()>,
) -> Result<(), Error> {
    info!("Status writer ready");
    ready_sender
        .send(())
        .await
        .map_err(Error::ReadyChannelSend)?;

    loop {
        let change = tokio::select! {
            () = token.cancelled() => break,
            result = status_receiver.recv() => result,
        }
        .ok_or(Error::StatusChannelClosed)?;
        let mut status_lock = status.write().await;
        macro_rules! apply (($f: ident) => {
            match change.$f {
                ChangeOperation::Keep => status_lock.$f,
                ChangeOperation::Set(value) => value}});
        status_lock.healthz = apply!(healthz);
        status_lock.livez = apply!(livez);
        status_lock.readyz = apply!(readyz);
    }

    info!("Shutting down status writer");

    Ok(())
}

pub struct Change {
    pub healthz: ChangeOperation,
    pub livez: ChangeOperation,
    pub readyz: ChangeOperation,
}

pub enum ChangeOperation {
    Keep,
    Set(bool),
}
