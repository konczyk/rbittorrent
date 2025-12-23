use clap::{Parser, ValueEnum};
use serde_json::Value;
use sha1::{Digest, Sha1};
use std::collections::BTreeMap;
use std::iter::from_fn;
use std::{fs, io};

#[derive(Debug)]
enum BencodeValue<'a> {
    String(String),
    Binary(Vec<u8>),
    Integer(isize),
    List(Vec<(BencodeValue<'a>, &'a [u8])>),
    Dict(BTreeMap<String, (BencodeValue<'a>, &'a [u8])>),
}

impl BencodeValue<'_> {
    fn to_json(self) -> Value {
        match self {
            BencodeValue::String(s) => s.into(),
            BencodeValue::Integer(n) => n.into(),
            BencodeValue::Binary(b) => hex::encode(b).into(),
            BencodeValue::List(l) => l.into_iter().map(|(x, _)| x.to_json()).collect::<Vec<Value>>().into(),
            BencodeValue::Dict(d) => Value::Object(d.into_iter().map(|(k, (v, _))| (k, v.to_json())).collect::<serde_json::Map<String, Value>>())
        }
    }
}

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

fn decode_bencoded_value(encoded_value: &[u8]) -> Value {
    to_bencode_value(encoded_value).0.to_json()
}

fn to_bencode_value(encoded_value: &[u8]) -> (BencodeValue<'_>, &[u8], &[u8]) {
    let (bv, rest) = match encoded_value[0] {
        b'l' => {
            let mut rest = &encoded_value[1..];
            let result = from_fn(|| {
                if rest.is_empty() || rest[0] == b'e' {
                    None
                } else {
                    let (val, reminder, raw) = to_bencode_value(rest);
                    rest = reminder;
                    Some((val, raw)) }
            }).collect::<Vec<(BencodeValue, &[u8])>>();

            (BencodeValue::List(result), &rest[1..])
        },
        b'd' => {
            let mut rest = &encoded_value[1..];
            let result = from_fn(|| {
                if rest.is_empty() || rest[0] == b'e' {
                    None
                } else {
                    let (key, reminder, _) = to_bencode_value(rest);
                    let (val, reminder, raw) = to_bencode_value(reminder);
                    rest = reminder;
                    if let BencodeValue::String(k) = key {
                        Some((k, (val, raw)))
                    } else {
                        None
                    }
                }
            }).collect::<BTreeMap<String, (BencodeValue, &[u8])>>();

            (BencodeValue::Dict(result), &rest[1..])
        },
        b'i' => {
            encoded_value
                .iter()
                .position(|x| *x == b'e')
                .and_then(|pos| {
                    str::from_utf8(&encoded_value[1..pos])
                        .ok()
                        .and_then(|v| v.parse::<isize>().ok())
                        .map(|n| (BencodeValue::Integer(n), &encoded_value[pos+1..]))
                })
                .expect("Invalid bencoded integer")
        },
        b'0' .. b'9' => {
            encoded_value
                .iter()
                .position(|x| *x == b':')
                .and_then(|pos| {
                    let (len, rest) = encoded_value.split_at(pos);
                    str::from_utf8(len).ok().and_then(|n| {
                        n.parse::<usize>()
                            .ok()
                            .map(|x| (x, &rest[1..]))
                    })
                })
                .and_then(|(n, s)| {
                    match str::from_utf8(&s[..n]) {
                        Ok(v) => Some((BencodeValue::String(v.to_string()), &s[n..])),
                        Err(_) => Some((BencodeValue::Binary(s[..n].to_vec()), &s[n..]))
                    }
                })
                .expect("Invalid bencoded string")
        },
        _ => panic!("Unhandled encoded value: {:?}", encoded_value),
    };

    let consumed = encoded_value.len() - rest.len();
    (bv, rest, &encoded_value[..consumed])
}

fn main() -> io::Result<()> {
    let args = Args::parse();

    match args.command {
        Command::Decode => {
            println!("{}", decode_bencoded_value(args.value.as_bytes()));
            Ok(())
        },
        Command::Info => {
            fs::read(args.value)
                .and_then(|cnt| {
                    let (val, _, _) = to_bencode_value(cnt.as_slice());
                    if let BencodeValue::Dict(d) = val {
                        if let Some((BencodeValue::String(url), _)) = d.get("announce") {
                            println!("Tracker URL: {url}");
                        }
                        if let Some((info, r)) = d.get("info") {
                            if let BencodeValue::Dict(i) = info {
                                if let Some((BencodeValue::Integer(length), _)) = i.get("length") {
                                    println!("Length: {length}");
                                }
                            }
                            let mut hasher = Sha1::new();
                            hasher.update(r);
                            println!("Info Hash: {:x}", hasher.finalize());
                        }
                    }
                    Ok(())
                })
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn decode_strings() {
        assert_eq!(decode_bencoded_value("3:abc".as_bytes()), json!("abc"));
    }

    #[test]
    fn decode_integers() {
        assert_eq!(decode_bencoded_value("i467e".as_bytes()), json!(467));
        assert_eq!(decode_bencoded_value("i-467e".as_bytes()), json!(-467));
    }

    #[test]
    fn decode_lists() {
        assert_eq!(decode_bencoded_value("l3:abci467ee".as_bytes()), Value::Array(vec![json!("abc"), json!(467)]));
        assert_eq!(decode_bencoded_value("l3:abcli12eei467ee".as_bytes()), Value::Array(vec![json!("abc"), json!(vec![json!(12)]), json!(467)]));
    }

    #[test]
    fn decode_dicts() {
        assert_eq!(decode_bencoded_value("d3:abci467ee".as_bytes()), serde_json::to_value(&BTreeMap::from([("abc", json!(467))])).unwrap());
    }

}
