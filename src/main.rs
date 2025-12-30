mod bencode;
mod torrent;

use bencode::{decode_bencoded_value, BencodeNode, BencodeValue};
use clap::{Parser, ValueEnum};
use reqwest::blocking;
use std::net::{Ipv4Addr, TcpStream};
use std::{fs, io};
use std::io::{Read, Write};
use torrent::Torrent;

#[derive(Debug, Clone, ValueEnum)]
#[value(rename_all = "lowercase")]
enum Command {
    Decode,
    Info,
    Peers,
    Handshake,
}

#[derive(Parser, Debug)]
struct Args {
    /// Name of the command
    #[arg(
        value_name = "COMMAND",
        required = true,
    )]
    command: Command,

    /// Value to decode or a torrent file
    #[arg(
        value_name = "VALUE",
        required = true,
    )]
    value: String,

    /// peer
    #[arg(
        value_name = "PEER",
        required = false,
    )]
    peer: Option<String>,
}

fn main() -> io::Result<()> {
    let args = Args::parse();
    let peer_id = "xwgeweorwehnrot34t29";

    match args.command {
        Command::Decode => {
            println!("{}", decode_bencoded_value(args.value.as_bytes()).0.to_json());
            Ok(())
        },
        Command::Info => {
            fs::read(args.value)
                .and_then(|cnt| {
                    let torrent = Torrent::new(cnt.as_slice());
                    println!("Tracker URL: {}", torrent.url);
                    println!("Length: {}", torrent.length);
                    println!("Info Hash: {:x}", torrent.info_hash);
                    println!("Piece Length: {}", torrent.piece_length);
                    println!("Piece Hashes:");
                    torrent.pieces.chunks(20).map(|x| hex::encode(x)).for_each(|x| println!("{x}"));
                    Ok(())
                })
        },
        Command::Peers => {
            fs::read(args.value)
                .and_then(|cnt| {
                    let torrent = Torrent::new(cnt.as_slice());
                    let info_hash = torrent.info_hash.iter().map(|b| format!("%{:02x}", b)).collect::<String>();
                    let body = blocking::get(
                        format!("{}?info_hash={info_hash}&peer_id={peer_id}&port=6881&uploaded=0&downloaded=0&left={}&compact=1", torrent.url, torrent.piece_length)
                    ).and_then(|result| result.bytes());
                    match body {
                        Ok(b) => {
                            let (node, _) = decode_bencoded_value(b.iter().as_slice());
                            if let BencodeValue::Dict(d) = node.value {
                                if let Some(BencodeNode { value: BencodeValue::Binary(peers), raw: _ }) = d.get("peers") {
                                    peers.chunks(6).for_each(|c| {
                                        let ip = Ipv4Addr::new(c[0], c[1], c[2], c[3]);
                                        let port = ((c[4] as u16) << 8) | c[5] as u16;
                                        println!("{ip}:{port}")
                                    });
                                }
                            }
                        },
                        Err(e) => eprintln!("An error occured {}", e),
                    }
                    Ok(())
                })
        },
        Command::Handshake => {
            fs::read(args.value)
                .and_then(|cnt| {
                    let torrent = Torrent::new(cnt.as_slice());
                    match args.peer {
                        Some(peer) => {
                            if let Ok(mut stream) = TcpStream::connect(&peer) {
                                let mut handshake: Vec<u8> = Vec::with_capacity(68);
                                handshake.push(19);
                                handshake.extend_from_slice(b"BitTorrent protocol");
                                handshake.extend_from_slice(&[0u8; 8]);
                                handshake.extend_from_slice(torrent.info_hash.as_slice());
                                handshake.extend_from_slice(peer_id.as_bytes());
                                let _ = stream.write_all(&handshake);

                                let mut response = [0u8; 68];
                                let _ = stream.read_exact(&mut response);

                                println!("Peer ID: {}", hex::encode(&response[48..68]));
                            } else {
                                eprintln!("Couldn't connect to server...");
                            }
                        },
                        None => eprintln!("Missing peer param"),
                    }
                    Ok(())
                })
        }
    }
}