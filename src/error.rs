use std::{
    io,
    num::{ParseFloatError, ParseIntError},
    str::Utf8Error,
};

use thiserror::Error;
use tokio::{sync::mpsc::error::SendError, task::JoinError};
use tracing::subscriber::SetGlobalDefaultError;

use crate::{config::ConfigurationChange, event::Event, status::Change, timer::watchdog::Message};

#[derive(Debug, Error)]
pub enum Error {
    #[error("Could not subscribe to tracing: {0}")]
    TraceSubscribe(SetGlobalDefaultError),
    #[error("Could not load configuration: {0}")]
    Config(envconfig::Error),
    #[error("The configuration change could not be sent: {0}")]
    ConfigChannelSend(SendError<ConfigurationChange>),
    #[error("The configuration channel has closed")]
    ConfigChannelClosed,
    #[error("The status change could not be sent: {0}")]
    StatusChannelSend(SendError<Change>),
    #[error("The status channel has closed")]
    StatusChannelClosed,
    #[error("The watchdog message could not be sent: {0}")]
    WatchdogChannelSend(SendError<Message>),
    #[error("The watchdog channel has closed")]
    WatchdogChannelClosed,
    #[error("The event could not be sent: {0}")]
    EventChannelSend(SendError<Event>),
    #[error("The event channel has closed")]
    EventChannelClosed,
    #[error("An event has initiated shutdown: {0:?}")]
    EventShutdown(Event),
    #[error("The shutdown channel has closed")]
    ShutdownChannelClosed,
    #[error("The ready could not be sent: {0}")]
    ReadyChannelSend(SendError<()>),
    #[error("Could not parse unrecognized event: {0}")]
    ParseEvent(String),
    #[error("The provided value of NOTIFYACCESS is not supported: {0}")]
    ParseNotifyAccess(String),
    #[error("Could not parse number of seconds from: {0}")]
    ParseSeconds(ParseFloatError),
    #[error("The UDS server could not delete a pre-existing socket: {0}")]
    UdsDeleteSocket(io::Error),
    #[error("The UDS server could not create a new socket: {0}")]
    UdsCreateSocket(io::Error),
    #[error("The UDS server could not get a socket option: {0}")]
    UdsGetSocketOption(nix::errno::Errno),
    #[error("The UDS server could not receive a datagram")]
    UdsReceiveDatagram(io::Error),
    #[error("The UDS server could not decode the datagram into UTF-8")]
    UdsDecodeDatagram(Utf8Error),
    #[error("The UDS server could not shut down")]
    UdsShutdown(io::Error),
    #[error("The HTTP server could not bind to the address: {0}")]
    HttpBindAddress(io::Error),
    #[error("The HTTP server encountered an error: {0}")]
    Http(io::Error),
    #[error("Could not split notify socket message")]
    MessageSplit(String),
    #[error("Could not parse value of socket message as integer: {0}")]
    MessageParseInt(ParseIntError),
    #[error("Could not parse value of socket message as float: {0}")]
    MessageParseFloat(ParseFloatError),
    #[error("The provided message is not a well-known assignment: {0}")]
    MessageUndefined(String),
    #[error("Could not setup up listener for unix signal: {0}")]
    Signal(io::Error),
    #[error("Could not join the task: {0}")]
    Join(JoinError),
}
