//! Connections over the network, through `renet` (proposals 0046 and 0058).

use std::cell::RefCell;
use std::collections::VecDeque;
use std::fmt;
use std::io;
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::rc::Rc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use renet::{
    ChannelConfig, ClientId, ConnectionConfig, RenetClient, RenetServer, SendType, ServerEvent,
};
use renet_netcode::{
    ClientAuthentication, ConnectToken, NETCODE_KEY_BYTES, NetcodeClientTransport,
    ServerAuthentication, ServerConfig, generate_random_bytes,
};
use renetcode::{NETCODE_MAX_PACKET_BYTES, NetcodeServer, ServerResult};

use crate::handshake::{Asker, Doorman, Heard, is_request};
use crate::transport::Transport;

/// Tells this game apart from others on the same port. It changes when the
/// messages do.
const PROTOCOL_ID: u64 = 0x5155_414B_4531_0002;

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

/// How long a connect token lasts, in seconds.
const TOKEN_SECONDS: u64 = 60;

/// How long a quiet connection lasts, in seconds, from `netcode`'s own default.
const TIMEOUT_SECONDS: i32 = 15;

/// How long before asking the host for a token again.
const ASK_AGAIN: Duration = Duration::from_millis(200);

/// How long to keep asking.
const ASK_PATIENCE: Duration = Duration::from_secs(6);

/// A connection that couldn't be made.
#[derive(Debug)]
pub struct NetError(String);

impl fmt::Display for NetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for NetError {}

impl From<io::Error> for NetError {
    fn from(error: io::Error) -> Self {
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
    /// The encryption, and who's connected.
    netcode: NetcodeServer,
    /// The socket.
    socket: UdpSocket,
    /// Hands out tokens.
    doorman: Doorman,
    /// What tokens are sealed with, known only here.
    private_key: [u8; NETCODE_KEY_BYTES],
    /// Where this says it's listening, inside each token.
    address: SocketAddr,
}

impl ServerEnd {
    /// A new connect token, as bytes.
    fn mint(private_key: &[u8; NETCODE_KEY_BYTES], address: SocketAddr) -> Option<Vec<u8>> {
        let client_id = u64::from_le_bytes(generate_random_bytes());
        let token = ConnectToken::generate(
            now(),
            PROTOCOL_ID,
            TOKEN_SECONDS,
            client_id,
            TIMEOUT_SECONDS,
            vec![address],
            None,
            private_key,
        )
        .ok()?;
        let mut bytes = Vec::new();
        token.write(&mut bytes).ok()?;
        Some(bytes)
    }

    /// Hears everything waiting at the socket.
    fn read_socket(&mut self) {
        let mut buffer = [0; NETCODE_MAX_PACKET_BYTES];
        loop {
            match self.socket.recv_from(&mut buffer) {
                Ok((length, from)) => {
                    let packet = &mut buffer[..length];
                    if is_request(packet) {
                        let (key, address) = (self.private_key, self.address);
                        let answer = self
                            .doorman
                            .answer(packet, from, now(), || Self::mint(&key, address));
                        if let Some(answer) = answer {
                            let _sent = self.socket.send_to(&answer, from);
                        }
                    } else {
                        let result = self.netcode.process_packet(from, packet);
                        take_result(result, &self.socket, &mut self.server);
                    }
                }
                // A client that's gone can show up here on some systems.
                Err(error) if error.kind() == io::ErrorKind::ConnectionReset => {}
                Err(_) => break,
            }
        }
    }

    /// Moves everything on by `elapsed`.
    fn update(&mut self, elapsed: Duration) {
        self.server.update(elapsed);
        self.netcode.update(elapsed);
        self.read_socket();
        for client in self.netcode.clients_id() {
            let result = self.netcode.update_client(client);
            take_result(result, &self.socket, &mut self.server);
        }
        for client in self.server.disconnections_id() {
            let result = self.netcode.disconnect(client);
            take_result(result, &self.socket, &mut self.server);
        }
    }

    /// Sends what's waiting for everyone.
    fn send_packets(&mut self) {
        for client in self.server.clients_id() {
            let Ok(packets) = self.server.get_packets_to_send(client) else {
                continue;
            };
            for packet in packets {
                if let Ok((address, payload)) =
                    self.netcode.generate_payload_packet(client, &packet)
                {
                    let _sent = self.socket.send_to(payload, address);
                }
            }
        }
    }

    /// Tells everyone it's over.
    fn disconnect_all(&mut self) {
        for client in self.netcode.clients_id() {
            let result = self.netcode.disconnect(client);
            take_result(result, &self.socket, &mut self.server);
        }
    }
}

/// Acts on what `netcode` made of something.
fn take_result(result: ServerResult<'_, '_>, socket: &UdpSocket, server: &mut RenetServer) {
    match result {
        ServerResult::None => {}
        ServerResult::PacketToSend { payload, addr } => {
            let _sent = socket.send_to(payload, addr);
        }
        ServerResult::Payload { client_id, payload } => {
            let _bad_packets_are_dropped = server.process_packet_from(payload, client_id);
        }
        ServerResult::ClientConnected {
            client_id,
            addr,
            payload,
            ..
        } => {
            server.add_connection(client_id);
            let _sent = socket.send_to(payload, addr);
        }
        ServerResult::ClientDisconnected {
            client_id,
            addr,
            payload,
        } => {
            server.remove_connection(client_id);
            if let Some(payload) = payload {
                let _sent = socket.send_to(payload, addr);
            }
        }
    }
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
    /// Listens on `port`, for up to `max_clients` who know `password`. Anyone
    /// can join if it's empty. Either way, all traffic is encrypted.
    ///
    /// # Errors
    ///
    /// Returns [`NetError`] if the port can't be used.
    pub fn bind(port: u16, max_clients: usize, password: &str) -> Result<Self, NetError> {
        let socket = UdpSocket::bind(SocketAddr::from(([0, 0, 0, 0], port)))?;
        socket.set_nonblocking(true)?;
        let address = socket.local_addr()?;
        let private_key = generate_random_bytes();
        let config = ServerConfig {
            current_time: now(),
            max_clients,
            protocol_id: PROTOCOL_ID,
            public_addresses: vec![address],
            authentication: ServerAuthentication::Secure { private_key },
        };
        Ok(Self {
            end: Rc::new(RefCell::new(ServerEnd {
                server: RenetServer::new(connection_config()),
                netcode: NetcodeServer::new(config),
                socket,
                doorman: Doorman::new(password),
                private_key,
                address,
            })),
            arrived: VecDeque::new(),
            gone: Rc::new(RefCell::new(Vec::new())),
        })
    }

    /// The port it's listening on.
    #[must_use]
    pub fn port(&self) -> u16 {
        self.end.borrow().address.port()
    }

    /// Hears from the network, `elapsed` since last time.
    pub fn update(&mut self, elapsed: Duration) {
        let mut end = self.end.borrow_mut();
        end.update(elapsed);
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
        self.end.borrow_mut().send_packets();
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

    /// Tells everyone it's over, and sends that now.
    pub fn close(&mut self) {
        self.end.borrow_mut().disconnect_all();
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

/// How far a connection has got.
enum Stage {
    /// Getting a token from the host.
    Asking {
        /// The exchange.
        asker: Box<Asker>,
        /// The socket.
        socket: UdpSocket,
        /// The host.
        server: SocketAddr,
        /// How long it's been going.
        waited: Duration,
        /// How long since the last request.
        since_asking: Duration,
    },
    /// Connecting, or connected.
    Open {
        /// The connection.
        client: Box<RenetClient>,
        /// The socket and the encryption.
        transport: Box<NetcodeClientTransport>,
    },
    /// Turned down, and why.
    Refused(String),
}

/// A connection to a server.
pub struct NetClient {
    /// How far it's got.
    stage: Stage,
}

impl NetClient {
    /// Starts connecting to `address`, with the default port if it has none,
    /// and with `password` if the host wants one.
    ///
    /// # Errors
    ///
    /// Returns [`NetError`] if the address can't be found.
    pub fn connect(address: &str, password: &str) -> Result<Self, NetError> {
        let with_port = if address.contains(':') {
            address.to_owned()
        } else {
            format!("{address}:{DEFAULT_PORT}")
        };
        let server = with_port
            .to_socket_addrs()?
            .find(SocketAddr::is_ipv4)
            .ok_or_else(|| NetError(format!("can't find {address}")))?;
        let socket = UdpSocket::bind(SocketAddr::from(([0, 0, 0, 0], 0)))?;
        socket.set_nonblocking(true)?;
        Ok(Self {
            stage: Stage::Asking {
                asker: Box::new(Asker::new(password)),
                socket,
                server,
                waited: Duration::ZERO,
                // So the first request goes straight away.
                since_asking: ASK_AGAIN,
            },
        })
    }

    /// Opens the connection `token` allows, to `server`.
    fn open(token: ConnectToken, socket: UdpSocket, server: SocketAddr) -> Stage {
        // The host can't know what it's called from out here.
        let mut connect_token = token;
        connect_token.server_addresses = [None; 32];
        connect_token.server_addresses[0] = Some(server);
        let authentication = ClientAuthentication::Secure { connect_token };
        match NetcodeClientTransport::new(now(), authentication, socket) {
            Ok(transport) => Stage::Open {
                client: Box::new(RenetClient::new(connection_config())),
                transport: Box::new(transport),
            },
            Err(error) => Stage::Refused(error.to_string()),
        }
    }

    /// Carries on getting a token, `elapsed` since last time.
    fn ask(stage: Stage, elapsed: Duration) -> Stage {
        let Stage::Asking {
            mut asker,
            socket,
            server,
            waited,
            since_asking,
        } = stage
        else {
            return stage;
        };
        let mut buffer = [0; NETCODE_MAX_PACKET_BYTES];
        // Anything from elsewhere is ignored, without even being read.
        while let Ok((length, from)) = socket.recv_from(&mut buffer) {
            if from != server {
                continue;
            }
            match asker.hear(&buffer[..length]) {
                Heard::Nothing => {}
                Heard::Token(token) => return Self::open(*token, socket, server),
                Heard::Refused => return Stage::Refused("wrong password".to_owned()),
            }
        }
        let (waited, mut since_asking) = (waited + elapsed, since_asking + elapsed);
        if waited >= ASK_PATIENCE {
            return Stage::Refused("the server didn't answer".to_owned());
        }
        if since_asking >= ASK_AGAIN {
            since_asking = Duration::ZERO;
            for request in asker.requests() {
                let _sent = socket.send_to(&request, server);
            }
        }
        Stage::Asking {
            asker,
            socket,
            server,
            waited,
            since_asking,
        }
    }
}

impl Transport for NetClient {
    fn send(&mut self, bytes: Vec<u8>) {
        if let Stage::Open { client, .. } = &mut self.stage {
            client.send_message(RELIABLE, bytes);
        }
    }

    fn send_unreliable(&mut self, bytes: Vec<u8>) {
        if let Stage::Open { client, .. } = &mut self.stage {
            client.send_message(UNRELIABLE, bytes);
        }
    }

    fn receive(&mut self) -> Option<Vec<u8>> {
        let Stage::Open { client, .. } = &mut self.stage else {
            return None;
        };
        client
            .receive_message(RELIABLE)
            .or_else(|| client.receive_message(UNRELIABLE))
            .map(|bytes| bytes.to_vec())
    }

    fn is_connected(&self) -> bool {
        match &self.stage {
            Stage::Asking { .. } => true,
            Stage::Open { client, .. } => !client.is_disconnected(),
            Stage::Refused(_) => false,
        }
    }

    fn refusal(&self) -> Option<String> {
        match &self.stage {
            Stage::Refused(reason) => Some(reason.clone()),
            _ => None,
        }
    }

    fn update(&mut self, elapsed: Duration) {
        if let Stage::Open { client, transport } = &mut self.stage {
            client.update(elapsed);
            let _lost_packets = transport.update(elapsed, client);
            return;
        }
        let stage = std::mem::replace(&mut self.stage, Stage::Refused(String::new()));
        self.stage = Self::ask(stage, elapsed);
    }

    fn flush(&mut self) {
        if let Stage::Open { client, transport } = &mut self.stage
            && client.is_connected()
        {
            let _lost_packets = transport.send_packets(client);
        }
    }

    fn is_ready(&self) -> bool {
        matches!(&self.stage, Stage::Open { client, .. } if client.is_connected())
    }
}

impl Drop for NetClient {
    fn drop(&mut self) {
        if let Stage::Open { transport, .. } = &mut self.stage {
            transport.disconnect();
        }
    }
}
