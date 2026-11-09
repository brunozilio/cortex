//! Numbers and letters as Brazilian Portuguese words, so they can be said.
//! Pure.

const UNITS: [&str; 20] = [
    "zero",
    "um",
    "dois",
    "três",
    "quatro",
    "cinco",
    "seis",
    "sete",
    "oito",
    "nove",
    "dez",
    "onze",
    "doze",
    "treze",
    "catorze",
    "quinze",
    "dezesseis",
    "dezessete",
    "dezoito",
    "dezenove",
];
const TENS: [&str; 10] =
    ["", "", "vinte", "trinta", "quarenta", "cinquenta", "sessenta", "setenta", "oitenta", "noventa"];
const HUNDREDS: [&str; 10] = [
    "",
    "cento",
    "duzentos",
    "trezentos",
    "quatrocentos",
    "quinhentos",
    "seiscentos",
    "setecentos",
    "oitocentos",
    "novecentos",
];

fn small(n: u64) -> String {
    match n {
        0..20 => UNITS[n as usize].into(),
        100 => "cem".into(),
        20..100 => match n % 10 {
            0 => TENS[(n / 10) as usize].into(),
            unit => format!("{} e {}", TENS[(n / 10) as usize], UNITS[unit as usize]),
        },
        _ => match n % 100 {
            0 => HUNDREDS[(n / 100) as usize].into(),
            rest => format!("{} e {}", HUNDREDS[(n / 100) as usize], small(rest)),
        },
    }
}

pub fn cardinal(n: u64) -> String {
    if n < 1000 {
        return small(n);
    }
    let scales: [(u64, &str, &str); 3] =
        [(1_000_000_000, "bilhão", "bilhões"), (1_000_000, "milhão", "milhões"), (1_000, "mil", "mil")];
    let mut parts: Vec<String> = Vec::new();
    let mut rest = n;
    for (size, one, many) in scales {
        let count = rest / size;
        rest %= size;
        if count == 0 {
            continue;
        }
        parts.push(match (count, size) {
            (1, 1_000) => "mil".into(),
            (1, _) => format!("um {one}"),
            _ => format!("{} {many}", cardinal(count)),
        });
    }
    if rest > 0 {
        let joined = if rest < 100 || rest.is_multiple_of(100) { "e " } else { "" };
        parts.push(format!("{joined}{}", small(rest)));
    }
    parts.join(" ")
}

pub fn spoken(text: &str) -> Option<String> {
    let digits = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit());
    let thousands = text.split('.').collect::<Vec<_>>();
    if thousands.len() > 1 && thousands[1..].iter().all(|g| g.len() == 3) && thousands.iter().all(|g| digits(g)) {
        return text.replace('.', "").parse().ok().map(cardinal);
    }
    let (whole, fraction) = match text.split_once([',', '.']) {
        Some((w, f)) => (w, Some(f)),
        None => (text, None),
    };
    if !digits(whole) || fraction.is_some_and(|f| !digits(f)) {
        return None;
    }
    let whole = cardinal(whole.parse().ok()?);
    Some(match fraction {
        None => whole,
        Some(f) => {
            let zeros = f.chars().take_while(|&c| c == '0').count();
            let mut said = vec!["zero"; zeros].join(" ");
            if zeros < f.len() {
                if !said.is_empty() {
                    said.push(' ');
                }
                said.push_str(&cardinal(f[zeros..].parse().ok()?));
            }
            format!("{whole} vírgula {said}")
        }
    })
}

pub fn letter(c: char) -> &'static str {
    match c.to_ascii_lowercase() {
        'a' => "á",
        'b' => "bê",
        'c' => "cê",
        'd' => "dê",
        'e' => "é",
        'f' => "éfe",
        'g' => "gê",
        'h' => "agá",
        'i' => "í",
        'j' => "jóta",
        'k' => "cá",
        'l' => "éle",
        'm' => "ême",
        'n' => "êne",
        'o' => "ó",
        'p' => "pê",
        'q' => "quê",
        'r' => "érre",
        's' => "ésse",
        't' => "tê",
        'u' => "ú",
        'v' => "vê",
        'w' => "dáblio",
        'x' => "xis",
        'y' => "ípsilon",
        'z' => "zê",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cardinals() {
        for (n, words) in [
            (0, "zero"),
            (16, "dezesseis"),
            (21, "vinte e um"),
            (100, "cem"),
            (101, "cento e um"),
            (342, "trezentos e quarenta e dois"),
            (1000, "mil"),
            (1001, "mil e um"),
            (1234, "mil duzentos e trinta e quatro"),
            (2500, "dois mil e quinhentos"),
            (1_000_000, "um milhão"),
            (3_200_000, "três milhões duzentos mil"),
        ] {
            assert_eq!(cardinal(n), words, "{n}");
        }
    }

    #[test]
    fn numbers_as_written() {
        assert_eq!(spoken("3,5").as_deref(), Some("três vírgula cinco"));
        assert_eq!(spoken("1.000").as_deref(), Some("mil"));
        assert_eq!(spoken("0,05").as_deref(), Some("zero vírgula zero cinco"));
        assert_eq!(spoken("2.5").as_deref(), Some("dois vírgula cinco"));
        assert_eq!(spoken("v2"), None);
    }
}
