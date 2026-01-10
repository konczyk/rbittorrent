use crate::torrent::bencode;
use sha1::digest::core_api::CoreWrapper;
use sha1::digest::Output;
use sha1::{Digest, Sha1, Sha1Core};
use std::io;
use std::io::ErrorKind::{InvalidData, TimedOut, WouldBlock};
use std::net::{Ipv4Addr, UdpSocket};
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

pub struct Torrent {
    pub url: String,
    pub length: isize,
    pub piece_length: isize,
    pub pieces: Vec<u8>,
    pub info_hash: Output<CoreWrapper<Sha1Core>>,
}

impl Torrent {
    pub fn new(data: &[u8]) -> Torrent {
        let (node, _) = bencode::decode_bencoded_value(data);
        let d = match &node.value {
            bencode::BencodeValue::Dict(d) => d,
            _ => panic!("Expected dictionary")
        };

        let url = match d.get("announce") {
            Some(bencode::BencodeNode { value: bencode::BencodeValue::String(url), raw: _ }) => url.clone(),
            _ => panic!("Torrent URL not found"),
        };

        let node_info = d.get("info").expect("Expected info");
        let mut hasher = Sha1::new();
        hasher.update(node_info.raw);
        let info_hash = hasher.finalize();

        let info_dict = match &node_info.value {
            bencode::BencodeValue::Dict(d) => d,
            _ => panic!("Expected dictionary")
        };

        let length = match info_dict.get("length") {
            Some(bencode::BencodeNode { value: bencode::BencodeValue::Integer(len), raw: _ }) => *len,
            _ => panic!("Torrent URL not found"),
        };

        let piece_length = match info_dict.get("piece length") {
            Some(bencode::BencodeNode { value: bencode::BencodeValue::Integer(len), raw: _ }) => *len,
            _ => panic!("Torrent URL not found"),
        };

        let pieces = match info_dict.get("pieces") {
            Some(bencode::BencodeNode { value: bencode::BencodeValue::Binary(_), raw }) => {
                let colon = (*raw).iter().position(|x| *x == b':').expect("Pieces are missing a colon");
                &raw[colon+1..]
            },
            _ => panic!("Torrent URL not found"),
        }.to_vec();

        Torrent { url, length, piece_length, pieces, info_hash }
    }

    pub async fn get_peers(&self, peer_id: &str) -> io::Result<Vec<(Ipv4Addr, u16)>> {
        if self.url.starts_with("http") {
            self.get_peers_by_http(peer_id).await
        } else if self.url.starts_with("udp") {
            self.get_peers_by_udp(peer_id)
        } else {
            Err(io::Error::new(io::ErrorKind::Other, format!("Unrecognized URL: {}", self.url)))
        }
    }

    fn get_udp_conn_id(socket: &UdpSocket, tid: u32) -> io::Result<u64> {
        let mut req = [0u8; 16];
        req[0..8].copy_from_slice(&0x41727101980u64.to_be_bytes().as_slice());
        req[8..12].copy_from_slice(&0u32.to_be_bytes().as_slice());
        req[12..16].copy_from_slice(&tid.to_be_bytes().as_slice());

        for _ in 0..5 {
            socket.send(req.as_slice())?;

            let mut res = [0u8; 16];
            match socket.recv(&mut res) {
                Ok(bytes) if bytes == 16 => {
                    if u32::from_be_bytes(res[0..4].try_into().unwrap()) == 0 &&
                        u32::from_be_bytes(res[4..8].try_into().unwrap()) == tid
                    {
                        return Ok(u64::from_be_bytes(res[8..16].try_into().unwrap()));
                    } else {
                        continue
                    }
                },
                Ok(_) => continue,
                Err(e) => {
                    if e.kind() == TimedOut || e.kind() == WouldBlock {
                        continue;
                    } else {
                        return Err(e)
                    }
                }
            }
        }
        Err(io::Error::new(TimedOut, "Tracker unreachable during conn_id fetching"))
    }

    fn get_udp_peers(&self, socket: &UdpSocket, conn_id: u64, peer_id: &str) -> io::Result<Vec<(Ipv4Addr, u16)>> {
        let mut peers_list = Vec::new();
        let tid = rand::random::<u32>();
        let key = rand::random::<u32>();
        let mut msg = [0u8; 98];

        msg[0..8].copy_from_slice(&conn_id.to_be_bytes());
        msg[8..12].copy_from_slice(&1u32.to_be_bytes());
        msg[12..16].copy_from_slice(&tid.to_be_bytes());
        msg[16..36].copy_from_slice(&self.info_hash.as_slice());
        msg[36..56].copy_from_slice(peer_id.as_bytes());
        msg[56..64].copy_from_slice(&0u64.to_be_bytes());
        msg[64..72].copy_from_slice(&(self.length as u64).to_be_bytes());
        msg[72..80].copy_from_slice(&0u64.to_be_bytes());
        msg[80..84].copy_from_slice(&2u32.to_be_bytes());
        msg[84..88].copy_from_slice(&0u32.to_be_bytes());
        msg[88..92].copy_from_slice(&key.to_be_bytes());
        msg[92..96].copy_from_slice(&(-1i32).to_be_bytes());
        msg[96..98].copy_from_slice(&6881u16.to_be_bytes());

        for _ in 0..5 {
            socket.send(msg.as_slice())?;

            let mut res = [0u8; 1024];
            match socket.recv(&mut res) {
                Ok(bytes) => {
                    let action = u32::from_be_bytes(res[0..4].try_into().unwrap());
                    let rtid = u32::from_be_bytes(res[4..8].try_into().unwrap());
                    if action == 1 && rtid == tid {
                        let _ = &res[20..bytes].chunks_exact(6).for_each(|c| {
                            peers_list.push((Ipv4Addr::new(c[0], c[1], c[2], c[3]), u16::from_be_bytes(c[4..6].try_into().unwrap())))
                        });
                        return Ok(peers_list)
                    } else if action == 3 && rtid == tid {
                        return Err(io::Error::new(InvalidData, "Tracker returned action 3"))
                    } else {
                        continue
                    }
                },
                Err(e) => {
                    if e.kind() == TimedOut || e.kind() == WouldBlock {
                        continue;
                    } else {
                        return Err(e)
                    }
                }
            }
        }
        Err(io::Error::new(TimedOut, "Tracker unreachable during peer fetching"))
    }

    fn get_peers_by_udp(&self, peer_id: &str) -> io::Result<Vec<(Ipv4Addr, u16)>> {
        let tid = rand::random::<u32>();
        let addr = self.url.strip_prefix("udp://")
            .and_then(|u| u.split("/").next())
            .unwrap_or(&self.url);
        if let Ok(socket) = UdpSocket::bind("0.0.0.0:0") {
            socket.set_read_timeout(Some(Duration::from_secs(5)))?;
            socket.connect(&addr)?;
            let conn_id = Self::get_udp_conn_id(&socket, tid)?;
            return self.get_udp_peers(&socket, conn_id, peer_id)
        }
        Ok(vec![])
    }

    async fn get_peers_by_http(&self, peer_id: &str) -> io::Result<Vec<(Ipv4Addr, u16)>> {
        let mut peers_list = Vec::new();
        let info_hash = self.info_hash.iter().map(|b| format!("%{:02x}", b)).collect::<String>();
        let body = reqwest::get(
            format!("{}?info_hash={info_hash}&peer_id={peer_id}&port=6881&uploaded=0&downloaded=0&left={}&compact=1", self.url, self.length)
        ).await.map_err(|e| {
            io::Error::new(io::ErrorKind::Other, e.to_string())
        })?;
        body.bytes().await.map(|b| {
            let (node, _) = bencode::decode_bencoded_value(b.iter().as_slice());
            if let bencode::BencodeValue::Dict(d) = node.value {
                if let Some(bencode::BencodeNode { value: bencode::BencodeValue::Binary(peers), raw: _ }) = d.get("peers") {
                    peers.chunks(6).for_each(|c| {
                        peers_list.push((Ipv4Addr::new(c[0], c[1], c[2], c[3]), ((c[4] as u16) << 8) | c[5] as u16));
                    });
                }
            }
            peers_list
        }).map_err(|e| {
            io::Error::new(io::ErrorKind::Other, e.to_string())
        })
    }

    pub async fn send_handshake(&self, stream: &mut TcpStream, peer_id: &str) -> io::Result<[u8; 68]> {
        let mut handshake: Vec<u8> = Vec::with_capacity(68);
        handshake.push(19);
        handshake.extend_from_slice(b"BitTorrent protocol");
        handshake.extend_from_slice(&[0u8; 8]);
        handshake.extend_from_slice(self.info_hash.as_slice());
        handshake.extend_from_slice(peer_id.as_bytes());
        stream.write_all(&handshake).await?;
        let mut response = [0u8; 68];
        stream.read_exact(&mut response).await?;
        Ok(response)
    }

    pub fn count_pieces(&self) -> u32 {
        (self.length as f64 / self.piece_length as f64).ceil() as u32
    }

    pub fn calc_piece_length(&self, piece: u32) -> u32 {
        let pieces = self.count_pieces();
        if piece <  pieces - 1 {
            self.piece_length as u32
        } else {
            self.length as u32 - self.piece_length as u32 * (pieces - 1)
        }
    }

}