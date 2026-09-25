pub fn normalize_hex(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.starts_with("0x") || trimmed.starts_with("0X") {
        format!("0x{}", trimmed[2..].to_ascii_lowercase())
    } else {
        format!("0x{}", trimmed.to_ascii_lowercase())
    }
}

pub fn is_address(value: &str) -> bool {
    let normalized = normalize_hex(value);
    normalized.len() == 42 && hex::decode(&normalized[2..]).is_ok()
}

pub fn is_tx_hash(value: &str) -> bool {
    let normalized = normalize_hex(value);
    normalized.len() == 66 && hex::decode(&normalized[2..]).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_hex_lowercases() {
        assert_eq!(
            normalize_hex("0xABCDef0000000000000000000000000000000001"),
            "0xabcdef0000000000000000000000000000000001"
        );
        assert!(is_address("0x0000000000000000000000000000000000000001"));
        assert!(!is_address("0x123"));
        assert!(is_tx_hash(
            "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        ));
    }
}
