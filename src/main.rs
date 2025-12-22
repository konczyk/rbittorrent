use std::collections::HashMap;
use std::iter::from_fn;
use clap::{Parser, ValueEnum};
use serde_json::Value;

#[derive(Debug, Clone, ValueEnum)]
#[value(rename_all = "lowercase")]
enum Command {
    Decode
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

fn decode_bencoded_value(encoded_value: &str) -> (Value, &str) {
    if let Some(mut rest) = encoded_value.strip_prefix('l') {
        let result = from_fn(|| {
            if rest.starts_with('e') {
                None
            } else {
                let (val, reminder) = decode_bencoded_value(rest);
                rest = reminder;
                Some(val)
            }
        }).collect::<Vec<Value>>();

        (result.into(), &rest[1..])
    } else if let Some(mut rest) = encoded_value.strip_prefix('d') {
        let result = from_fn(|| {
            if rest.starts_with('e') {
                None
            } else {
                let (key, reminder) = decode_bencoded_value(rest);
                let (val, reminder) = decode_bencoded_value(reminder);
                rest = reminder;
                Some((key, val))
            }
        }).collect::<HashMap<Value, Value>>();

        (serde_json::to_value(result).unwrap(), &rest[1..])
    } else if let Some((n, rest)) = encoded_value
        .strip_prefix('i')
        .and_then(|s| s.split_once('e')
        .and_then(|(n, rest)| n.parse::<isize>()
            .ok()
            .map(|x| (x, rest))))
    {
        (n.into(), rest)
    } else if let Some((n, s)) = encoded_value
        .split_once(':')
        .and_then(|(n, s)| n.parse::<usize>()
            .ok()
            .map(|x| (x, s)))
    {
        (s[..n].into(), &s[n..])
    } else {
        panic!("Unhandled encoded value: {}", encoded_value)
    }
}

fn main() {
    let args = Args::parse();

    match args.command {
        Command::Decode => {
            println!("{}", decode_bencoded_value(args.value.as_str()).0)
        },
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use serde_json::json;
    use super::*;

    #[test]
    fn decode_strings() {
        assert_eq!(decode_bencoded_value("3:abc"), ("abc".into(), ""));
    }

    #[test]
    fn decode_integers() {
        assert_eq!(decode_bencoded_value("i467e"), (467.into(), ""));
        assert_eq!(decode_bencoded_value("i-467e"), ((-467).into(), ""));
    }

    #[test]
    fn decode_lists() {
        assert_eq!(decode_bencoded_value("l3:abci467ee"), (vec![json!("abc"), json!(467)].into(), ""));
        assert_eq!(decode_bencoded_value("l3:abcli12eei467ee"), (vec![json!("abc"), json!(vec![json!(12)]), json!(467)].into(), ""));
    }

    #[test]
    fn decode_dicts() {
        assert_eq!(decode_bencoded_value("d3:abci467ee"), (serde_json::to_value(&HashMap::from([(json!("abc"), json!(467))])).unwrap(), ""));
    }
}
