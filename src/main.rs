use clap::{Parser, ValueEnum};

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


fn decode_bencoded_value(encoded_value: &str) -> serde_json::Value {
    if let Some(n) = encoded_value
        .strip_prefix('i')
        .and_then(|s| s.strip_suffix('e')
        .and_then(|n| n.parse::<isize>().ok()))
    {
        n.into()
    } else if let Some((n, s)) = encoded_value
        .split_once(':')
        .and_then(|(n, s)| n.parse::<usize>().ok().map(|x| (x, s)))
    {
        s[..n].into()
    } else {
        panic!("Unhandled encoded value: {}", encoded_value)
    }
}

fn main() {
    let args = Args::parse();

    match args.command {
        Command::Decode => {
            println!("{}", decode_bencoded_value(args.value.as_str()))
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_strings() {
        assert_eq!(decode_bencoded_value("3:abc"), "abc");
    }

    #[test]
    fn decode_integers() {
        assert_eq!(decode_bencoded_value("i467e"), 467);
        assert_eq!(decode_bencoded_value("i-467e"), -467);
    }
}
