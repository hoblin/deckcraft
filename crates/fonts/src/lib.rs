//! DeckCraft fonts: the font database (craft-fonts families, user and system fonts, and open
//! substitutes for the Office fonts presentations ask for), vertical metrics, glyph outlines and
//! OpenType shaping, script/slot classification and per-cluster fallback.
//!
//! Shaping here is style-agnostic: [`shape`] and [`shape_range`] turn text in one face into glyph
//! ids, clusters and advances in font units. `deckcraft-text` applies sizes, tracking, scaling and
//! justification on top.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
#![forbid(unsafe_code)]

mod fallback;
mod fontdb;
pub mod script;
mod shape;

pub use fallback::{bundled_families, covers_cluster, ligature_carets};
pub use fontdb::{FALLBACK_FAMILY, FaceRef, FontDb, FontFace, LAST_RESORT_FAMILY, base_style, bundled, substitutes, system_font_dirs};
pub use harfrust::Feature;
use harfrust::Tag;
pub use kurbo::BezPath;
pub use script::{FontSlot, ScriptTag};
pub use shape::{ShapeParams, ShapedGlyph, shape_range};

/// A font from the optional craft-fonts build input (https://github.com/storytold/craft-fonts;
/// empty unless built with `CRAFT_FONTS_DIR`, see `build.rs`).
pub struct CraftFont {
    pub family: &'static str,
    pub style: &'static str,
    /// ISO 15924 scripts the font is for, e.g. `"Jpan"`.
    pub scripts: &'static [&'static str],
    pub bytes: &'static [u8],
}

include!(concat!(env!("OUT_DIR"), "/craft_fonts.rs"));

/// The craft-fonts faces for Japanese (`"Jpan"`), in manifest order. Empty without craft-fonts.
pub fn japanese_fonts() -> impl Iterator<Item = &'static CraftFont> {
    CRAFT_FONTS.iter().filter(|f| f.scripts.contains(&"Jpan"))
}

/// Japanese faces in document-fallback order: Mincho (serif) families first, matching the
/// serif default text font, then the others; Regular before other styles.
pub fn japanese_document_fonts() -> Vec<&'static CraftFont> {
    let mut v: Vec<_> = japanese_fonts().collect();
    v.sort_by_key(|f| (!f.family.contains("Mincho"), f.style != "Regular"));
    v
}

/// Japanese faces in UI order: BIZ UDPGothic first (the UI face), then the others; `bold` puts
/// bold styles before regular ones.
pub fn japanese_ui_fonts(bold: bool) -> Vec<&'static CraftFont> {
    let mut v: Vec<_> = japanese_fonts().collect();
    v.sort_by_key(|f| (f.family != "BIZ UDPGothic", (f.style == "Bold") != bold));
    v
}

/// The default theme's font.
pub const DEFAULT_FAMILY: &str = "Inter";

/// An OpenType feature setting: `"liga"`, `"-kern"`, `"ss01"`.
pub fn feature(tag: &str) -> Option<Feature> {
    let (on, t) = match tag.strip_prefix('-') {
        Some(r) => (false, r),
        None => (true, tag.strip_prefix('+').unwrap_or(tag)),
    };
    let b = t.as_bytes();
    if b.len() != 4 {
        return None;
    }
    Some(Feature::new(Tag::new(&[b[0], b[1], b[2], b[3]]), on as u32, ..))
}

/// A strong right-to-left character (Hebrew, Arabic, …)?
pub fn is_rtl(c: char) -> bool {
    use unicode_bidi::BidiClass::{AL, R};
    matches!(unicode_bidi::bidi_class(c), R | AL)
}

/// Shape `text` with `face`. `chars` lets callers substitute characters (e.g. uppercase for All
/// Caps) while keeping clusters pointing into the original string. Glyphs come in logical
/// order: each direction run is shaped with [`shape_range`] (right-to-left runs right to left,
/// with the whole string as context), then its clusters put back in text order (each cluster's
/// glyphs keep the shaper's order). Paragraph layout uses [`shape_range`] directly and keeps the
/// visual order.
pub fn shape(face: &FontFace, text: &str, features: &[Feature], map: impl Fn(char) -> char) -> Vec<ShapedGlyph> {
    let mut out = Vec::with_capacity(text.len());
    for (r, rtl) in direction_runs(text) {
        let mut g = shape_range(face, text, r, &ShapeParams { features, rtl, ..Default::default() }, &map);
        if rtl {
            // Visual (clusters descending) → logical; the sort is stable, so each cluster's
            // glyphs keep the shaper's order.
            g.sort_by_key(|x| x.cluster);
        }
        out.extend(g);
    }
    out
}

/// Maximal runs of one direction (neutrals join the run they're in): (byte range, right-to-left).
fn direction_runs(text: &str) -> Vec<(std::ops::Range<usize>, bool)> {
    let mut runs: Vec<(std::ops::Range<usize>, bool)> = Vec::new();
    let mut cur: Option<bool> = None;
    let mut start = 0;
    for (i, c) in text.char_indices() {
        let strong = if is_rtl(c) {
            Some(true)
        } else if unicode_bidi::bidi_class(c) == unicode_bidi::BidiClass::L {
            Some(false)
        } else {
            None
        };
        match (cur, strong) {
            (None, Some(d)) => cur = Some(d),
            (Some(a), Some(d)) if a != d => {
                runs.push((start..i, a));
                start = i;
                cur = Some(d);
            }
            _ => {}
        }
    }
    runs.push((start..text.len(), cur.unwrap_or(false)));
    runs
}

/// Glyph id of the first of `chars` the face has (0 = .notdef).
pub fn first_glyph(face: &FontFace, chars: &[char]) -> u32 {
    chars.iter().map(|c| face.glyph_for(*c)).find(|g| *g != 0).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn any_face() -> std::sync::Arc<FontFace> {
        FontDb::global().face(DEFAULT_FAMILY, "Regular")
    }

    #[test]
    fn last_resort_face_parses() {
        let f = fontdb::last_resort_face();
        assert_eq!(f.family, fontdb::LAST_RESORT_FAMILY);
        assert!(f.upem > 0.0);
    }

    #[test]
    fn some_face_always_resolves() {
        let db = FontDb::with_font_dirs(Vec::new());
        db.set_system_fallback(false);
        let f = db.face("No Such Font Anywhere", "Regular");
        assert!(!f.family.is_empty());
        assert!(!shape(&f, "Hello", &[], |c| c).is_empty());
    }

    #[test]
    fn office_fonts_have_substitutes() {
        assert!(substitutes("Calibri").contains(&"Carlito"));
        assert!(substitutes("Cambria").contains(&"Caladea"));
        assert!(substitutes("Times New Roman").contains(&"Liberation Serif"));
        assert!(substitutes("Aptos").contains(&"Inter"));
        assert!(substitutes("zzz").is_empty());
    }

    #[test]
    fn shaping_produces_clusters_and_advances() {
        let face = any_face();
        let g = shape(&face, "Hello", &[], |c| c);
        assert_eq!(g.len(), 5);
        assert_eq!(g.iter().map(|g| g.cluster).collect::<Vec<_>>(), vec![0, 1, 2, 3, 4]);
        assert!(g.iter().all(|g| g.x_advance > 0 && g.gid != 0));
    }

    #[test]
    fn mapping_keeps_clusters() {
        let face = any_face();
        let g = shape(&face, "ab", &[], |c| c.to_ascii_uppercase());
        assert_eq!(g[0].gid, face.glyph_for('A'));
        assert_eq!(g[1].cluster, 1);
    }

    #[test]
    fn outlines_and_metrics() {
        let db = FontDb::global();
        let face = any_face();
        let o = db.outline(&face, face.glyph_for('O'));
        assert!(!o.elements().is_empty());
        assert!(face.ascent > 0.0 && face.descent > 0.0);
        assert!(feature("abc").is_none());
        assert!(feature("liga").is_some());
    }

    fn name_table(names: &[(u16, &str)]) -> Vec<u8> {
        let mut records = Vec::new();
        let mut strings = Vec::new();
        for (id, text) in names {
            let utf16: Vec<u8> = text.encode_utf16().flat_map(u16::to_be_bytes).collect();
            for v in [3u16, 1, 0x0409, *id, utf16.len() as u16, strings.len() as u16] {
                records.extend(v.to_be_bytes());
            }
            strings.extend(utf16);
        }
        let mut table = Vec::new();
        for v in [0u16, names.len() as u16, (6 + records.len()) as u16] {
            table.extend(v.to_be_bytes());
        }
        table.extend(records);
        table.extend(strings);
        table
    }

    fn renamed(font: &[u8], names: &[(u16, &str)]) -> Vec<u8> {
        let mut out = font.to_vec();
        out.resize(out.len().next_multiple_of(4), 0);
        let table = name_table(names);
        let (at, len) = (out.len() as u32, table.len() as u32);
        out.extend(table);
        let tables = u16::from_be_bytes([out[4], out[5]]) as usize;
        let record = (0..tables).map(|i| 12 + 16 * i).find(|r| &out[*r..*r + 4] == b"name").unwrap();
        out[record + 8..record + 12].copy_from_slice(&at.to_be_bytes());
        out[record + 12..record + 16].copy_from_slice(&len.to_be_bytes());
        out
    }

    #[test]
    fn a_family_named_per_weight_answers_to_both_names() {
        let dir = std::env::temp_dir().join(format!("deckcraft-per-weight-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for (file, weight) in [("TestGothic-Bold.ttf", "Bold"), ("TestGothic-Light.ttf", "Light")] {
            let legacy = format!("Test Gothic {weight}");
            let names = [(1, legacy.as_str()), (2, "Regular"), (16, "Test Gothic"), (17, weight)];
            std::fs::write(dir.join(file), renamed(epaint_default_fonts::UBUNTU_LIGHT, &names)).unwrap();
        }
        let style_linked = [(1, "Test Gothic"), (2, "Bold"), (16, "Test Gothic"), (17, "Bold")];
        std::fs::write(dir.join("TestGothic-A.ttf"), renamed(epaint_default_fonts::UBUNTU_LIGHT, &style_linked)).unwrap();
        let db = FontDb::with_font_dirs(vec![dir.clone()]);
        db.set_system_fallback(false);
        assert_eq!(db.face("Test Gothic", "Bold").family, "Test Gothic");
        let by_weight_name = db.face("Test Gothic Bold", "Regular");
        let by_family = db.face("Test Gothic", "Light");
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!((by_weight_name.family.as_str(), by_weight_name.style.as_str()), ("Test Gothic", "Bold"));
        assert_eq!((by_family.family.as_str(), by_family.style.as_str()), ("Test Gothic", "Light"));
        assert!(db.has_family("Test Gothic Light"));
    }

    #[test]
    fn craft_fonts_latin_when_present() {
        if !CRAFT_FONTS.iter().any(|f| f.family == "Inter") {
            eprintln!("skipped: built without craft-fonts' Latin manifest (set CRAFT_FONTS_DIR)");
            return;
        }
        let db = FontDb::with_font_dirs(Vec::new());
        db.set_system_fallback(false);
        assert_eq!(db.face("Inter", "Regular").family, "Inter");
        assert_eq!(db.face("Aptos", "Regular").family, "Inter");
        if CRAFT_FONTS.iter().any(|f| f.family == "Carlito") {
            assert_eq!(db.face("Calibri", "Bold").family, "Carlito");
        }
    }
}
