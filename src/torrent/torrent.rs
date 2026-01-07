use crate::torrent::bencode;
use reqwest::blocking;
use sha1::digest::core_api::CoreWrapper;
use sha1::digest::Output;
use sha1::{Digest, Sha1, Sha1Core};
use std::io;
use std::io::{Read, Write};
use std::net::{Ipv4Addr, TcpStream};

pub struct Torrent<'a> {
    pub url: String,
    pub length: isize,
    pub piece_length: isize,
    pub pieces: &'a [u8],
    pub info_hash: Output<CoreWrapper<Sha1Core>>,
}

impl<'a> Torrent<'a> {
    pub fn new(data: &'a [u8]) -> Torrent<'a> {
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
        };

        Torrent { url, length, piece_length, pieces, info_hash }
    }

    pub fn get_peers(&self, peer_id: &str) -> io::Result<Vec<(Ipv4Addr, u16)>> {
        let mut peers_list = Vec::new();
        let info_hash = self.info_hash.iter().map(|b| format!("%{:02x}", b)).collect::<String>();
        let body = blocking::get(
            format!("{}?info_hash={info_hash}&peer_id={peer_id}&port=6881&uploaded=0&downloaded=0&left={}&compact=1", self.url, self.piece_length)
        ).and_then(|result| result.bytes());
        body.map(|b| {
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
            eprintln!("{e}");
            io::Error::new(io::ErrorKind::Other, e.to_string())
        })
    }

    pub fn send_handshake(&self, stream: &mut TcpStream, peer_id: &str) -> io::Result<[u8; 68]> {
        let mut handshake: Vec<u8> = Vec::with_capacity(68);
        handshake.push(19);
        handshake.extend_from_slice(b"BitTorrent protocol");
        handshake.extend_from_slice(&[0u8; 8]);
        handshake.extend_from_slice(self.info_hash.as_slice());
        handshake.extend_from_slice(peer_id.as_bytes());
        stream.write_all(&handshake)?;
        let mut response = [0u8; 68];
        stream.read_exact(&mut response)?;
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