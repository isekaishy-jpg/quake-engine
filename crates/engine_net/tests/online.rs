//! Real sockets on this machine, with and without loss.

use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use engine_net::{NetClient, NetServer, Transport};
use renet::{ConnectionConfig, RenetClient};
use renet_netcode::{ClientAuthentication, NetcodeClientTransport};

/// A tick.
const TICK: Duration = Duration::from_micros(13_889);

/// How long a test may take to get anywhere.
const PATIENCE: Duration = Duration::from_secs(10);

/// Forwards packets between clients and a server, dropping some.
struct LossyProxy {
    /// Where clients send.
    front: UdpSocket,
    /// The server.
    server: SocketAddr,
    /// A socket for each client, towards the server.
    backs: HashMap<SocketAddr, UdpSocket>,
    /// Which packet this is.
    count: u32,
    /// One packet in this many is lost, each way.
    drop_every: u32,
    /// Every packet that's gone through.
    seen: Vec<Vec<u8>>,
    /// One packet in this many has a byte changed on the way, if any.
    spoil_every: Option<u32>,
}

impl LossyProxy {
    /// A proxy in front of `server`, losing one packet in `drop_every`.
    fn new(server: SocketAddr, drop_every: u32) -> Self {
        let front = UdpSocket::bind("127.0.0.1:0").expect("a socket");
        front.set_nonblocking(true).expect("nonblocking");
        Self {
            front,
            server,
            backs: HashMap::new(),
            count: 0,
            drop_every,
            seen: Vec::new(),
            spoil_every: None,
        }
    }

    /// What goes on, after noting it, and perhaps spoiling it.
    fn pass(&mut self, packet: Vec<u8>) -> Vec<u8> {
        self.seen.push(packet.clone());
        let mut packet = packet;
        if let Some(every) = self.spoil_every
            && self.count.is_multiple_of(every)
            && let Some(last) = packet.last_mut()
        {
            *last ^= 0x55;
        }
        packet
    }

    /// The address clients use.
    fn address(&self) -> SocketAddr {
        self.front.local_addr().expect("an address")
    }

    /// Whether this packet is lost.
    fn lost(&mut self) -> bool {
        self.count = self.count.wrapping_add(1);
        self.count.is_multiple_of(self.drop_every)
    }

    /// Moves everything waiting along.
    fn pump(&mut self) {
        let mut buffer = [0; 2048];
        let mut sends = Vec::new();
        while let Ok((length, from)) = self.front.recv_from(&mut buffer) {
            let _back = self.backs.entry(from).or_insert_with(|| {
                let socket = UdpSocket::bind("127.0.0.1:0").expect("a socket");
                socket.set_nonblocking(true).expect("nonblocking");
                socket
            });
            let packet = buffer[..length].to_vec();
            sends.push((from, packet));
        }
        for (from, packet) in sends {
            if !self.lost() {
                let packet = self.pass(packet);
                if let Some(back) = self.backs.get(&from) {
                    let _sent = back.send_to(&packet, self.server);
                }
            }
        }
        let mut replies = Vec::new();
        for (client, back) in &self.backs {
            while let Ok((length, _from)) = back.recv_from(&mut buffer) {
                replies.push((*client, buffer[..length].to_vec()));
            }
        }
        for (client, packet) in replies {
            if !self.lost() {
                let packet = self.pass(packet);
                let _sent = self.front.send_to(&packet, client);
            }
        }
    }
}

/// A server with a client connected through `address`.
fn run_until(
    server: &mut NetServer,
    client: &mut NetClient,
    proxy: &mut Option<LossyProxy>,
    done: &mut dyn FnMut(&mut NetServer, &mut NetClient) -> bool,
) {
    let started = Instant::now();
    while !done(server, client) {
        assert!(started.elapsed() < PATIENCE, "it took too long");
        server.update(TICK);
        client.update(TICK);
        server.flush();
        client.flush();
        if let Some(proxy) = proxy {
            proxy.pump();
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// Connects a client, and returns the server's end of it.
fn connect(
    server: &mut NetServer,
    client: &mut NetClient,
    proxy: &mut Option<LossyProxy>,
) -> Box<dyn Transport> {
    let mut arrival = None;
    run_until(server, client, proxy, &mut |server, client| {
        arrival = arrival.take().or_else(|| server.take_arrival());
        arrival.is_some() && client.is_ready()
    });
    arrival.expect("a client")
}

/// A connection to a game with `wanted` for a password, using `given`, which
/// runs until it's connected or refused. Returns the client, and whether the
/// server ever saw it.
fn try_password(wanted: &str, given: &str) -> (NetClient, bool) {
    let mut server = NetServer::bind(0, 4, wanted).expect("a port");
    let address = format!("127.0.0.1:{}", server.port());
    let mut client = NetClient::connect(&address, given).expect("an address");
    let mut arrived = false;
    run_until(
        &mut server,
        &mut client,
        &mut None,
        &mut |server, client| {
            arrived |= server.take_arrival().is_some();
            (arrived && client.is_ready()) || !client.is_connected()
        },
    );
    (client, arrived)
}

#[test]
fn the_right_password_gets_in() {
    let (client, arrived) = try_password("rocket", "rocket");
    assert!(client.is_ready() && arrived);
    assert_eq!(client.refusal(), None);
}

#[test]
fn the_wrong_password_is_refused() {
    for given in ["rockets", "", "ROCKET"] {
        let (client, arrived) = try_password("rocket", given);
        assert!(!arrived, "{given:?} got in");
        assert!(!client.is_connected());
        assert_eq!(client.refusal().as_deref(), Some("wrong password"));
    }
}

#[test]
fn a_password_isnt_needed_in_an_open_game_but_has_to_match() {
    assert!(try_password("", "").1);
    assert!(!try_password("", "something").1);
}

#[test]
fn nobody_watching_can_read_what_goes_past() {
    let mut server = NetServer::bind(0, 4, "rocket").expect("a port");
    let backend = SocketAddr::from(([127, 0, 0, 1], server.port()));
    let mut proxy = Some(LossyProxy::new(backend, u32::MAX));
    let address = proxy.as_ref().expect("a proxy").address().to_string();
    let mut client = NetClient::connect(&address, "rocket").expect("an address");
    let mut seat = connect(&mut server, &mut client, &mut proxy);
    let (down, up) = (
        b"a secret going down".to_vec(),
        b"a secret going up".to_vec(),
    );
    seat.send(down.clone());
    client.send(up.clone());
    let (mut heard_down, mut heard_up) = (None, None);
    run_until(
        &mut server,
        &mut client,
        &mut proxy,
        &mut |_server, client| {
            heard_down = heard_down.take().or_else(|| client.receive());
            heard_up = heard_up.take().or_else(|| seat.receive());
            heard_down.is_some() && heard_up.is_some()
        },
    );
    assert_eq!(heard_down.as_ref(), Some(&down), "it did go through");
    assert_eq!(heard_up.as_ref(), Some(&up));
    let seen = proxy.expect("a proxy").seen;
    assert!(seen.len() > 4, "and the proxy saw it go");
    let shows =
        |packet: &[u8], words: &[u8]| packet.windows(words.len()).any(|window| window == words);
    for packet in &seen {
        assert!(
            !shows(packet, &down) && !shows(packet, &up),
            "it's in plain text"
        );
        assert!(!shows(packet, b"rocket"), "and so is the password");
    }
}

#[test]
fn a_client_with_no_token_never_gets_in() {
    // The old way in, whose keys are all zeros, so anyone could read it.
    let mut server = NetServer::bind(0, 4, "").expect("a port");
    let server_addr = SocketAddr::from(([127, 0, 0, 1], server.port()));
    let socket = UdpSocket::bind("127.0.0.1:0").expect("a socket");
    let authentication = ClientAuthentication::Unsecure {
        protocol_id: 0x5155_414B_4531_0002,
        client_id: 1,
        server_addr,
        user_data: None,
    };
    let time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("a time");
    let mut transport =
        NetcodeClientTransport::new(time, authentication, socket).expect("a transport");
    let mut client = RenetClient::new(ConnectionConfig::default());
    let started = Instant::now();
    while started.elapsed() < Duration::from_millis(300) {
        client.update(TICK);
        let _result = transport.update(TICK, &mut client);
        server.update(TICK);
        server.flush();
        assert!(server.take_arrival().is_none(), "it got in");
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(!client.is_connected());
}

#[test]
fn a_spoiled_packet_is_dropped_and_sent_again() {
    let mut server = NetServer::bind(0, 4, "").expect("a port");
    let backend = SocketAddr::from(([127, 0, 0, 1], server.port()));
    let lossy = LossyProxy::new(backend, u32::MAX);
    let address = lossy.address().to_string();
    let mut proxy = Some(lossy);
    let mut client = NetClient::connect(&address, "").expect("an address");
    let mut seat = connect(&mut server, &mut client, &mut proxy);
    if let Some(proxy) = &mut proxy {
        proxy.spoil_every = Some(3);
    }
    for number in 0..50u8 {
        seat.send(vec![number; 40]);
    }
    let mut heard = Vec::new();
    run_until(
        &mut server,
        &mut client,
        &mut proxy,
        &mut |_server, client| {
            heard.extend(std::iter::from_fn(|| client.receive()));
            heard.len() >= 50
        },
    );
    let expected: Vec<Vec<u8>> = (0..50u8).map(|number| vec![number; 40]).collect();
    assert_eq!(heard, expected, "all of it, unchanged, in order");
}

/// How many answers `requests` get, and the largest of them.
fn answers_to(requests: &[Vec<u8>]) -> (usize, usize) {
    let mut server = NetServer::bind(0, 4, "").expect("a port");
    let target = SocketAddr::from(([127, 0, 0, 1], server.port()));
    let socket = UdpSocket::bind("127.0.0.1:0").expect("a socket");
    socket.set_nonblocking(true).expect("nonblocking");
    for request in requests {
        let _sent = socket.send_to(request, target).expect("it goes");
    }
    let started = Instant::now();
    let (mut count, mut largest) = (0, 0);
    let mut buffer = [0; 2048];
    while started.elapsed() < Duration::from_millis(200) {
        server.update(TICK);
        while let Ok((length, _from)) = socket.recv_from(&mut buffer) {
            count += 1;
            largest = largest.max(length);
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    (count, largest)
}

#[test]
fn an_answer_is_never_bigger_than_its_request() {
    // A valid curve point, taken from a real client's first packet.
    let proxy_server = NetServer::bind(0, 4, "").expect("a port");
    let backend = SocketAddr::from(([127, 0, 0, 1], proxy_server.port()));
    let mut lossy = LossyProxy::new(backend, u32::MAX);
    let address = lossy.address().to_string();
    let mut client = NetClient::connect(&address, "").expect("an address");
    client.update(TICK);
    std::thread::sleep(Duration::from_millis(5));
    lossy.pump();
    let real = lossy.seen.first().expect("a request").clone();
    assert_eq!(real.len(), 1200);
    let (count, largest) = answers_to(std::slice::from_ref(&real));
    assert_eq!(count, 1, "a request of the right size is answered");
    assert!(largest <= real.len(), "{largest}");
    let short = real[..600].to_vec();
    assert_eq!(answers_to(&[short]).0, 0, "and one that's cut short isn't");
}

#[test]
fn someone_guessing_passwords_is_slowed_right_down() {
    // Different messages from one address, as fast as they can be sent.
    let mut server = NetServer::bind(0, 4, "rocket").expect("a port");
    let port = server.port();
    let started = Instant::now();
    let mut answered = 0;
    let target = format!("127.0.0.1:{port}");
    let mut proxy_seen = Vec::new();
    for _ in 0..40 {
        let mut lossy = LossyProxy::new(SocketAddr::from(([127, 0, 0, 1], port)), u32::MAX);
        let address = lossy.address().to_string();
        let mut client = NetClient::connect(&address, "guess").expect("an address");
        client.update(TICK);
        std::thread::sleep(Duration::from_millis(1));
        lossy.pump();
        proxy_seen.append(&mut lossy.seen);
        server.update(TICK);
        std::thread::sleep(Duration::from_millis(1));
        lossy.pump();
        answered += lossy.seen.len();
    }
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "{target} took too long"
    );
    assert_eq!(proxy_seen.len(), 40, "they all asked");
    assert!(answered <= 1, "one guess a second, not {answered}");
}

#[test]
fn messages_cross_real_sockets_both_ways() {
    let mut server = NetServer::bind(0, 4, "").expect("a port");
    let address = format!("127.0.0.1:{}", server.port());
    let mut client = NetClient::connect(&address, "").expect("an address");
    let mut none = None;
    let mut seat = connect(&mut server, &mut client, &mut none);
    seat.send(vec![1, 2, 3]);
    seat.send_unreliable(vec![4]);
    client.send(vec![5]);
    let (mut to_client, mut to_server) = (Vec::new(), Vec::new());
    run_until(
        &mut server,
        &mut client,
        &mut none,
        &mut |_server, client| {
            to_client.extend(std::iter::from_fn(|| client.receive()));
            to_server.extend(std::iter::from_fn(|| seat.receive()));
            to_client.len() == 2 && to_server.len() == 1
        },
    );
    assert!(to_client.contains(&vec![1, 2, 3]));
    assert!(to_client.contains(&vec![4]));
    assert_eq!(to_server, [vec![5]]);
}

#[test]
fn a_large_message_arrives_whole() {
    let mut server = NetServer::bind(0, 4, "").expect("a port");
    let address = format!("127.0.0.1:{}", server.port());
    let mut client = NetClient::connect(&address, "").expect("an address");
    let mut none = None;
    let mut seat = connect(&mut server, &mut client, &mut none);
    let big: Vec<u8> = (0..60_000_u32)
        .map(|index| u8::try_from(index % 251).expect("small"))
        .collect();
    seat.send_unreliable(big.clone());
    let mut received = None;
    run_until(
        &mut server,
        &mut client,
        &mut none,
        &mut |_server, client| {
            received = received.take().or_else(|| client.receive());
            received.is_some()
        },
    );
    assert_eq!(received.as_ref(), Some(&big));
}

/// Sends 100 reliable messages through a proxy losing one packet in
/// `drop_every`, each way, and checks they all arrive in order.
fn reliable_messages_survive(drop_every: u32) {
    let mut server = NetServer::bind(0, 4, "").expect("a port");
    let target = SocketAddr::from(([127, 0, 0, 1], server.port()));
    let proxy = LossyProxy::new(target, drop_every);
    let mut client = NetClient::connect(&proxy.address().to_string(), "").expect("an address");
    let mut proxy = Some(proxy);
    let mut seat = connect(&mut server, &mut client, &mut proxy);
    for number in 0..100_u8 {
        seat.send(vec![number]);
    }
    let mut received = Vec::new();
    run_until(
        &mut server,
        &mut client,
        &mut proxy,
        &mut |_server, client| {
            received.extend(std::iter::from_fn(|| client.receive()));
            received.len() >= 100
        },
    );
    let expected: Vec<Vec<u8>> = (0..100_u8).map(|number| vec![number]).collect();
    assert_eq!(received, expected, "all of them, in order");
}

/// What a real connection should stay well under.
#[test]
fn reliable_messages_arrive_at_two_percent_loss() {
    reliable_messages_survive(50);
}

/// The most the game is designed to play well at.
#[test]
fn reliable_messages_arrive_at_five_percent_loss() {
    reliable_messages_survive(20);
}

/// Not a target: a margin, to show nothing breaks on a very bad connection.
#[test]
fn reliable_messages_still_arrive_on_a_very_bad_connection() {
    reliable_messages_survive(5);
}

#[test]
fn a_client_going_away_is_noticed() {
    let mut server = NetServer::bind(0, 4, "").expect("a port");
    let address = format!("127.0.0.1:{}", server.port());
    let mut client = NetClient::connect(&address, "").expect("an address");
    let mut none = None;
    let seat = connect(&mut server, &mut client, &mut none);
    assert!(seat.is_connected());
    drop(client);
    let started = Instant::now();
    while seat.is_connected() {
        assert!(started.elapsed() < PATIENCE, "it wasn't noticed");
        server.update(TICK);
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn a_server_full_of_clients_turns_more_away() {
    let mut server = NetServer::bind(0, 1, "").expect("a port");
    let address = format!("127.0.0.1:{}", server.port());
    let mut first = NetClient::connect(&address, "").expect("an address");
    let mut none = None;
    let _seat = connect(&mut server, &mut first, &mut none);
    let mut second = NetClient::connect(&address, "").expect("an address");
    let started = Instant::now();
    while second.is_connected() {
        assert!(started.elapsed() < PATIENCE, "it was never refused");
        server.update(TICK);
        first.update(TICK);
        second.update(TICK);
        server.flush();
        first.flush();
        second.flush();
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(!second.is_ready());
}

#[test]
fn a_bad_address_is_an_error() {
    assert!(NetClient::connect("not a real host name!", "").is_err());
}
