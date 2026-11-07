//! Connections over the network, through `renet` (proposals 0046, 0058 and
//! 0059).

use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::rc::Rc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::{fmt, io};

use renet::{
    ChannelConfig, ClientId, ConnectionConfig, RenetClient, RenetServer, SendType, ServerEvent,
};
use renet_netcode::{
    ClientAuthentication, ConnectToken, NETCODE_KEY_BYTES, NetcodeClientTransport,
    ServerAuthentication, ServerConfig, generate_random_bytes,
};
use renetcode::{NETCODE_MAX_PACKET_BYTES, NETCODE_USER_DATA_BYTES, NetcodeServer, ServerResult};

use crate::handshake::{Asker, Doorman, Expected, Heard, is_request};
use crate::identity::{HostIdentity, PlayerIdentity, PublicKey, tidy_code};
use crate::transport::Transport;

/// Tells this game apart from others on the same port. It changes when the
/// messages do.
const PROTOCOL_ID: u64 = 0x5155_414B_4531_0008;

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

/// How a game is hosted.
pub struct HostOptions {
    /// What joining takes, or nothing for an open game.
    pub password: String,
    /// Who the host is.
    pub identity: HostIdentity,
    /// Players who aren't let in.
    pub banned: BTreeSet<PublicKey>,
}

impl HostOptions {
    /// A game with `password`, under a new identity.
    #[must_use]
    pub fn new(password: &str) -> Self {
        Self {
            password: password.to_owned(),
            identity: HostIdentity::generate(),
            banned: BTreeSet::new(),
        }
    }
}

/// How a game is joined.
pub struct JoinOptions {
    /// The password, if the host wants one.
    pub password: String,
    /// Who this player is.
    pub identity: PlayerIdentity,
    /// The key this host had last time, if it's been met.
    pub known_host: Option<PublicKey>,
    /// The code the host gave out, if any.
    pub code: Option<String>,
}

impl JoinOptions {
    /// A first visit with `password`, under a new identity.
    #[must_use]
    pub fn new(password: &str) -> Self {
        Self {
            password: password.to_owned(),
            identity: PlayerIdentity::generate(),
            known_host: None,
            code: None,
        }
    }
}

/// `address`, with the default port if it has none.
#[must_use]
pub fn full_address(address: &str) -> String {
    if address.contains(':') {
        address.to_owned()
    } else {
        format!("{address}:{DEFAULT_PORT}")
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
    /// Who each client is, as their token says.
    people: HashMap<ClientId, PublicKey>,
    /// Whether the identity has changed since it was last kept.
    identity_changed: bool,
}

impl ServerEnd {
    /// A new connect token for `player`, as bytes.
    fn mint(
        private_key: &[u8; NETCODE_KEY_BYTES],
        address: SocketAddr,
        player: PublicKey,
    ) -> Option<Vec<u8>> {
        let client_id = u64::from_le_bytes(generate_random_bytes());
        let mut user_data = [0; NETCODE_USER_DATA_BYTES];
        user_data[..32].copy_from_slice(&player);
        let token = ConnectToken::generate(
            now(),
            PROTOCOL_ID,
            TOKEN_SECONDS,
            client_id,
            TIMEOUT_SECONDS,
            vec![address],
            Some(&user_data),
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
                        let answer = self.doorman.answer(packet, from, now(), |player| {
                            Self::mint(&key, address, player)
                        });
                        if let Some(answer) = answer {
                            let _sent = self.socket.send_to(&answer, from);
                        }
                    } else {
                        let result = self.netcode.process_packet(from, packet);
                        take_result(result, &self.socket, &mut self.server, &mut self.people);
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
            take_result(result, &self.socket, &mut self.server, &mut self.people);
        }
        for client in self.server.disconnections_id() {
            let result = self.netcode.disconnect(client);
            take_result(result, &self.socket, &mut self.server, &mut self.people);
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
            take_result(result, &self.socket, &mut self.server, &mut self.people);
        }
    }
}

/// Acts on what `netcode` made of something.
fn take_result(
    result: ServerResult<'_, '_>,
    socket: &UdpSocket,
    server: &mut RenetServer,
    people: &mut HashMap<ClientId, PublicKey>,
) {
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
            user_data,
        } => {
            if let Ok(player) = PublicKey::try_from(&user_data[..32]) {
                let _new = people.insert(client_id, player);
            }
            server.add_connection(client_id);
            let _sent = socket.send_to(payload, addr);
        }
        ServerResult::ClientDisconnected {
            client_id,
            addr,
            payload,
        } => {
            server.remove_connection(client_id);
            let _gone = people.remove(&client_id);
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
    /// Clients that have gone, until their ends are dropped.
    gone: Rc<RefCell<HashSet<ClientId>>>,
}

impl NetServer {
    /// Listens on `port`, for up to `max_clients`. Anyone can join if there's
    /// no password. Either way, all traffic is encrypted.
    ///
    /// # Errors
    ///
    /// Returns [`NetError`] if the port can't be used.
    pub fn bind(port: u16, max_clients: usize, options: HostOptions) -> Result<Self, NetError> {
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
                doorman: Doorman::new(&options.password, options.identity, options.banned),
                private_key,
                address,
                people: HashMap::new(),
                identity_changed: false,
            })),
            arrived: VecDeque::new(),
            gone: Rc::new(RefCell::new(HashSet::new())),
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
                    let _new = self.gone.borrow_mut().insert(client_id);
                }
            }
        }
    }

    /// Sends what's waiting.
    pub fn flush(&mut self) {
        self.end.borrow_mut().send_packets();
    }

    /// Takes a client that has just connected. Having got this far, they've
    /// been told of any change of the host's key.
    pub fn take_arrival(&mut self) -> Option<Box<dyn Transport>> {
        let id = self.arrived.pop_front()?;
        {
            let mut end = self.end.borrow_mut();
            let player = end.people.get(&id).copied();
            if let Some(player) = player {
                let changed = end.doorman.identity.note_player(player);
                end.identity_changed |= changed;
            }
        }
        Some(Box::new(ServerSide {
            end: Rc::clone(&self.end),
            gone: Rc::clone(&self.gone),
            id,
        }))
    }

    /// Who the host is now.
    #[must_use]
    pub fn identity(&self) -> HostIdentity {
        self.end.borrow().doorman.identity.clone()
    }

    /// Changes the host's key, telling players as they come back.
    pub fn rotate_identity(&mut self) {
        let mut end = self.end.borrow_mut();
        end.doorman.identity.rotate();
        end.identity_changed = true;
    }

    /// Whether the identity has changed since this was last asked.
    pub fn take_identity_changed(&mut self) -> bool {
        std::mem::take(&mut self.end.borrow_mut().identity_changed)
    }

    /// Stops `player` from getting in.
    pub fn ban(&mut self, player: PublicKey) {
        let _new = self.end.borrow_mut().doorman.banned.insert(player);
    }

    /// Lets `player` in again.
    pub fn unban(&mut self, player: &PublicKey) {
        let _was = self.end.borrow_mut().doorman.banned.remove(player);
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
    /// Clients that have gone, until their ends are dropped.
    gone: Rc<RefCell<HashSet<ClientId>>>,
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

    fn peer_identity(&self) -> Option<PublicKey> {
        self.end.borrow().people.get(&self.id).copied()
    }
}

impl Drop for ServerSide {
    fn drop(&mut self) {
        let _was = self.gone.borrow_mut().remove(&self.id);
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
        /// Who the host proved to be.
        host: PublicKey,
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
    /// Starts connecting to `address`, with the default port if it has none.
    ///
    /// # Errors
    ///
    /// Returns [`NetError`] if the address can't be found.
    pub fn connect(address: &str, options: JoinOptions) -> Result<Self, NetError> {
        let server = full_address(address)
            .to_socket_addrs()?
            .find(SocketAddr::is_ipv4)
            .ok_or_else(|| NetError(format!("can't find {address}")))?;
        let socket = UdpSocket::bind(SocketAddr::from(([0, 0, 0, 0], 0)))?;
        socket.set_nonblocking(true)?;
        Ok(Self {
            stage: Stage::Asking {
                asker: Box::new(Asker::new(Expected {
                    password: options.password,
                    identity: options.identity,
                    known_host: options.known_host,
                    code: options.code.as_deref().map(tidy_code),
                })),
                socket,
                server,
                waited: Duration::ZERO,
                // So the first request goes straight away.
                since_asking: ASK_AGAIN,
            },
        })
    }

    /// Opens the connection `token` allows, to `server`.
    fn open(token: ConnectToken, host: PublicKey, socket: UdpSocket, server: SocketAddr) -> Stage {
        // The host can't know what it's called from out here.
        let mut connect_token = token;
        connect_token.server_addresses = [None; 32];
        connect_token.server_addresses[0] = Some(server);
        let authentication = ClientAuthentication::Secure { connect_token };
        match NetcodeClientTransport::new(now(), authentication, socket) {
            Ok(transport) => Stage::Open {
                client: Box::new(RenetClient::new(connection_config())),
                transport: Box::new(transport),
                host,
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
                Heard::Token(token, host) => return Self::open(*token, host, socket, server),
                Heard::Refused(reason) => return Stage::Refused(reason),
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

    fn peer_identity(&self) -> Option<PublicKey> {
        match &self.stage {
            Stage::Open { host, .. } => Some(*host),
            _ => None,
        }
    }

    fn update(&mut self, elapsed: Duration) {
        if let Stage::Open {
            client, transport, ..
        } = &mut self.stage
        {
            client.update(elapsed);
            let _lost_packets = transport.update(elapsed, client);
            return;
        }
        let stage = std::mem::replace(&mut self.stage, Stage::Refused(String::new()));
        self.stage = Self::ask(stage, elapsed);
    }

    fn flush(&mut self) {
        if let Stage::Open {
            client, transport, ..
        } = &mut self.stage
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
