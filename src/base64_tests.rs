//! Tests de base64.

use crate::base64::{decode, encode};

#[test]
fn rfc4648_vectors() {
    for (clair, code) in [("", ""), ("f", "Zg=="), ("fo", "Zm8="), ("foo", "Zm9v"), ("foob", "Zm9vYg=="), ("fooba", "Zm9vYmE="), ("foobar", "Zm9vYmFy")] {
        assert_eq!(encode(clair.as_bytes()), code);
        assert_eq!(decode(code).unwrap(), clair.as_bytes());
    }
}

#[test]
fn every_byte_value_round_trips_and_padding_is_optional() {
    let tout: Vec<u8> = (0..=255).collect();
    assert_eq!(decode(&encode(&tout)).unwrap(), tout);
    assert_eq!(decode("Zm9vYg").unwrap(), b"foob", "sans « = »");
    assert_eq!(decode("Zm9v\r\nYg==").unwrap(), b"foob", "les retours à la ligne sont ignorés");
    assert!(decode("Zm9v!").is_none(), "un caractère qui n'est pas du base64");
}
