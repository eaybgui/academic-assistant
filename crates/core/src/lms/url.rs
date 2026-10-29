//! Platform URL normalization (spec §5.3).

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BaseUrlError {
    #[error("Enter the platform URL")]
    Empty,
    #[error("The platform URL must start with https://")]
    NotHttps,
    #[error("Enter a valid platform URL")]
    Invalid,
}

/// Reduces user input to an `https` origin (`https://host[:port]`, lowercase host,
/// no path, query, fragment or trailing slash). Input without a scheme gets `https://`.
pub fn normalize_base_url(input: &str) -> Result<String, BaseUrlError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(BaseUrlError::Empty);
    }
    let rest = match trimmed.split_once("://") {
        Some((scheme, rest)) if scheme.eq_ignore_ascii_case("https") => rest,
        Some(_) => return Err(BaseUrlError::NotHttps),
        None => trimmed,
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host = authority.split(':').next().unwrap_or_default();
    let valid = !authority.is_empty()
        && !authority.contains('@')
        && !authority.contains(char::is_whitespace)
        && host.contains('.')
        && !host.starts_with('.')
        && !host.ends_with('.');
    if !valid {
        return Err(BaseUrlError::Invalid);
    }
    Ok(format!("https://{}", authority.to_ascii_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_only_the_https_origin() {
        assert_eq!(
            normalize_base_url("aulavirtual.uv.es").unwrap(),
            "https://aulavirtual.uv.es"
        );
        assert_eq!(
            normalize_base_url(" https://aulavirtual.uv.es/my/courses.php?x=1#top ").unwrap(),
            "https://aulavirtual.uv.es"
        );
        assert_eq!(
            normalize_base_url("HTTPS://UAH.Blackboard.com/ultra/stream").unwrap(),
            "https://uah.blackboard.com"
        );
        assert_eq!(
            normalize_base_url("https://host.example:8443/x").unwrap(),
            "https://host.example:8443"
        );
    }

    #[test]
    fn rejects_empty_non_https_and_malformed_urls() {
        assert_eq!(normalize_base_url("   "), Err(BaseUrlError::Empty));
        assert_eq!(
            normalize_base_url("http://aulavirtual.uv.es"),
            Err(BaseUrlError::NotHttps)
        );
        assert_eq!(
            normalize_base_url("ftp://x.es"),
            Err(BaseUrlError::NotHttps)
        );
        assert_eq!(normalize_base_url("https://"), Err(BaseUrlError::Invalid));
        assert_eq!(
            normalize_base_url("https://user@host.es"),
            Err(BaseUrlError::Invalid)
        );
        assert_eq!(
            normalize_base_url("https://ex ample.es"),
            Err(BaseUrlError::Invalid)
        );
        assert_eq!(
            normalize_base_url("https://nodot"),
            Err(BaseUrlError::Invalid)
        );
    }

    #[test]
    fn error_messages_are_user_facing() {
        assert_eq!(BaseUrlError::Empty.to_string(), "Enter the platform URL");
        assert_eq!(
            BaseUrlError::NotHttps.to_string(),
            "The platform URL must start with https://"
        );
        assert_eq!(
            BaseUrlError::Invalid.to_string(),
            "Enter a valid platform URL"
        );
    }
}
