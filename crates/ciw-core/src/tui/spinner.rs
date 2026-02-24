const BRAILLE_FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

pub fn frame(idx: usize) -> char {
    BRAILLE_FRAMES[idx % BRAILLE_FRAMES.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn braille_char_range() {
        for &ch in BRAILLE_FRAMES {
            assert!(
                ('\u{2800}'..='\u{28FF}').contains(&ch),
                "char {:?} not in Braille range",
                ch
            );
        }
    }

    #[test]
    fn wrap_around() {
        let first = frame(0);
        let wrapped = frame(BRAILLE_FRAMES.len());
        assert_eq!(first, wrapped);
    }

    #[test]
    fn all_frames_distinct() {
        let mut seen = std::collections::HashSet::new();
        for i in 0..BRAILLE_FRAMES.len() {
            assert!(seen.insert(frame(i)), "duplicate frame at index {}", i);
        }
    }

    #[test]
    fn large_index_no_panic() {
        let _ = frame(usize::MAX);
    }
}
