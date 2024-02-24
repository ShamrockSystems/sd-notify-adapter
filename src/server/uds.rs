use std::{net::Shutdown, os::fd::AsFd, path::PathBuf};

use nix::sys::{self, socket::sockopt::RcvBuf};
use tokio::{net::UnixDatagram, sync::mpsc::Sender};
use tokio_util::sync::CancellationToken;
use tracing::{info, instrument};

use crate::{
    config::{ConfigurationChange, SharedConfiguration},
    error::Error,
    event::Event,
    message::Message,
};

#[instrument(name = "UDS server", skip_all)]
pub async fn server(
    token: CancellationToken,
    config: SharedConfiguration,
    ready_sender: Sender<()>,
    config_sender: Sender<ConfigurationChange>,
    event_sender: Sender<Event>,
) -> Result<(), Error> {
    let notify_socket = PathBuf::from(config.read().await.notify_socket.clone());

    if notify_socket.exists() {
        std::fs::remove_file(&notify_socket).map_err(Error::UdsDeleteSocket)?;
    }
    let socket = UnixDatagram::bind(&notify_socket).map_err(Error::UdsCreateSocket)?;
    let fd = socket.as_fd();

    let buffer_size = sys::socket::getsockopt(&fd, RcvBuf).map_err(Error::UdsGetSocketOption)?;
    let mut buffer = vec![0u8; buffer_size];

    info!("UDS server ready");
    ready_sender
        .send(())
        .await
        .map_err(Error::ReadyChannelSend)?;

    loop {
        let (length, _) = tokio::select! {
            () = token.cancelled() => break,
            result = socket.recv_from(&mut buffer) => result,
        }
        .map_err(Error::UdsReceiveDatagram)?;
        let datagram = std::str::from_utf8(&buffer[..length]).map_err(Error::UdsDecodeDatagram)?;
        process_datagram(
            config.clone(),
            config_sender.clone(),
            event_sender.clone(),
            datagram,
        )
        .await?;
    }

    info!("Shutting down UDS server");

    socket
        .shutdown(Shutdown::Both)
        .map_err(Error::UdsShutdown)?;

    Ok(())
}

async fn process_datagram(
    config: SharedConfiguration,
    config_sender: Sender<ConfigurationChange>,
    event_sender: Sender<Event>,
    datagram: &str,
) -> Result<(), Error> {
    let messages = datagram
        .lines()
        .map(str::parse)
        .collect::<Result<Vec<Message>, _>>()?;
    for message in messages {
        if config.read().await.echo {
            println!("{}", String::from(message.clone()));
        }

        macro_rules! send_event (($e: expr) => {event_sender.send($e).await.map_err(Error::EventChannelSend)};);
        macro_rules! send_config_change (($e: expr) => {config_sender.send($e).await.map_err(Error::ConfigChannelSend)};);

        match message {
            Message::Ready => send_event!(Event::Ready)?,
            Message::Reloading => send_event!(Event::Reloading)?,
            Message::Stopping => send_event!(Event::Stopping)?,
            Message::ErrorNumber(_) => send_event!(Event::ErrorNumber)?,
            Message::BusError(_) => send_event!(Event::BusError)?,
            Message::Watchdog => send_event!(Event::Watchdog)?,
            Message::WatchdogTrigger => send_event!(Event::WatchdogTrigger)?,
            Message::WatchdogMicrosecond(timeout) => {
                send_config_change!(ConfigurationChange::WatchdogTimeout(timeout))?;
            }
            Message::ExtendTimeoutMicrosecond(extension) => {
                let current_timeout = config.read().await.unit_timeout_start_sec;
                let timeout = current_timeout + extension;
                send_config_change!(ConfigurationChange::StartupTimeout(timeout))?;
            }
            _ => {}
        }
    }

    Ok(())
}
