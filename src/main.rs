use clap::{Parser, ValueEnum};
use serde_json::Value;
use std::collections::HashMap;
use std::iter::from_fn;
use std::io;

#[derive(Debug, Clone, ValueEnum)]
#[value(rename_all = "lowercase")]
enum Command {
    Decode,
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

fn decode_bencoded_value(encoded_value: &[u8]) -> (Value, &[u8]) {
    match encoded_value[0] {
        b'l' => {
            let mut rest = &encoded_value[1..];
            let result = from_fn(|| {
                if rest.is_empty() || rest[0] == b'e' {
                    None
                } else {
                    let (val, reminder) = decode_bencoded_value(rest);
                    rest = reminder;
                    Some(val)
                }
            }).collect::<Vec<Value>>();

            (result.into(), &rest[1..])
        },
        b'd' => {
            let mut rest = &encoded_value[1..];
            let result = from_fn(|| {
                if rest.is_empty() || rest[0] == b'e' {
                    None
                } else {
                    let (key, reminder) = decode_bencoded_value(rest);
                    let (val, reminder) = decode_bencoded_value(reminder);
                    rest = reminder;
                    Some((key, val))
                }
            }).collect::<HashMap<Value, Value>>();

            (serde_json::to_value(result).unwrap(), &rest[1..])
        },
        b'i' => {
            encoded_value
                .iter()
                .position(|x| *x == b'e')
                .and_then(|pos| {
                    str::from_utf8(&encoded_value[1..pos])
                        .ok()
                        .and_then(|v| v.parse::<isize>().ok())
                        .map(|n| (n.into(), &encoded_value[pos+1..]))
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
                    str::from_utf8(&s[..n])
                        .map(|v| (v.into(), &s[n..]))
                        .ok()
                })
                .expect("Invalid bencoded string")
        },
        _ => panic!("Unhandled encoded value: {:?}", encoded_value),
    }
}

fn main() -> io::Result<()> {
    let args = Args::parse();

    match args.command {
        Command::Decode => {
            println!("{}", decode_bencoded_value(args.value.as_bytes()).0);
            Ok(())
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashMap;

    #[test]
    fn decode_strings() {
        assert_eq!(decode_bencoded_value("3:abc".as_bytes()), ("abc".into(), "".as_bytes()));
    }

    #[test]
    fn decode_integers() {
        assert_eq!(decode_bencoded_value("i467e".as_bytes()), (467.into(), "".as_bytes()));
        assert_eq!(decode_bencoded_value("i-467e".as_bytes()), ((-467).into(), "".as_bytes()));
    }

    #[test]
    fn decode_lists() {
        assert_eq!(decode_bencoded_value("l3:abci467ee".as_bytes()), (vec![json!("abc"), json!(467)].into(), "".as_bytes()));
        assert_eq!(decode_bencoded_value("l3:abcli12eei467ee".as_bytes()), (vec![json!("abc"), json!(vec![json!(12)]), json!(467)].into(), "".as_bytes()));
    }

    #[test]
    fn decode_dicts() {
        assert_eq!(decode_bencoded_value("d3:abci467ee".as_bytes()), (serde_json::to_value(&HashMap::from([(json!("abc"), json!(467))])).unwrap(), "".as_bytes()));
    }
}
