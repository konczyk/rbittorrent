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
    if let Some((n, s)) = encoded_value
        .split_once(':')
        .and_then(|(n, s)| n.parse::<usize>().ok().map(|x| (x, s)))
    {
        serde_json::Value::String(s[..n].to_string())
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
}
