//! Text cut into spans so single characters carry extra letter spacing (in
//! em, on top of the style's tracking) and ranges carry their own weight or
//! family. Byte offsets stay those of the whole text.

use cosmic_text::Attrs;

use super::layout::StyleRun;

pub(super) fn spans<'t, 'a>(
    text: &'t str,
    tracking: f32,
    spaced: &[(usize, f32)],
    runs: &[StyleRun],
    attrs_for: impl Fn(Option<&StyleRun>) -> Attrs<'a>,
) -> Vec<(&'t str, Attrs<'a>)> {
    let extra_at = |offset: usize| spaced.iter().find(|s| s.0 == offset).map_or(0.0, |s| s.1);
    let run_at = |offset: usize| runs.iter().rev().find(|r| offset >= r.start && offset < r.end);
    let mut out: Vec<(&str, Attrs)> = Vec::new();
    let mut open: Option<(usize, Option<StyleRun>, f32)> = None;
    let close = |out: &mut Vec<(&'t str, Attrs<'a>)>, start: usize, end: usize, run: Option<StyleRun>, em: f32| {
        let attrs = attrs_for(run.as_ref());
        let em = tracking + em;
        out.push((&text[start..end], if em == 0.0 { attrs } else { attrs.letter_spacing(em) }));
    };
    for (offset, _) in text.char_indices() {
        let key = (run_at(offset).copied(), extra_at(offset));
        match open {
            Some((_, run, em)) if (run, em) == key => {}
            Some((start, run, em)) => {
                close(&mut out, start, offset, run, em);
                open = Some((offset, key.0, key.1));
            }
            None => open = Some((offset, key.0, key.1)),
        }
    }
    match open {
        Some((start, run, em)) => close(&mut out, start, text.len(), run, em),
        None => close(&mut out, 0, 0, None, 0.0),
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::system::{FontFamily, FontWeight};

    fn attrs(_: Option<&StyleRun>) -> Attrs<'static> {
        Attrs::new()
    }

    #[test]
    fn spaced_characters_get_their_own_span() {
        let cut: Vec<&str> =
            spans("fix /motion now", 0.0, &[(3, 0.4), (11, 0.4)], &[], attrs).into_iter().map(|s| s.0).collect();
        assert_eq!(cut, ["fix", " ", "/motion", " ", "now"]);
        let plain: Vec<&str> = spans("hey", 0.0, &[(9, 1.0)], &[], attrs).into_iter().map(|s| s.0).collect();
        assert_eq!(plain, ["hey"], "offsets past the text are ignored");
    }

    #[test]
    fn runs_cut_the_text_where_they_begin_and_end() {
        let bold = StyleRun { start: 2, end: 6, weight: Some(FontWeight::SemiBold), family: Some(FontFamily::Sans) };
        let cut: Vec<&str> = spans("a **b** c", 0.0, &[], &[bold], attrs).into_iter().map(|s| s.0).collect();
        assert_eq!(cut, ["a ", "**b*", "* c"]);
    }

    #[test]
    fn empty_text_still_has_one_span() {
        assert_eq!(spans("", 0.0, &[], &[], attrs).len(), 1);
    }
}
