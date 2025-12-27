mod bencode;

use clap::{Parser, ValueEnum};
use sha1::{Digest, Sha1};
use std::{fs, io};
use bencode::{decode_bencoded_value, BencodeNode, BencodeValue};

#[derive(Debug, Clone, ValueEnum)]
#[value(rename_all = "lowercase")]
enum Command {
    Decode,
    Info,
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
                        if let Some(BencodeNode { value: BencodeValue::String(url), raw: _}) = d.get("announce") {
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
    }
}