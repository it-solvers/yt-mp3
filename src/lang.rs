// 화면 문구를 한국어와 영어 중에서 고르기

use std::sync::atomic::{AtomicBool, Ordering};

// 지금 한국어를 쓰는지. 다운로드 thread에서도 읽으므로 Atomic 사용
static KOREAN: AtomicBool = AtomicBool::new(false);

pub fn is_korean() -> bool {
    KOREAN.load(Ordering::Relaxed)
}

pub fn set_korean(korean: bool) {
    KOREAN.store(korean, Ordering::Relaxed);
}

// 시스템 언어가 한국어인지
pub fn system_is_korean() -> bool {
    sys_locale::get_locale().is_some_and(|locale| is_korean_locale(&locale))
}

// "ko-KR", "ko_KR" 같은 값의 언어 부분이 한국어인지
fn is_korean_locale(locale: &str) -> bool {
    locale
        .split(['-', '_'])
        .next()
        .is_some_and(|language| language.eq_ignore_ascii_case("ko"))
}

// 사용법: tr!("한국어 문구", "English text")
// format!처럼 {변수} 를 문구 안에 쓸 수 있음
macro_rules! tr {
    ($korean:literal, $english:literal $(,)?) => {
        if $crate::lang::is_korean() {
            format!($korean)
        } else {
            format!($english)
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_korean_locale() {
        assert!(is_korean_locale("ko-KR"));
        assert!(is_korean_locale("ko_KR"));
        assert!(is_korean_locale("ko"));
        assert!(!is_korean_locale("en-US"));
        assert!(!is_korean_locale("kok-IN"));
        assert!(!is_korean_locale(""));
    }
}
