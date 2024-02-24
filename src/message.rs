use std::str::FromStr;

use valuable::Valuable;

use crate::{config::Seconds, error::Error};

#[derive(Clone)]
pub enum Message {
    Ready,
    Reloading,
    Stopping,
    MonotonicMicrosecond(Seconds),
    Status(String),
    NotifyAccess(NotifyAccess),
    ErrorNumber(i32),
    BusError(String),
    ExitStatus(i32),
    MainPID(i32),
    Watchdog,
    WatchdogTrigger,
    WatchdogMicrosecond(Seconds),
    ExtendTimeoutMicrosecond(Seconds),
    FDStore,
    FDStoreRemove,
    FDName(String),
    FDPoll,
    Barrier,
}

const SECOND_TO_MICROSECOND: f64 = 1_000_000.0;

impl FromStr for Message {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (key, value) = s.split_once('=').ok_or(Error::MessageSplit(s.into()))?;
        macro_rules! parse_message (($e: path) => {Ok($e(value.parse::<i32>().map_err(Error::MessageParseInt)?))});
        macro_rules! micro_second (($e: path) => {Ok($e(Seconds(value.parse::<f64>().map_err(Error::MessageParseFloat)?/SECOND_TO_MICROSECOND)))});

        match (key, value) {
            ("READY", "1") => Ok(Self::Ready),
            ("RELOADING", "1") => Ok(Self::Reloading),
            ("STOPPING", "1") => Ok(Self::Stopping),
            ("MONOTONIC_USEC", _) => micro_second!(Self::MonotonicMicrosecond),
            ("STATUS", _) => Ok(Self::Status(value.into())),
            ("NOTIFYACCESS", _) => Ok(Self::NotifyAccess(value.parse()?)),
            ("ERRNO", _) => parse_message!(Self::ErrorNumber),
            ("BUSERROR", _) => Ok(Self::BusError(value.into())),
            ("EXIT_STATUS", _) => parse_message!(Self::ExitStatus),
            ("MAINPID", _) => parse_message!(Self::MainPID),
            ("WATCHDOG", "1") => Ok(Self::Watchdog),
            ("WATCHDOG", "trigger") => Ok(Self::WatchdogTrigger),
            ("WATCHDOG_USEC", _) => micro_second!(Self::WatchdogMicrosecond),
            ("EXTEND_TIMEOUT_USEC", _) => micro_second!(Self::ExtendTimeoutMicrosecond),
            ("FDSTORE", "1") => Ok(Self::FDStore),
            ("FDSTOREREMOVE", "1") => Ok(Self::FDStoreRemove),
            ("FDNAME", _) => Ok(Self::FDName(value.into())),
            ("FDPOLL", "0") => Ok(Self::FDPoll),
            ("BARRIER", "1") => Ok(Self::Barrier),
            _ => Err(Error::MessageUndefined(s.into())),
        }
    }
}

impl From<Message> for String {
    fn from(value: Message) -> Self {
        macro_rules! as_microsecond(($k: expr, $d: expr) => {format!("{}={}", $k, $d.0 * SECOND_TO_MICROSECOND)});
        match value {
            Message::Ready => String::from("READY=1"),
            Message::Reloading => String::from("RELOADING=1"),
            Message::Stopping => String::from("STOPPING=1"),
            Message::MonotonicMicrosecond(timestamp) => {
                as_microsecond!("MONOTONIC_USEC", timestamp)
            }
            Message::Status(status) => format!("STATUS={status}"),
            Message::NotifyAccess(a) => match a {
                NotifyAccess::None => String::from("NOTIFYACCESS=none"),
                NotifyAccess::Main => String::from("NOTIFYACCESS=main"),
                NotifyAccess::Exec => String::from("NOTIFYACCESS=exec"),
                NotifyAccess::All => String::from("NOTIFYACCESS=all"),
            },
            Message::ErrorNumber(number) => format!("ERRNO={number}"),
            Message::BusError(error) => format!("BUSERROR={error}"),
            Message::ExitStatus(status) => format!("EXIT_STATUS={status}"),
            Message::MainPID(pid) => format!("MAINPID={pid}"),
            Message::Watchdog => String::from("WATCHDOG=1"),
            Message::WatchdogTrigger => String::from("WATCHDOG=trigger"),
            Message::WatchdogMicrosecond(timeout) => as_microsecond!("WATCHDOG_USEC", timeout),
            Message::ExtendTimeoutMicrosecond(timeout) => {
                as_microsecond!("EXTEND_TIMEOUT_USEC", timeout)
            }
            Message::FDStore => String::from("FDSTORE=1"),
            Message::FDStoreRemove => String::from("FDSTOREREMOVE=1"),
            Message::FDName(name) => format!("FDNAME={name}"),
            Message::FDPoll => String::from("FDPOLL=0"),
            Message::Barrier => String::from("BARRIER=1"),
        }
    }
}

#[derive(Clone, Valuable)]
pub enum NotifyAccess {
    None,
    Main,
    Exec,
    All,
}

impl FromStr for NotifyAccess {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "none" => Ok(Self::None),
            "main" => Ok(Self::Main),
            "exec" => Ok(Self::Exec),
            "all" => Ok(Self::All),
            _ => Err(Error::ParseNotifyAccess(s.into())),
        }
    }
}
