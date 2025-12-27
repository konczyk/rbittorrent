use std::collections::BTreeMap;
use std::iter::from_fn;
use serde_json::Value;

#[derive(Debug, PartialEq)]
pub struct BencodeNode<'a> {
    pub value: BencodeValue<'a>,
    pub raw: &'a [u8]
}

impl BencodeNode<'_> {
    pub fn to_json(self) -> Value {
        self.value.to_json()
    }
}

#[derive(Debug, PartialEq)]
pub enum BencodeValue<'a> {
    String(String),
    Binary(Vec<u8>),
    Integer(isize),
    List(Vec<BencodeNode<'a>>),
    Dict(BTreeMap<String, BencodeNode<'a>>),
}

impl BencodeValue<'_> {
    pub fn to_json(self) -> Value {
        match self {
            BencodeValue::String(s) => s.into(),
            BencodeValue::Integer(n) => n.into(),
            BencodeValue::Binary(b) => hex::encode(b).into(),
            BencodeValue::List(l) => l.into_iter().map(|x| x.to_json()).collect::<Vec<Value>>().into(),
            BencodeValue::Dict(d) => Value::Object(d.into_iter().map(|(k, v)| (k, v.to_json())).collect::<serde_json::Map<String, Value>>())
        }
    }
}

pub fn decode_bencoded_value(encoded_value: &[u8]) -> (BencodeNode<'_>, &[u8]) {
    let (bv, rest) = match encoded_value[0] {
        b'l' => {
            let mut rest = &encoded_value[1..];
            let result = from_fn(|| {
                if rest.is_empty() || rest[0] == b'e' {
                    None
                } else {
                    let (node, reminder) = decode_bencoded_value(rest);
                    rest = reminder;
                    Some(node) }
            }).collect::<Vec<BencodeNode>>();

            (BencodeValue::List(result), &rest[1..])
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
                    if let BencodeValue::String(k) = key.value {
                        Some((k, val))
                    } else {
                        None
                    }
                }
            }).collect::<BTreeMap<String, BencodeNode>>();

            (BencodeValue::Dict(result), &rest[1..])
        },
        b'i' => {
            encoded_value
                .iter()
                .position(|&x| x == b'e')
                .and_then(|pos| {
                    str::from_utf8(&encoded_value[1..pos])
                        .ok()
                        .and_then(|v| v.parse::<isize>().ok())
                        .map(|n| (BencodeValue::Integer(n), &encoded_value[pos+1..]))
                })
                .expect("Invalid bencoded integer")
        },
        b'0' ..= b'9' => {
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
    (BencodeNode { value: bv, raw: &encoded_value[..consumed]}, rest)
}



#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use serde_json::json;

    #[test]
    fn handle_strings() {
        assert_eq!(
            decode_bencoded_value("3:abc".as_bytes()),
            (BencodeNode { value: BencodeValue::String("abc".to_string()), raw: "3:abc".as_bytes() }, "".as_bytes())
        );
        assert_eq!(BencodeValue::String("abc".to_string()).to_json(), json!("abc"))
    }

    #[test]
    fn handle_integers() {
        assert_eq!(
            decode_bencoded_value("i467e".as_bytes()),
            (BencodeNode { value: BencodeValue::Integer(467), raw: "i467e".as_bytes()}, "".as_bytes())
        );
        assert_eq!(
            decode_bencoded_value("i-467e".as_bytes()),
            (BencodeNode { value: BencodeValue::Integer(-467), raw: "i-467e".as_bytes()}, "".as_bytes())
        );
        assert_eq!(BencodeValue::Integer(467).to_json(), json!(467));
        assert_eq!(BencodeValue::Integer(-467).to_json(), json!(-467));
    }

    #[test]
    fn decode_lists() {
        assert_eq!(
            decode_bencoded_value("l3:abci467ee".as_bytes()),
            (BencodeNode {
                value: BencodeValue::List(vec![
                    (BencodeNode { value: BencodeValue::String("abc".to_string()), raw: "3:abc".as_bytes()}),
                    (BencodeNode { value: BencodeValue::Integer(467), raw: "i467e".as_bytes()})
                ]),
                raw: "l3:abci467ee".as_bytes()
            },
            "".as_bytes(),
            )
        );
    }

    #[test]
    fn decode_dicts() {
        assert_eq!(
            decode_bencoded_value("d3:abci467ee".as_bytes()),
            (BencodeNode {
                value: BencodeValue::Dict(BTreeMap::from([
                    ("abc".to_string(), (BencodeNode { value: BencodeValue::Integer(467), raw: "i467e".as_bytes() }))
                ])),
                raw: "d3:abci467ee".as_bytes()
            },
            "".as_bytes(),
            )
        );
    }

}
