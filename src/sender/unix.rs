// Copyright 2024 FastLabs Developers
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::fmt;
use std::io;
use std::io::BufWriter;
use std::os::unix::net::UnixDatagram;
use std::os::unix::net::UnixStream;
use std::path::Path;

use crate::impl_syslog_sender;
use crate::sender::SyslogSender;
use crate::SyslogContext;

/// Create a Unix datagram sender that sends messages to the given path.
pub fn unix_datagram(path: impl AsRef<Path>) -> io::Result<SyslogSender> {
    UnixDatagramSender::connect(path).map(SyslogSender::UnixDatagram)
}

/// Create a Unix stream sender that sends messages to the given path.
pub fn unix_stream(path: impl AsRef<Path>) -> io::Result<SyslogSender> {
    UnixStreamSender::connect(path).map(SyslogSender::UnixStream)
}

/// Create a Unix sender that sends messages to the given path.
///
/// Automatically chooses between Unix datagram and Unix stream based on the path.
pub fn unix(path: impl AsRef<Path>) -> io::Result<SyslogSender> {
    const EPROTOTYPE: i32 = nix::errno::Errno::EPROTOTYPE as i32;
    let path = path.as_ref();
    match unix_datagram(path) {
        Ok(sender) => Ok(sender),
        Err(err) => match err.raw_os_error() {
            Some(EPROTOTYPE) => unix_stream(path),
            _ => Err(err),
        },
    }
}

pub fn unix_well_known() -> io::Result<SyslogSender> {
    cfg_if::cfg_if! {
        if #[cfg(target_os = "linux")] {
            unix("/dev/log")
        } else if #[cfg(target_os = "macos")] {
            // NOTE: This may not work on Monterey (12.x) and above,
            //  see also https://github.com/python/cpython/issues/91070.
            unix("/var/run/syslog")
        } else if #[cfg(target_os = "freebsd")] {
            unix("/var/run/log")
        } else {
            unix("/dev/log")
        }
    }
}

/// A syslog sender that sends messages to a Unix datagram socket.
///
/// Typically, this sender is used to send messages to the local syslog daemon. Typical paths are
/// `/dev/log`, `/var/run/syslog`, or `/var/run/log`.
#[derive(Debug)]
pub struct UnixDatagramSender {
    socket: UnixDatagramWriteAdapter,
    context: SyslogContext,
}

impl UnixDatagramSender {
    /// Connect to a Unix datagram socket at the given path.
    pub fn connect(path: impl AsRef<Path>) -> io::Result<Self> {
        let socket = UnixDatagram::unbound()?;
        socket.connect(path)?;
        Ok(Self {
            socket: UnixDatagramWriteAdapter { socket },
            context: SyslogContext::default(),
        })
    }

    /// Set the context when formatting Syslog message.
    pub fn set_context(&mut self, context: SyslogContext) {
        self.context = context;
    }

    /// Mutate the context when formatting Syslog message.
    pub fn mut_context(&mut self) -> &mut SyslogContext {
        &mut self.context
    }
}

impl_syslog_sender!(UnixDatagramSender, context, socket);

#[derive(Debug)]
struct UnixDatagramWriteAdapter {
    socket: UnixDatagram,
}

impl io::Write for UnixDatagramWriteAdapter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.socket.send(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }

    // HACK - without this method, the 'write!' macro will be fragmented into multiple write calls
    fn write_fmt(&mut self, fmt: fmt::Arguments<'_>) -> io::Result<()> {
        self.write_all(fmt.to_string().as_bytes())
    }
}

/// A syslog sender that sends messages to a Unix stream socket.
///
/// Typically, this sender is used to send messages to the local syslog daemon. Typical paths are
/// `/dev/log`, `/var/run/syslog`, or `/var/run/log`.
#[derive(Debug)]
pub struct UnixStreamSender {
    writer: BufWriter<UnixStream>,
    context: SyslogContext,
}

impl UnixStreamSender {
    /// Connect to a Unix stream socket at the given path.
    pub fn connect(path: impl AsRef<Path>) -> io::Result<Self> {
        let socket = UnixStream::connect(path)?;
        Ok(Self {
            writer: BufWriter::new(socket),
            context: SyslogContext::default(),
        })
    }

    /// Set the context when formatting Syslog message.
    pub fn set_context(&mut self, context: SyslogContext) {
        self.context = context;
    }

    /// Mutate the context when formatting Syslog message.
    pub fn mut_context(&mut self) -> &mut SyslogContext {
        &mut self.context
    }

    /// Flush the writer.
    pub fn flush(&mut self) -> io::Result<()> {
        use std::io::Write;
        self.writer.flush()
    }
}

impl_syslog_sender!(UnixStreamSender, context, writer);
