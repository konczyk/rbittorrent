mod bencode;

use bencode::{decode_bencoded_value, BencodeNode, BencodeValue};
use clap::{Parser, ValueEnum};
use reqwest::blocking;
use sha1::{Digest, Sha1};
use std::{fs, io};
use std::net::Ipv4Addr;

#[derive(Debug, Clone, ValueEnum)]
#[value(rename_all = "lowercase")]
enum Command {
    Decode,
    Info,
    Peers,
}

#[derive(Parser, Debug)]
struct Args {
    /// Name of the command
    #[arg(
        value_name = "COMMAND",
        required = true,
    )]
    command: Command,

    /// Value to process
    #[arg(
        value_name = "VALUE",
        required = true,
    )]
    value: String,
}

fn main() -> io::Result<()> {
    let args = Args::parse();

    match args.command {
        Command::Decode => {
            println!("{}", decode_bencoded_value(args.value.as_bytes()).0.to_json());
            Ok(())
        },
        Command::Info => {
            fs::read(args.value)
                .and_then(|cnt| {
                    let (node, _) = decode_bencoded_value(cnt.as_slice());
                    if let BencodeValue::Dict(d) = node.value {
                        if let Some(BencodeNode { value: BencodeValue::String(url), raw: _ }) = d.get("announce") {
                            println!("Tracker URL: {url}");
                        }
                        if let Some(node) = d.get("info") {
                            if let BencodeValue::Dict(i) = &node.value {
                                if let Some(BencodeNode { value: BencodeValue::Integer(length), raw: _ }) = i.get("length") {
                                    println!("Length: {length}");
                                }
                                let mut hasher = Sha1::new();
                                hasher.update(node.raw);
                                println!("Info Hash: {:x}", hasher.finalize());
                                if let Some(BencodeNode { value: BencodeValue::Integer(length), raw: _ }) = i.get("piece length") {
                                    println!("Piece Length: {length}");
                                }
                                if let Some(BencodeNode { value: BencodeValue::Binary(pieces), raw: _ }) = i.get("pieces") {
                                    println!("Piece Hashes:");
                                    pieces.chunks(20).map(|x| hex::encode(x)).for_each(|x| println!("{x}"));
                                }
                            }
                        }
                    }
                    Ok(())
                })
        },
        Command::Peers => {
            fs::read(args.value)
                .and_then(|cnt| {
                    let (node, _) = decode_bencoded_value(cnt.as_slice());
                    if let BencodeValue::Dict(d) = node.value {
                        if let Some(BencodeNode { value: BencodeValue::String(url), raw: _ }) = d.get("announce") {
                            if let Some(node) = d.get("info") {
                                if let BencodeValue::Dict(i) = &node.value {
                                    let mut hasher = Sha1::new();
                                    hasher.update(node.raw);
                                    let info_hash = hasher.finalize().iter().map(|b| format!("%{:02x}", b)).collect::<String>();
                                    let peer_id = "xwgeweorwehnrot34t29";
                                    if let Some(BencodeNode { value: BencodeValue::Integer(length), raw: _ }) = i.get("piece length") {
                                        let body = blocking::get(
                                            format!("{url}?info_hash={info_hash}&peer_id={peer_id}&port=6881&uploaded=0&downloaded=0&left={length}&compact=1")
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
                                    }
                                }
                            }
                        }
                    }
                    Ok(())
                })
        }
    }
}