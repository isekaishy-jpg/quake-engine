//! Real sockets on this machine, with and without loss.

use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

use engine_net::{NetClient, NetServer, Transport};

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
        }
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
            if !self.lost()
                && let Some(back) = self.backs.get(&from)
            {
                let _sent = back.send_to(&packet, self.server);
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

#[test]
fn messages_cross_real_sockets_both_ways() {
    let mut server = NetServer::bind(0, 4).expect("a port");
    let address = format!("127.0.0.1:{}", server.port());
    let mut client = NetClient::connect(&address).expect("an address");
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
    let mut server = NetServer::bind(0, 4).expect("a port");
    let address = format!("127.0.0.1:{}", server.port());
    let mut client = NetClient::connect(&address).expect("an address");
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
    let mut server = NetServer::bind(0, 4).expect("a port");
    let target = SocketAddr::from(([127, 0, 0, 1], server.port()));
    let proxy = LossyProxy::new(target, drop_every);
    let mut client = NetClient::connect(&proxy.address().to_string()).expect("an address");
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
    let mut server = NetServer::bind(0, 4).expect("a port");
    let address = format!("127.0.0.1:{}", server.port());
    let mut client = NetClient::connect(&address).expect("an address");
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
    let mut server = NetServer::bind(0, 1).expect("a port");
    let address = format!("127.0.0.1:{}", server.port());
    let mut first = NetClient::connect(&address).expect("an address");
    let mut none = None;
    let _seat = connect(&mut server, &mut first, &mut none);
    let mut second = NetClient::connect(&address).expect("an address");
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
    assert!(NetClient::connect("not a real host name!").is_err());
}
