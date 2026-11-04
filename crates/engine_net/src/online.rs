//! Connections over the network, through `renet` (proposal 0046).

use std::cell::RefCell;
use std::collections::VecDeque;
use std::fmt;
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::rc::Rc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use renet::{
    ChannelConfig, ClientId, ConnectionConfig, RenetClient, RenetServer, SendType, ServerEvent,
};
use renet_netcode::{
    ClientAuthentication, NetcodeClientTransport, NetcodeServerTransport, ServerAuthentication,
    ServerConfig,
};

use crate::transport::Transport;

/// Tells this game apart from others on the same port. It changes when the
/// messages do.
const PROTOCOL_ID: u64 = 0x5155_414B_4531_0001;

/// The port used when none is given, from `DEFAULTnet_hostport`.
pub const DEFAULT_PORT: u16 = 26000;

/// The channel of what has to arrive.
const RELIABLE: u8 = 0;

/// The channel of what may not.
const UNRELIABLE: u8 = 1;

/// How long before something unacknowledged is sent again.
const RESEND: Duration = Duration::from_millis(100);

/// How much a reliable channel holds while waiting. A level's model names and
/// static sounds arrive at once.
const RELIABLE_MEMORY: usize = 4 * 1024 * 1024;

/// How much an unreliable channel holds. A full snapshot is the largest thing
/// in it.
const UNRELIABLE_MEMORY: usize = 1024 * 1024;

/// A connection that couldn't be made.
#[derive(Debug)]
pub struct NetError(String);

impl fmt::Display for NetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for NetError {}

impl From<std::io::Error> for NetError {
    fn from(error: std::io::Error) -> Self {
        Self(error.to_string())
    }
}

/// The two channels, each way.
fn connection_config() -> ConnectionConfig {
    let channels = vec![
        ChannelConfig {
            channel_id: RELIABLE,
            max_memory_usage_bytes: RELIABLE_MEMORY,
            send_type: SendType::ReliableOrdered {
                resend_time: RESEND,
            },
        },
        ChannelConfig {
            channel_id: UNRELIABLE,
            max_memory_usage_bytes: UNRELIABLE_MEMORY,
            send_type: SendType::Unreliable,
        },
    ];
    ConnectionConfig {
        server_channels_config: channels.clone(),
        client_channels_config: channels,
        ..ConnectionConfig::default()
    }
}

/// The time since the epoch, which `netcode` counts from.
fn now() -> Duration {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
}

/// The server's side, shared with each client's transport.
struct ServerEnd {
    /// The connections.
    server: RenetServer,
    /// The sockets.
    transport: NetcodeServerTransport,
}

/// Listens for clients.
pub struct NetServer {
    /// What's shared.
    end: Rc<RefCell<ServerEnd>>,
    /// Clients that have connected and haven't been taken.
    arrived: VecDeque<ClientId>,
    /// Clients that have gone.
    gone: Rc<RefCell<Vec<ClientId>>>,
}

impl NetServer {
    /// Listens on `port`, for up to `max_clients`.
    ///
    /// # Errors
    ///
    /// Returns [`NetError`] if the port can't be used.
    pub fn bind(port: u16, max_clients: usize) -> Result<Self, NetError> {
        let socket = UdpSocket::bind(SocketAddr::from(([0, 0, 0, 0], port)))?;
        let address = socket.local_addr()?;
        let config = ServerConfig {
            current_time: now(),
            max_clients,
            protocol_id: PROTOCOL_ID,
            public_addresses: vec![address],
            authentication: ServerAuthentication::Unsecure,
        };
        let transport = NetcodeServerTransport::new(config, socket)?;
        Ok(Self {
            end: Rc::new(RefCell::new(ServerEnd {
                server: RenetServer::new(connection_config()),
                transport,
            })),
            arrived: VecDeque::new(),
            gone: Rc::new(RefCell::new(Vec::new())),
        })
    }

    /// The port it's listening on.
    #[must_use]
    pub fn port(&self) -> u16 {
        let end = self.end.borrow();
        end.transport
            .addresses()
            .first()
            .map_or(0, SocketAddr::port)
    }

    /// Hears from the network, `elapsed` since last time.
    pub fn update(&mut self, elapsed: Duration) {
        let mut guard = self.end.borrow_mut();
        let end = &mut *guard;
        end.server.update(elapsed);
        let _lost_packets = end.transport.update(elapsed, &mut end.server);
        while let Some(event) = end.server.get_event() {
            match event {
                ServerEvent::ClientConnected { client_id } => self.arrived.push_back(client_id),
                ServerEvent::ClientDisconnected { client_id, .. } => {
                    self.gone.borrow_mut().push(client_id);
                }
            }
        }
    }

    /// Sends what's waiting.
    pub fn flush(&mut self) {
        let mut guard = self.end.borrow_mut();
        let end = &mut *guard;
        end.transport.send_packets(&mut end.server);
    }

    /// Takes a client that has just connected.
    pub fn take_arrival(&mut self) -> Option<Box<dyn Transport>> {
        let id = self.arrived.pop_front()?;
        Some(Box::new(ServerSide {
            end: Rc::clone(&self.end),
            gone: Rc::clone(&self.gone),
            id,
        }))
    }

    /// Tells everyone it's over.
    pub fn close(&mut self) {
        let mut guard = self.end.borrow_mut();
        let end = &mut *guard;
        end.transport.disconnect_all(&mut end.server);
    }
}

/// One client, as the server reaches them.
struct ServerSide {
    /// What's shared.
    end: Rc<RefCell<ServerEnd>>,
    /// Clients that have gone.
    gone: Rc<RefCell<Vec<ClientId>>>,
    /// Who.
    id: ClientId,
}

impl Transport for ServerSide {
    fn send(&mut self, bytes: Vec<u8>) {
        self.end
            .borrow_mut()
            .server
            .send_message(self.id, RELIABLE, bytes);
    }

    fn send_unreliable(&mut self, bytes: Vec<u8>) {
        self.end
            .borrow_mut()
            .server
            .send_message(self.id, UNRELIABLE, bytes);
    }

    fn receive(&mut self) -> Option<Vec<u8>> {
        let mut end = self.end.borrow_mut();
        end.server
            .receive_message(self.id, RELIABLE)
            .or_else(|| end.server.receive_message(self.id, UNRELIABLE))
            .map(|bytes| bytes.to_vec())
    }

    fn is_connected(&self) -> bool {
        !self.gone.borrow().contains(&self.id)
    }
}

impl Drop for ServerSide {
    fn drop(&mut self) {
        if let Ok(mut end) = self.end.try_borrow_mut() {
            end.server.disconnect(self.id);
        }
    }
}

/// A connection to a server.
pub struct NetClient {
    /// The connection.
    client: RenetClient,
    /// The socket.
    transport: NetcodeClientTransport,
}

impl NetClient {
    /// Starts connecting to `address`, with the default port if it has none.
    ///
    /// # Errors
    ///
    /// Returns [`NetError`] if the address can't be found.
    pub fn connect(address: &str) -> Result<Self, NetError> {
        let with_port = if address.contains(':') {
            address.to_owned()
        } else {
            format!("{address}:{DEFAULT_PORT}")
        };
        let server_addr = with_port
            .to_socket_addrs()?
            .find(SocketAddr::is_ipv4)
            .ok_or_else(|| NetError(format!("can't find {address}")))?;
        let socket = UdpSocket::bind(SocketAddr::from(([0, 0, 0, 0], 0)))?;
        let time = now();
        let authentication = ClientAuthentication::Unsecure {
            protocol_id: PROTOCOL_ID,
            client_id: u64::try_from(time.as_micros() & u128::from(u64::MAX)).unwrap_or(1),
            server_addr,
            user_data: None,
        };
        let transport = NetcodeClientTransport::new(time, authentication, socket)
            .map_err(|error| NetError(error.to_string()))?;
        Ok(Self {
            client: RenetClient::new(connection_config()),
            transport,
        })
    }
}

impl Transport for NetClient {
    fn send(&mut self, bytes: Vec<u8>) {
        self.client.send_message(RELIABLE, bytes);
    }

    fn send_unreliable(&mut self, bytes: Vec<u8>) {
        self.client.send_message(UNRELIABLE, bytes);
    }

    fn receive(&mut self) -> Option<Vec<u8>> {
        self.client
            .receive_message(RELIABLE)
            .or_else(|| self.client.receive_message(UNRELIABLE))
            .map(|bytes| bytes.to_vec())
    }

    fn is_connected(&self) -> bool {
        !self.client.is_disconnected()
    }

    fn update(&mut self, elapsed: Duration) {
        self.client.update(elapsed);
        let _lost_packets = self.transport.update(elapsed, &mut self.client);
    }

    fn flush(&mut self) {
        if self.client.is_connected() {
            let _lost_packets = self.transport.send_packets(&mut self.client);
        }
    }

    fn is_ready(&self) -> bool {
        self.client.is_connected()
    }
}

impl Drop for NetClient {
    fn drop(&mut self) {
        self.transport.disconnect();
    }
}
