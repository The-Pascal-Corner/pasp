use std::path::Path;

pub fn parse_number(name: &str) -> Option<u32> {
    let stem = Path::new(name).file_stem()?.to_str()?;
    let digits: String = stem.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() { None } else { digits.parse().ok() }
}

pub fn remove_diacritics(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            'à' | 'á' | 'ả' | 'ã' | 'ạ' | 'â' | 'ă' | 'ầ' | 'ấ' | 'ẩ' | 'ẫ' | 'ậ' | 'ằ' | 'ắ' | 'ẳ' | 'ẵ' | 'ặ' => result.push('a'),
            'À' | 'Á' | 'Ả' | 'Ã' | 'Ạ' | 'Â' | 'Ă' | 'Ầ' | 'Ấ' | 'Ẩ' | 'Ẫ' | 'Ậ' | 'Ằ' | 'Ắ' | 'Ẳ' | 'Ẵ' | 'Ặ' => result.push('A'),
            'è' | 'é' | 'ẻ' | 'ẽ' | 'ẹ' | 'ê' | 'ề' | 'ế' | 'ể' | 'ễ' | 'ệ' => result.push('e'),
            'È' | 'É' | 'Ẻ' | 'Ẽ' | 'Ẹ' | 'Ê' | 'Ề' | 'Ế' | 'Ể' | 'Ễ' | 'Ệ' => result.push('E'),
            'ì' | 'í' | 'ỉ' | 'ĩ' | 'ị' => result.push('i'),
            'Ì' | 'Í' | 'Ỉ' | 'Ĩ' | 'Ị' => result.push('I'),
            'ò' | 'ó' | 'ỏ' | 'õ' | 'ọ' | 'ô' | 'ơ' | 'ồ' | 'ố' | 'ổ' | 'ỗ' | 'ộ' | 'ờ' | 'ớ' | 'ở' | 'ỡ' | 'ợ' => result.push('o'),
            'Ò' | 'Ó' | 'Ỏ' | 'Õ' | 'Ọ' | 'Ô' | 'Ơ' | 'Ồ' | 'Ố' | 'Ổ' | 'Ỗ' | 'Ộ' | 'Ờ' | 'Ớ' | 'Ở' | 'Ỡ' | 'Ợ' => result.push('O'),
            'ù' | 'ú' | 'ủ' | 'ũ' | 'ụ' | 'ư' | 'ừ' | 'ứ' | 'ử' | 'ữ' | 'ự' => result.push('u'),
            'Ù' | 'Ú' | 'Ủ' | 'Ũ' | 'Ụ' | 'Ư' | 'Ừ' | 'Ứ' | 'Ử' | 'Ữ' | 'Ự' => result.push('U'),
            'ỳ' | 'ý' | 'ỷ' | 'ỹ' | 'ỵ' => result.push('y'),
            'Ỳ' | 'Ý' | 'Ỷ' | 'Ỹ' | 'Ỵ' => result.push('Y'),
            'đ' => result.push('d'),
            'Đ' => result.push('D'),
            _ => result.push(ch),
        }
    }
    result
}

pub fn format_name(template: &str, num: u32, title: &str) -> String {
    template
        .replace("{n}", &num.to_string())
        .replace("{t}", title)
}

pub fn extract_title(name: &str, prefix: &str) -> Option<String> {
    let stem = Path::new(name).file_stem()?.to_str()?;
    if prefix.is_empty() {
        let cleaned: String = stem.chars().skip_while(|c| c.is_ascii_digit()).skip_while(|&c| c == '_' || c == ' ' || c == '-' || c == '.').collect();
        return Some(cleaned);
    }
    if let Some(pos) = stem.find(prefix) {
        let after = &stem[pos + prefix.len()..];
        let cleaned: String = after.trim_start_matches(|c: char| c == '_' || c == ' ' || c == '-' || c == '.').to_string();
        Some(cleaned)
    } else {
        Some(stem.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_number() {
        assert_eq!(parse_number("17_Doraemon_Nobita_Test.mp4"), Some(17));
        assert_eq!(parse_number("no_number.mp4"), None);
    }

    #[test]
    fn test_remove_diacritics() {
        assert_eq!(remove_diacritics("Thám hiểm vùng đất mới"), "Tham hiem vung dat moi");
        assert_eq!(remove_diacritics("Điện ảnh Việt Nam"), "Dien anh Viet Nam");
    }

    #[test]
    fn test_format_name() {
        assert_eq!(format_name("Movie {n} - {t}.mp4", 17, "Test"), "Movie 17 - Test.mp4");
    }

    #[test]
    fn test_extract_title() {
        let result = extract_title("17_Doraemon_Nobita_Test.mp4", "Doraemon_Nobita_");
        assert_eq!(result.as_deref(), Some("Test"));
    }
}
