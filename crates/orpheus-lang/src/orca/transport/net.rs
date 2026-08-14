//! UDP and OSC senders for the `;` and `=` operators, plus (v7) the UDP
//! command listener.
//!
//! Both send transports write datagrams from a lazily bound, unconnected
//! UDP socket to a configurable target, defaulting to the reference
//! client's ports (UDP out `49161`, OSC `49162`, both on `127.0.0.1` —
//! `io.js` `this.ip`). The OSC wire format matches `osc.js` `play()`:
//! address `/<path>` and one int32 argument per message glyph, each the
//! glyph's base-36 `valueOf`.
//!
//! [`UdpCommandListener`] is the input direction: the reference's
//! `udp.js` `selectInput` (default port `49160`) pipes every received
//! datagram straight into `commander.trigger`, so inbound UDP speaks the
//! same command language as the `$` operator. The listener owns its own
//! `orca-udp-in` thread (never the audio thread) and hands received
//! strings to the host, which routes them through the shared command
//! interpreter (`orca::parse_command`). Off by default; the host enables
//! it with `:orca listen`.

use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, UdpSocket};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::JoinHandle;
use std::time::Duration;

use rosc::{OscMessage, OscPacket, OscType, encoder};

use super::TransportError;
use crate::orca::engine::value_of;

/// A datagram endpoint: target address plus a lazily bound local socket.
#[derive(Debug)]
struct Endpoint {
    target: SocketAddr,
    socket: Option<UdpSocket>,
}

impl Endpoint {
    const fn new(target: SocketAddr) -> Self {
        Self {
            target,
            socket: None,
        }
    }

    /// Points the endpoint at a new target, rebinding on next send (the
    /// address family may have changed).
    fn set_target(&mut self, target: SocketAddr) {
        self.target = target;
        self.socket = None;
    }

    /// Sends one datagram, binding an ephemeral local socket on first use.
    fn send(&mut self, payload: &[u8]) -> Result<(), TransportError> {
        if self.socket.is_none() {
            let local: SocketAddr = if self.target.is_ipv4() {
                (Ipv4Addr::UNSPECIFIED, 0).into()
            } else {
                (Ipv6Addr::UNSPECIFIED, 0).into()
            };
            self.socket = Some(UdpSocket::bind(local)?);
        }
        let socket = self.socket.as_ref().expect("socket bound above");
        socket.send_to(payload, self.target)?;
        Ok(())
    }
}

/// The `;` transport: sends the raw message string as one UDP datagram
/// (`udp.js` `play()` sends `Buffer.from(data)` verbatim).
#[derive(Debug)]
pub struct UdpTransport {
    endpoint: Endpoint,
}

impl UdpTransport {
    /// A transport aimed at `target`; no socket is bound until the first
    /// send.
    #[must_use]
    pub const fn new(target: SocketAddr) -> Self {
        Self {
            endpoint: Endpoint::new(target),
        }
    }

    /// The current target address.
    #[must_use]
    #[allow(dead_code)]
    pub(crate) const fn target(&self) -> SocketAddr {
        self.endpoint.target
    }

    /// Re-aims the transport.
    pub fn set_target(&mut self, target: SocketAddr) {
        self.endpoint.set_target(target);
    }

    /// Sends `message` verbatim as one datagram. An empty message sends an
    /// empty datagram, matching the reference's missing empty-message guard.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError::Socket`] when binding or sending fails.
    pub fn send(&mut self, message: &str) -> Result<(), TransportError> {
        self.endpoint.send(message.as_bytes())
    }
}

/// The `=` transport: encodes and sends one OSC message per event.
#[derive(Debug)]
pub struct OscTransport {
    endpoint: Endpoint,
}

impl OscTransport {
    /// A transport aimed at `target`; no socket is bound until the first
    /// send.
    #[must_use]
    pub const fn new(target: SocketAddr) -> Self {
        Self {
            endpoint: Endpoint::new(target),
        }
    }

    /// The current target address.
    #[must_use]
    #[allow(dead_code)]
    pub(crate) const fn target(&self) -> SocketAddr {
        self.endpoint.target
    }

    /// Re-aims the transport.
    pub fn set_target(&mut self, target: SocketAddr) {
        self.endpoint.set_target(target);
    }

    /// Sends an OSC message with address `/<path>` and one int32 argument
    /// per arg glyph — the glyph's base-36 value, exactly the reference's
    /// `oscMsg.append(orca.valueOf(msg.charAt(i)))`.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError::OscEncode`] when encoding fails and
    /// [`TransportError::Socket`] when binding or sending fails.
    pub fn send(&mut self, path: char, args: &str) -> Result<(), TransportError> {
        let message = OscMessage {
            addr: format!("/{path}"),
            args: args
                .chars()
                // `value_of` is bounded by 35; the fallback is unreachable.
                .map(|glyph| OscType::Int(i32::try_from(value_of(glyph)).unwrap_or(i32::MAX)))
                .collect(),
        };
        let payload = encoder::encode(&OscPacket::Message(message))
            .map_err(|error| TransportError::OscEncode(error.to_string()))?;
        self.endpoint.send(&payload)
    }
}

/// How often the listener thread checks its shutdown flag while idle.
const LISTENER_POLL: Duration = Duration::from_millis(250);

/// Datagram receive buffer; commands are tiny (the `$` operator caps
/// messages at 36 glyphs), so one MTU-sized buffer is generous.
const LISTENER_BUFFER_BYTES: usize = 2048;

/// A UDP listener feeding received datagrams to the host's command
/// interpreter, mirroring `udp.js` `selectInput` → `commander.trigger`.
///
/// Binding spawns one `orca-udp-in` thread that blocks on the socket (with
/// a periodic shutdown check) and forwards each datagram — lossily decoded
/// as UTF-8, trailing whitespace trimmed — over a channel the host drains
/// with [`Self::poll`]. Dropping the listener stops and joins the thread.
#[derive(Debug)]
pub struct UdpCommandListener {
    local_addr: SocketAddr,
    stop: Arc<AtomicBool>,
    received: Receiver<String>,
    worker: Option<JoinHandle<()>>,
}

impl UdpCommandListener {
    /// Binds `address` and starts the listener thread. Port `0` binds an
    /// ephemeral port (see [`Self::local_addr`]).
    ///
    /// # Errors
    ///
    /// Returns [`TransportError::Socket`] when the bind (or thread spawn)
    /// fails — e.g. the port is already in use.
    pub fn bind(address: SocketAddr) -> Result<Self, TransportError> {
        let socket = UdpSocket::bind(address)?;
        socket.set_read_timeout(Some(LISTENER_POLL))?;
        let local_addr = socket.local_addr()?;
        let stop = Arc::new(AtomicBool::new(false));
        let (sender, received) = channel();
        let stop_flag = Arc::clone(&stop);
        let worker = std::thread::Builder::new()
            .name("orca-udp-in".to_owned())
            .spawn(move || listen(&socket, &stop_flag, &sender))?;
        Ok(Self {
            local_addr,
            stop,
            received,
            worker: Some(worker),
        })
    }

    /// The bound local address (the actual port when bound with port `0`).
    #[must_use]
    pub const fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Drains the command strings received since the last poll, in arrival
    /// order.
    #[must_use]
    pub fn poll(&self) -> Vec<String> {
        self.received.try_iter().collect()
    }
}

impl Drop for UdpCommandListener {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

/// The listener thread body: receive, decode, forward, until stopped.
fn listen(socket: &UdpSocket, stop: &AtomicBool, sender: &Sender<String>) {
    let mut buffer = [0_u8; LISTENER_BUFFER_BYTES];
    while !stop.load(Ordering::Relaxed) {
        match socket.recv_from(&mut buffer) {
            Ok((length, _)) => {
                let message = String::from_utf8_lossy(&buffer[..length])
                    .trim_end()
                    .to_owned();
                if sender.send(message).is_err() {
                    break;
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(_) => break,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use rosc::decoder;

    use super::*;

    fn loopback_receiver() -> (UdpSocket, SocketAddr) {
        let socket = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind loopback receiver");
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("set read timeout");
        let address = socket.local_addr().expect("local addr");
        (socket, address)
    }

    fn receive(socket: &UdpSocket) -> Vec<u8> {
        let mut buffer = [0_u8; 1024];
        let (length, _) = socket.recv_from(&mut buffer).expect("datagram arrives");
        buffer[..length].to_vec()
    }

    #[test]
    fn udp_sends_the_message_verbatim() {
        let (receiver, address) = loopback_receiver();
        let mut transport = UdpTransport::new(address);
        transport.send("hello5").expect("send succeeds");
        assert_eq!(receive(&receiver), b"hello5");
    }

    #[test]
    fn udp_sends_an_empty_message_as_an_empty_datagram() {
        let (receiver, address) = loopback_receiver();
        let mut transport = UdpTransport::new(address);
        transport.send("").expect("send succeeds");
        assert_eq!(receive(&receiver), b"");
    }

    #[test]
    fn udp_retarget_takes_effect_on_the_next_send() {
        let (first, first_address) = loopback_receiver();
        let (second, second_address) = loopback_receiver();
        let mut transport = UdpTransport::new(first_address);
        transport.send("a").expect("send succeeds");
        transport.set_target(second_address);
        assert_eq!(transport.target(), second_address);
        transport.send("b").expect("send succeeds");
        assert_eq!(receive(&first), b"a");
        assert_eq!(receive(&second), b"b");
    }

    #[test]
    fn osc_message_decodes_with_slash_path_and_base36_int_args() {
        let (receiver, address) = loopback_receiver();
        let mut transport = OscTransport::new(address);
        // `=a0cz` -> path `a`, args `0cz` -> /a with ints 0, 12, 35.
        transport.send('a', "0cz").expect("send succeeds");
        let payload = receive(&receiver);
        let (_, packet) = decoder::decode_udp(&payload).expect("valid OSC packet");
        let OscPacket::Message(message) = packet else {
            panic!("expected a message packet");
        };
        assert_eq!(message.addr, "/a");
        assert_eq!(
            message.args,
            vec![OscType::Int(0), OscType::Int(12), OscType::Int(35)]
        );
    }

    #[test]
    fn udp_command_listener_receives_datagrams_on_a_loopback_port() {
        let listener = UdpCommandListener::bind((Ipv4Addr::LOCALHOST, 0).into())
            .expect("bind ephemeral listener");
        let address = listener.local_addr();
        assert_ne!(address.port(), 0, "ephemeral port resolved");

        let sender = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind sender");
        sender
            .send_to(b"bpm:140\n", address)
            .expect("send datagram");

        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let mut received = Vec::new();
        while received.is_empty() && std::time::Instant::now() < deadline {
            received = listener.poll();
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(
            received,
            vec!["bpm:140".to_owned()],
            "trailing whitespace trimmed"
        );
    }

    #[test]
    fn udp_command_listener_bind_fails_on_an_occupied_port() {
        let holder = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind holder");
        let address = holder.local_addr().expect("local addr");
        let error = UdpCommandListener::bind(address).expect_err("port is taken");
        assert!(error.to_string().contains("socket error"), "got: {error}");
    }

    #[test]
    fn osc_message_with_no_args_is_just_the_address() {
        let (receiver, address) = loopback_receiver();
        let mut transport = OscTransport::new(address);
        transport.send('7', "").expect("send succeeds");
        let payload = receive(&receiver);
        let (_, packet) = decoder::decode_udp(&payload).expect("valid OSC packet");
        let OscPacket::Message(message) = packet else {
            panic!("expected a message packet");
        };
        assert_eq!(message.addr, "/7");
        assert!(message.args.is_empty());
    }
}
