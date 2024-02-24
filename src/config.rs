use std::{path::PathBuf, str::FromStr, sync::Arc, time::Duration};

use derive_more::{Add, FromStr};
use envconfig::Envconfig;
use tokio::sync::{
    mpsc::{Receiver, Sender},
    RwLock,
};
use tokio_util::sync::CancellationToken;
use tracing::{info, instrument, warn};
use valuable::Valuable;

use crate::{error::Error, event::EventList};

#[allow(clippy::struct_excessive_bools)]
#[derive(Envconfig, Valuable)]
pub struct Configuration {
    // General configuration
    #[envconfig(from = "NOTIFY_SOCKET", default = "/var/run/adapter/adapter.sock")]
    pub notify_socket: ConfigString,
    #[envconfig(from = "ADAPTER_PORT", default = "8089")]
    pub port: u16,
    #[envconfig(from = "ADAPTER_ECHO", default = "true")]
    pub echo: bool,
    #[envconfig(from = "ADAPTER_LOG", default = "true")]
    pub log: bool,
    #[envconfig(from = "ADAPTER_CHANNEL_SIZE", default = "32")]
    pub channel_size: usize,
    #[envconfig(from = "ADAPTER_INITIAL_LIVEZ", default = "false")]
    pub initial_livez: bool,
    #[envconfig(from = "ADAPTER_INITIAL_READYZ", default = "false")]
    pub initial_readyz: bool,
    #[envconfig(from = "ADAPTER_ALLOW_MESSAGE_WATCHDOG_USEC", default = "true")]
    pub allow_message_watchdog_usec: bool,
    #[envconfig(from = "ADAPTER_ALLOW_MESSAGE_EXTEND_TIMEOUT_USEC", default = "true")]
    pub allow_message_extend_timeout_usec: bool,
    // Status change configuration
    #[envconfig(from = "ADAPTER_STATUS_LIVEZ_TRUE", default = "ready,watchdog")]
    pub status_livez_true: EventList,
    #[envconfig(
        from = "ADAPTER_STATUS_LIVEZ_FALSE",
        default = "errno,buserror,watchdog_trigger,watchdog_timeout,start_timeout"
    )]
    pub status_livez_false: EventList,
    #[envconfig(from = "ADAPTER_STATUS_READYZ_TRUE", default = "ready,watchdog")]
    pub status_readyz_true: EventList,
    #[envconfig(
        from = "ADAPTER_STATUS_READYZ_FALSE",
        default = "reloading,stopping,errno,buserror,watchdog_trigger,watchdog_timeout,start_timeout"
    )]
    pub status_readyz_false: EventList,
    #[envconfig(from = "ADAPTER_STATUS_SHUTDOWN", default = "")]
    pub status_shutdown: EventList,
    // `systemd` unit configuration
    #[envconfig(from = "ADAPTER_UNIT_TIMEOUT_START_SEC", default = "90")]
    pub unit_timeout_start_sec: Seconds,
    #[envconfig(from = "ADAPTER_UNIT_WATCHDOG_SEC", default = "0")]
    pub unit_watchdog_sec: Seconds,
}

pub type SharedConfiguration = Arc<RwLock<Configuration>>;

#[instrument(name = "Config writer", skip_all)]
pub async fn config_writer(
    token: CancellationToken,
    config: SharedConfiguration,
    mut config_receiver: Receiver<ConfigurationChange>,
    ready_sender: Sender<()>,
) -> Result<(), Error> {
    info!("Config writer ready");
    ready_sender
        .send(())
        .await
        .map_err(Error::ReadyChannelSend)?;

    loop {
        let change = tokio::select! {
            () = token.cancelled() => break,
            result = config_receiver.recv() => result,
        }
        .ok_or(Error::ConfigChannelClosed)?;
        let mut config_lock = config.write().await;
        match change {
            ConfigurationChange::WatchdogTimeout(timeout) => {
                if config_lock.allow_message_watchdog_usec {
                    config_lock.unit_watchdog_sec = timeout;
                } else {
                    warn!("Attempted to override watchdog timeout, but ADAPTER_ALLOW_MESSAGE_WATCHDOG_USEC is false");
                }
            }
            ConfigurationChange::StartupTimeout(timeout) => {
                if config_lock.allow_message_extend_timeout_usec {
                    config_lock.unit_timeout_start_sec = timeout;
                } else {
                    warn!("Attempted to override startup timeout, but ADAPTER_ALLOW_MESSAGE_EXTEND_TIMEOUT_USEC is false");
                }
            }
        }
    }

    info!("Shutting down config writer");

    Ok(())
}

pub enum ConfigurationChange {
    WatchdogTimeout(Seconds),
    StartupTimeout(Seconds),
}

#[derive(FromStr, Clone, Copy, Add, Valuable, PartialEq)]
pub struct Seconds(pub f64);

impl From<Seconds> for Duration {
    fn from(value: Seconds) -> Self {
        Duration::from_secs_f64(value.0)
    }
}

#[allow(clippy::module_name_repetitions)]
#[derive(Clone)]
pub struct ConfigString(pub Arc<str>);
impl Valuable for ConfigString {
    fn as_value(&self) -> valuable::Value<'_> {
        valuable::Value::String(&self.0)
    }

    fn visit(&self, visit: &mut dyn valuable::Visit) {
        visit.visit_value(self.as_value());
    }
}

impl FromStr for ConfigString {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(ConfigString(String::from(s).into()))
    }
}

impl From<ConfigString> for PathBuf {
    fn from(value: ConfigString) -> Self {
        PathBuf::from(value.0.as_ref())
    }
}
