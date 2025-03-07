pub fn sanitize_ticker(ticker: String) -> String {
    return ticker
        .chars()
        .take(20)
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .collect::<String>()
        .to_lowercase();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_ticker_pass_no_harm() {
        let result = sanitize_ticker("123".to_string());
        assert_eq!(result, "123".to_string());
    }

    #[test]
    fn sanitize_ticker_pass_delimiters() {
        let result = sanitize_ticker("123-_".to_string());
        assert_eq!(result, "123-_".to_string());
    }

    #[test]
    fn sanitize_ticker_pass_remove_non_alnum() {
        let result = sanitize_ticker("123*&(^(*&123..,./.,/".to_string());
        assert_eq!(result, "123123".to_string());
    }

    #[test]
    fn sanitize_ticker_pass_max_len() {
        let result = sanitize_ticker("123123123123123123123".to_string());
        assert_eq!(result, "12312312312312312312".to_string());
    }

    #[test]
    fn sanitize_ticker_pass_to_lowercase() {
        let result = sanitize_ticker("AaAa".to_string());
        assert_eq!(result, "aaaa".to_string());
    }

    #[test]
    fn sanitize_ticker_fail_to_lowercase() {
        let result = sanitize_ticker("AaAa".to_string());
        assert_ne!(result, "AaAa".to_string());
    }
}
