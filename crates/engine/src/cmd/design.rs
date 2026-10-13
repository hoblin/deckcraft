//! Design tab and masters: themes, colours, fonts, backgrounds, slide size, header & footer,
//! Slide Master view.

use std::sync::Arc;

use deckcraft_geom::Size;
use deckcraft_model::text::{Run, RunKind, TextBody};
use deckcraft_model::theme::{builtin_color_schemes, builtin_font_schemes, builtin_themes};
use deckcraft_model::{Background, Fill, PhType, Presentation, Shape, ShapeId, defaults};
use serde_json::{Value, json};

use super::*;
use crate::{Result, Session, Target};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!("design.theme", "Themes", ["Design", "Themes"], None, "{name} (see design.themes)", has_doc, theme),
        cmd!(query "design.themes", "Theme Gallery", [], None, "{} → themes, colour schemes and font schemes", always, themes),
        cmd!("design.colors", "Colors", ["Design", "Variants"], None, "{name} | {colors: {accent1: #RRGGBB, …}}", has_doc, colors),
        cmd!("design.fonts", "Fonts", ["Design", "Variants"], None, "{name} | {major, minor}", has_doc, fonts),
        cmd!(
            "design.background",
            "Format Background",
            ["Design", "Customize"],
            None,
            "{color? | gradient? | picture?: base64 | pattern?: {preset, fg, bg} | style?: 1..12 | reset?, all?: bool (apply to all), index?} (on a slide master or layout, the dark styles 3, 4, 7, 8, 11 and 12 also turn its text to the theme's light colours, and a fill set afterwards keeps that)",
            has_slide,
            background
        ),
        cmd!("design.hideBackgroundGraphics", "Hide Background Graphics", ["Format Background"], None, "{hide?: bool}", has_slide, hide_bg_graphics),
        cmd!(
            "design.slideSize",
            "Slide Size",
            ["Design", "Customize"],
            None,
            "{preset: widescreen|standard|…, scale?: maximize|ensureFit|none} | {w: pt, h: pt, scale?: maximize|ensureFit|none}",
            has_doc,
            slide_size
        ),
        cmd!(
            "design.headerFooter",
            "Header and Footer…",
            ["Insert", "Text"],
            None,
            "{date?: bool, dateText?: fixed text, slideNumber?: bool, footer?: bool, footerText?, hideOnTitle?: bool, all?: bool (default true)} (with all, footerText and dateText also go into the footer and date placeholders of every slide master and layout)",
            has_doc,
            header_footer
        ),
        cmd!(noundo "view.slideMaster", "Slide Master", ["View", "Master Views"], None, "{master?: index, layout?: index}", has_doc, master_view),
        cmd!(noundo "view.closeMaster", "Close Master View", ["Slide Master", "Close"], None, "{}", has_doc, close_master),
        cmd!("master.insertLayout", "Insert Layout", ["Slide Master", "Edit Master"], None, "{name?}", has_doc, insert_layout),
        cmd!("master.renameLayout", "Rename Layout", ["Slide Master", "Edit Master"], None, "{name, master?, layout?}", has_doc, rename_layout),
        cmd!("master.deleteLayout", "Delete Layout", ["Slide Master", "Edit Master"], None, "{master?, layout?}", has_doc, delete_layout),
        cmd!(
            "master.insertPlaceholder",
            "Insert Placeholder",
            ["Slide Master", "Master Layout"],
            None,
            "{kind?: content|text|picture|chart|table|media|title|subtitle|date|footer|slideNumber, rect?: [x,y,w,h], prompt?: text} (without `rect`, a kind the slide master has (title, text, date, footer, slide number) sits where it has it; on a date, footer or slide number, `prompt` is its text; a layout has one title, date, footer and slide number)",
            has_doc,
            insert_placeholder
        ),
    ]
}

fn theme(s: &mut Session, p: &Value) -> Result<Value> {
    let name = str_param(p, "name").ok_or_else(|| bad("design.theme", "missing `name`"))?;
    let t =
        builtin_themes().into_iter().find(|t| t.name.eq_ignore_ascii_case(name)).ok_or_else(|| bad("design.theme", format!("no theme `{name}`")))?;
    s.edit(|doc, sel| {
        let mi = master_index(doc, sel);
        let m = doc.masters.get_mut(mi).ok_or_else(|| bad("design.theme", "no master"))?;
        let m = Arc::make_mut(m);
        m.theme = t.clone();
        m.name = t.name.clone();
        m.color_map = vec![];
        Ok(())
    })?;
    Ok(json!({"theme": name}))
}

fn master_index(doc: &Presentation, sel: &crate::Selection) -> usize {
    match sel.target {
        Target::Master { master } | Target::Layout { master, .. } => master,
        Target::Slides => doc.slides.get(sel.slide).and_then(|s| doc.masters.iter().position(|m| m.layout(s.layout).is_some())).unwrap_or(0),
    }
}

fn themes(_s: &mut Session, _p: &Value) -> Result<Value> {
    Ok(json!({
        "themes": builtin_themes().iter().map(|t| json!({"name": t.name, "major": t.fonts.major.latin, "minor": t.fonts.minor.latin, "colors": t.colors.colors.iter().map(|c| format!("#{}", c.hex())).collect::<Vec<_>>()})).collect::<Vec<_>>(),
        "colorSchemes": builtin_color_schemes().iter().map(|c| c.name.clone()).collect::<Vec<_>>(),
        "fontSchemes": builtin_font_schemes().iter().map(|f| json!({"name": f.name, "major": f.major.latin, "minor": f.minor.latin})).collect::<Vec<_>>(),
    }))
}

fn colors(s: &mut Session, p: &Value) -> Result<Value> {
    let scheme = if let Some(name) = str_param(p, "name") {
        Some(
            builtin_color_schemes()
                .into_iter()
                .find(|c| c.name.eq_ignore_ascii_case(name))
                .ok_or_else(|| bad("design.colors", format!("no colour scheme `{name}`")))?,
        )
    } else {
        None
    };
    let custom = p.get("colors").and_then(Value::as_object).cloned();
    s.edit(|doc, sel| {
        let mi = master_index(doc, sel);
        let m = Arc::make_mut(doc.masters.get_mut(mi).ok_or_else(|| bad("design.colors", "no master"))?);
        if let Some(sc) = &scheme {
            m.theme.colors = sc.clone();
        }
        if let Some(c) = &custom {
            for (k, v) in c {
                if let (Some(slot), Some(rgb)) = (deckcraft_color::SchemeSlot::from_xml(k), v.as_str().and_then(deckcraft_color::Rgba::from_hex)) {
                    m.theme.colors.set(slot, rgb);
                }
            }
            m.theme.colors.name = "Custom".into();
        }
        Ok(())
    })?;
    ok()
}

fn fonts(s: &mut Session, p: &Value) -> Result<Value> {
    let scheme = if let Some(name) = str_param(p, "name") {
        builtin_font_schemes()
            .into_iter()
            .find(|f| f.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| bad("design.fonts", format!("no font scheme `{name}`")))?
    } else {
        let major = str_param(p, "major").ok_or_else(|| bad("design.fonts", "give `name`, or `major` and `minor`"))?;
        let minor = str_param(p, "minor").unwrap_or(major);
        deckcraft_model::theme::FontScheme {
            name: "Custom".into(),
            major: deckcraft_model::theme::FontSet { latin: major.into(), ..Default::default() },
            minor: deckcraft_model::theme::FontSet { latin: minor.into(), ..Default::default() },
        }
    };
    s.edit(|doc, sel| {
        let mi = master_index(doc, sel);
        Arc::make_mut(doc.masters.get_mut(mi).ok_or_else(|| bad("design.fonts", "no master"))?).theme.fonts = scheme;
        Ok(())
    })?;
    ok()
}

fn background(s: &mut Session, p: &Value) -> Result<Value> {
    use deckcraft_color::SchemeSlot::{Bg1, Bg2, Tx1, Tx2};
    let template = s.doc()?.selection.target != Target::Slides;
    let mut dark = None;
    let reset = bool_or(p, "reset", false);
    let bg: Option<Background> = if reset {
        None
    } else if let Some(c) = color_param(p, "color") {
        Some(Background::Fill { fill: Fill::solid(c) })
    } else if let Some(i) = usize_param(p, "style") {
        // Background styles: 3 theme bg fills × 4 colours (light 1, light 2, dark 2, dark 1).
        let i = i.clamp(1, 12) - 1;
        let column = i % 4;
        dark = Some(column >= 2);
        let slots = if template { [Bg1, Bg2, Bg2, Bg1] } else { [Bg1, Bg2, Tx2, Tx1] };
        Some(Background::Ref { idx: 1001 + (i / 4) as u32, color: deckcraft_model::ColorRef::scheme(slots.get(column).copied().unwrap_or(Bg1)) })
    } else if let Some(g) = p.get("gradient") {
        let mut g = super::shape::gradient_param(g).ok_or_else(|| bad("design.background", "gradient needs two or more `stops`"))?;
        g.rotate_with_shape = false;
        Some(Background::Fill { fill: Fill::Gradient(g) })
    } else if let Some(pt) = p.get("pattern") {
        Some(Background::Fill {
            fill: Fill::Pattern(deckcraft_model::style::PatternFill {
                preset: pt.get("preset").and_then(Value::as_str).unwrap_or("pct50").to_string(),
                fg: pt.get("fg").and_then(color_value).unwrap_or(deckcraft_model::ColorRef::scheme(deckcraft_color::SchemeSlot::Accent1)),
                bg: pt.get("bg").and_then(color_value).unwrap_or(deckcraft_model::ColorRef::scheme(deckcraft_color::SchemeSlot::Bg1)),
            }),
        })
    } else if p.get("picture").is_some() {
        let (name, bytes) = super::insert::media_bytes(&with_param(p, "data", p.get("picture").cloned().unwrap_or_default()), "design.background")?;
        let ct = super::insert::content_type(&name, &bytes);
        let media = s.edit(|doc, _| Ok(doc.add_media(&name, ct, bytes)))?;
        Some(Background::Fill { fill: Fill::Picture(deckcraft_model::style::PictureFill { media, ..Default::default() }) })
    } else {
        return Err(bad("design.background", "give `color`, `gradient`, `pattern`, `picture`, `style` or `reset`"));
    };
    let all = bool_or(p, "all", false);
    let index = usize_param(p, "index");
    s.edit(|doc, sel| {
        match sel.target {
            Target::Master { master } => {
                if let Some(m) = doc.masters.get_mut(master) {
                    let m = Arc::make_mut(m);
                    m.background = bg.clone();
                    if let Some(dark) = dark {
                        m.color_map = if dark { dark_map() } else { vec![] };
                    }
                }
            }
            Target::Layout { master, layout } => {
                let master_light = doc.masters.get(master).is_none_or(|m| m.color_map.is_empty());
                if let Some(l) = layout_mut(doc, master, layout) {
                    l.background = bg.clone();
                    l.color_map = match dark {
                        _ if reset => None,
                        Some(true) => Some(dark_map()),
                        Some(false) if master_light => None,
                        Some(false) => Some(vec![]),
                        None => l.color_map.take(),
                    };
                }
            }
            Target::Slides => {
                let list: Vec<usize> = if all { (0..doc.slides.len()).collect() } else { vec![index.unwrap_or(sel.slide)] };
                for i in list {
                    if let Some(sl) = doc.slides.get_mut(i) {
                        Arc::make_mut(sl).background = bg.clone();
                    }
                }
            }
        }
        Ok(())
    })?;
    ok()
}

fn hide_bg_graphics(s: &mut Session, p: &Value) -> Result<Value> {
    if let Target::Master { master } = s.doc()?.selection.target {
        return Err(bad(
            "design.hideBackgroundGraphics",
            format!("a slide master's graphics are its own; view.slideMaster {{\"master\": {master}, \"layout\": index}} selects one of its layouts"),
        ));
    }
    s.edit(|doc, sel| {
        let shown = match sel.target {
            Target::Layout { master, layout } => layout_mut(doc, master, layout).map(|l| &mut l.show_master_shapes),
            _ => doc.slides.get_mut(sel.slide).map(|sl| &mut Arc::make_mut(sl).show_master_shapes),
        };
        if let Some(shown) = shown {
            *shown = !bool_param(p, "hide").unwrap_or(*shown);
        }
        Ok(())
    })?;
    ok()
}

fn dark_map() -> Vec<(String, String)> {
    [("bg1", "dk1"), ("tx1", "lt1"), ("bg2", "dk2"), ("tx2", "lt2")].into_iter().map(|(k, v)| (k.into(), v.into())).collect()
}

fn layout_mut(doc: &mut Presentation, master: usize, layout: usize) -> Option<&mut deckcraft_model::Layout> {
    doc.masters.get_mut(master).and_then(|m| Arc::make_mut(m).layouts.get_mut(layout))
}

fn scale_shapes(shapes: &mut [Shape], sx: f64, sy: f64, k: f64, ox: f64, oy: f64) {
    for sh in shapes {
        if let Some(x) = sh.xfrm.as_mut() {
            x.x = x.x * sx + ox;
            x.y = x.y * sy + oy;
            x.w *= sx;
            x.h *= sy;
        }
        if let Some(t) = sh.text.as_mut() {
            deckcraft_model::edit::format_all(t, &|r| {
                if let Some(sz) = r.size.as_mut() {
                    *sz = (*sz * k).max(1.0);
                }
            });
        }
    }
}

fn slide_size(s: &mut Session, p: &Value) -> Result<Value> {
    let size = if let Some(pre) = str_param(p, "preset") {
        let k = pre.to_ascii_lowercase();
        let found = defaults::SLIDE_SIZES.iter().find(|(l, _, _)| l.to_ascii_lowercase().starts_with(&k) || l.to_ascii_lowercase().contains(&k));
        let (_, w, h) = found.ok_or_else(|| bad("design.slideSize", format!("unknown preset `{pre}`")))?;
        Size::new(*w, *h)
    } else {
        Size::new(
            f64_param(p, "w").ok_or_else(|| bad("design.slideSize", "missing `w`"))?.clamp(72.0, 4032.0),
            f64_param(p, "h").ok_or_else(|| bad("design.slideSize", "missing `h`"))?.clamp(72.0, 4032.0),
        )
    };
    let mode = str_param(p, "scale").unwrap_or("ensureFit").to_string();
    s.edit(|doc, _| {
        let old = doc.slide_size;
        let (sx, sy) = (size.width / old.width.max(1.0), size.height / old.height.max(1.0));
        let (sx, sy, k, ox, oy) = match mode.as_str() {
            "none" => (1.0, 1.0, 1.0, 0.0, 0.0),
            "maximize" => {
                let k = sx.max(sy);
                (k, k, k, (size.width - old.width * k) / 2.0, (size.height - old.height * k) / 2.0)
            }
            _ => {
                let k = sx.min(sy);
                (k, k, k, (size.width - old.width * k) / 2.0, (size.height - old.height * k) / 2.0)
            }
        };
        // Masters and layouts stretch to the new size; slides scale by mode.
        let (msx, msy) = (size.width / old.width.max(1.0), size.height / old.height.max(1.0));
        for m in &mut doc.masters {
            let m = Arc::make_mut(m);
            scale_shapes(&mut m.shapes, msx, msy, 1.0, 0.0, 0.0);
            for l in &mut m.layouts {
                scale_shapes(&mut l.shapes, msx, msy, 1.0, 0.0, 0.0);
            }
        }
        for sl in &mut doc.slides {
            scale_shapes(&mut Arc::make_mut(sl).shapes, sx, sy, k, ox, oy);
        }
        doc.slide_size = size;
        Ok(())
    })?;
    Ok(json!({"w": size.width, "h": size.height}))
}

fn header_footer(s: &mut Session, p: &Value) -> Result<Value> {
    let footer_text = str_param(p, "footerText");
    let date_text = str_param(p, "dateText");
    s.edit(|doc, sel| {
        let hf = &mut doc.header_footer;
        if let Some(v) = bool_param(p, "date") {
            hf.date = v;
        }
        if let Some(v) = str_param(p, "dateText") {
            hf.date_text = v.into();
        }
        if let Some(v) = bool_param(p, "slideNumber") {
            hf.slide_number = v;
        }
        if let Some(v) = bool_param(p, "footer") {
            hf.footer = v;
        }
        if let Some(v) = str_param(p, "footerText") {
            hf.footer_text = v.into();
            hf.footer = hf.footer || !v.is_empty();
        }
        if let Some(v) = bool_param(p, "hideOnTitle") {
            hf.hide_on_title = v;
        }
        let all = bool_or(p, "all", true);
        let list: Vec<usize> = if all { (0..doc.slides.len()).collect() } else { vec![sel.slide] };
        for i in list {
            place_footers(doc, i, date_text.is_some());
        }
        if all && (footer_text.is_some() || date_text.is_some()) {
            for m in &mut doc.masters {
                let m = Arc::make_mut(m);
                let template = m.shapes.iter_mut().chain(m.layouts.iter_mut().flat_map(|l| l.shapes.iter_mut()));
                for sh in template {
                    match (sh.ph_type(), footer_text, date_text) {
                        (Some(PhType::Footer), Some(text), _) => set_paragraphs(sh, TextBody::from_text(text)),
                        (Some(PhType::Date), _, Some(text)) => set_date(sh, text),
                        _ => {}
                    }
                }
            }
        }
        Ok(())
    })?;
    ok()
}

/// Give slide `i` the date, footer and slide number placeholders the header and footer settings
/// turn on, and only those, with the footer text; with `fix_date`, its date placeholder takes the
/// settings' date too.
pub(crate) fn place_footers(doc: &mut Presentation, i: usize, fix_date: bool) {
    const KINDS: [PhType; 3] = [PhType::Date, PhType::Footer, PhType::SlideNum];
    let hf = doc.header_footer.clone();
    let Some(layout) = doc.slides.get(i).map(|x| x.layout) else { return };
    let (is_title, footers): (bool, Vec<Shape>) = doc
        .layout(layout)
        .map(|(_, l)| {
            let footers = l.shapes.iter().filter(|x| x.ph_type().is_some_and(|k| KINDS.contains(&k))).cloned().collect();
            (l.kind == deckcraft_model::LayoutType::Title, footers)
        })
        .unwrap_or_default();
    let want = |k: PhType| {
        !(hf.hide_on_title && is_title)
            && match k {
                PhType::Date => hf.date,
                PhType::SlideNum => hf.slide_number,
                PhType::Footer => hf.footer,
                _ => false,
            }
    };
    let Some(present) = doc.slides.get(i).map(|sl| KINDS.map(|k| sl.shapes.iter().any(|x| x.ph_type() == Some(k)))) else { return };
    let added: Vec<(&Shape, ShapeId)> = KINDS
        .into_iter()
        .zip(present)
        .filter(|(k, has)| !has && want(*k))
        .filter_map(|(k, _)| footers.iter().find(|lp| lp.ph_type() == Some(k)))
        .map(|lp| (lp, ShapeId(doc.alloc_id())))
        .collect();
    let Some(sl) = doc.slides.get_mut(i) else { return };
    let sl = Arc::make_mut(sl);
    sl.shapes.retain(|x| x.ph_type().is_none_or(|k| !KINDS.contains(&k) || want(k)));
    for sh in &mut sl.shapes {
        match sh.ph_type() {
            Some(PhType::Footer) => sh.text = Some(TextBody::from_text(&hf.footer_text)),
            Some(PhType::Date) if fix_date => set_date(sh, &hf.date_text),
            _ => {}
        }
    }
    for (lp, id) in added {
        let kind = lp.ph_type().unwrap_or_default();
        sl.shapes.push(Shape { id, name: lp.name.clone(), ph: lp.ph.clone(), text: Some(footer_body(kind, &hf)), ..Default::default() });
    }
}

fn field(field: &str, text: String) -> TextBody {
    TextBody {
        paragraphs: vec![deckcraft_model::Paragraph {
            runs: vec![Run { text, props: Default::default(), kind: RunKind::Field { field: field.into() } }],
            ..Default::default()
        }],
        ..Default::default()
    }
}

fn footer_body(k: PhType, hf: &deckcraft_model::HeaderFooter) -> TextBody {
    match k {
        PhType::SlideNum => field("slidenum", "‹#›".into()),
        PhType::Date if hf.date_text.is_empty() => field("datetime1", today()),
        PhType::Date => TextBody::from_text(&hf.date_text),
        _ => TextBody::from_text(&hf.footer_text),
    }
}

fn set_paragraphs(sh: &mut Shape, body: TextBody) {
    sh.text = Some(TextBody { paragraphs: body.paragraphs, ..sh.text.clone().unwrap_or_default() });
}

/// Point a date placeholder at a fixed date (its own plain text) or, for an empty
/// `fixed`, at an automatic date field (one it already has keeps its format).
fn set_date(sh: &mut Shape, fixed: &str) {
    let runs = || sh.text.iter().flat_map(|t| &t.paragraphs).flat_map(|p| &p.runs);
    let has_field = runs().any(|r| matches!(&r.kind, RunKind::Field { field } if deckcraft_text::datetime::is_date_field(field)));
    let props = runs().next().map(|r| r.props.clone()).unwrap_or_default();
    let (text, kind) = match fixed {
        "" if has_field => return,
        "" => (today(), RunKind::Field { field: "datetime1".into() }),
        _ if !has_field && sh.text.as_ref().is_some_and(|t| t.text() == fixed) => return,
        _ => (fixed.to_string(), RunKind::Text),
    };
    let para = deckcraft_model::Paragraph { runs: vec![Run { text, props, kind }], ..Default::default() };
    sh.text = Some(TextBody { paragraphs: vec![para], ..sh.text.clone().unwrap_or_default() });
}

/// Today's local date as a `datetime1` field shows it (M/D/YYYY): the saved text of a new date
/// field. Empty where there is no clock (wasm).
pub fn today() -> String {
    date_field_text("datetime1")
}

/// What date field `kind` shows now (empty if unknown or there is no clock).
pub fn date_field_text(kind: &str) -> String {
    use deckcraft_text::datetime::{DateTime, field_text};
    DateTime::now_local().and_then(|now| field_text(kind, None, now)).unwrap_or_default()
}

/// Days since 1970-01-01 → (year, month, day) (Howard Hinnant's algorithm).
pub fn civil(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn master_view(s: &mut Session, p: &Value) -> Result<Value> {
    let st = s.doc()?;
    let mi = usize_param(p, "master").unwrap_or_else(|| master_index(&st.doc, &st.selection));
    let current = || st.current_slide().and_then(|sl| st.doc.masters.get(mi).and_then(|m| m.layouts.iter().position(|l| l.id == sl.layout)));
    let layout = usize_param(p, "layout").or_else(|| if p.get("master").is_some() { None } else { current() });
    let target = match layout {
        Some(l) => Target::Layout { master: mi, layout: l },
        None => Target::Master { master: mi },
    };
    s.select(|_, sel| {
        sel.target = target;
        sel.shapes.clear();
        sel.text = None;
    })?;
    Ok(serde_json::to_value(target).unwrap_or_default())
}

fn close_master(s: &mut Session, _p: &Value) -> Result<Value> {
    s.select(|_, sel| {
        sel.target = Target::Slides;
        sel.shapes.clear();
        sel.text = None;
    })?;
    ok()
}

fn insert_layout(s: &mut Session, p: &Value) -> Result<Value> {
    let name = str_param(p, "name").unwrap_or("Custom Layout").to_string();
    s.edit(|doc, sel| {
        let mi = master_index(doc, sel);
        let id = deckcraft_model::LayoutId(doc.alloc_id());
        let tid = ShapeId(doc.alloc_id());
        let m = Arc::make_mut(doc.masters.get_mut(mi).ok_or_else(|| bad("master.insertLayout", "no master"))?);
        let title = m.shapes.iter().find(|x| x.ph_type() == Some(PhType::Title)).cloned().map(|mut t| {
            t.id = tid;
            t.text = Some(TextBody::default());
            t
        });
        let at = match sel.target {
            Target::Layout { layout, .. } => layout + 1,
            _ => m.layouts.len(),
        };
        m.layouts.insert(
            at.min(m.layouts.len()),
            deckcraft_model::Layout {
                id,
                name,
                kind: deckcraft_model::LayoutType::Custom,
                shapes: title.into_iter().collect(),
                show_master_shapes: true,
                preserve: true,
                ..Default::default()
            },
        );
        sel.target = Target::Layout { master: mi, layout: at.min(m.layouts.len().saturating_sub(1)) };
        Ok(json!({"id": id}))
    })
}

fn layout_target(s: &Session, p: &Value) -> Result<(usize, usize)> {
    let st = s.doc()?;
    let (m, l) = match st.selection.target {
        Target::Layout { master, layout } => (master, layout),
        _ => (master_index(&st.doc, &st.selection), 0),
    };
    Ok((usize_param(p, "master").unwrap_or(m), usize_param(p, "layout").unwrap_or(l)))
}

fn no_layout_at(doc: &deckcraft_model::Presentation, mi: usize, li: usize) -> String {
    match doc.masters.get(mi) {
        Some(m) => none_at(&format!("layout of slide master {mi}"), li, m.layouts.len()),
        None => none_at("slide master", mi, doc.masters.len()),
    }
}

fn rename_layout(s: &mut Session, p: &Value) -> Result<Value> {
    let (mi, li) = layout_target(s, p)?;
    let name = str_param(p, "name").ok_or_else(|| bad("master.renameLayout", "missing `name`"))?.to_string();
    s.edit(|doc, _| {
        let missing = no_layout_at(doc, mi, li);
        let l = doc.masters.get_mut(mi).and_then(|m| Arc::make_mut(m).layouts.get_mut(li)).ok_or_else(|| bad("master.renameLayout", missing))?;
        l.name = name;
        Ok(())
    })?;
    ok()
}

fn delete_layout(s: &mut Session, p: &Value) -> Result<Value> {
    let (mi, li) = layout_target(s, p)?;
    s.edit(|doc, sel| {
        let id =
            doc.masters.get(mi).and_then(|m| m.layouts.get(li)).map(|l| l.id).ok_or_else(|| bad("master.deleteLayout", no_layout_at(doc, mi, li)))?;
        let users: Vec<String> = doc.slides.iter().enumerate().filter(|(_, x)| x.layout == id).map(|(i, _)| i.to_string()).collect();
        if !users.is_empty() {
            return Err(bad("master.deleteLayout", format!("slides at index {} use this layout", users.join(", "))));
        }
        let m = Arc::make_mut(doc.masters.get_mut(mi).ok_or_else(|| bad("master.deleteLayout", "no master"))?);
        if m.layouts.len() <= 1 {
            return Err(bad("master.deleteLayout", "a master needs at least one layout"));
        }
        m.layouts.remove(li);
        sel.target = Target::Layout { master: mi, layout: li.min(m.layouts.len().saturating_sub(1)) };
        Ok(())
    })?;
    ok()
}

const PLACEHOLDERS: [(&str, PhType, &str); 11] = [
    ("content", PhType::Obj, "Content Placeholder"),
    ("text", PhType::Body, "Text Placeholder"),
    ("picture", PhType::Picture, "Picture Placeholder"),
    ("chart", PhType::Chart, "Chart Placeholder"),
    ("table", PhType::Table, "Table Placeholder"),
    ("media", PhType::Media, "Media Placeholder"),
    ("title", PhType::Title, "Title"),
    ("subtitle", PhType::SubTitle, "Subtitle"),
    ("date", PhType::Date, "Date Placeholder"),
    ("footer", PhType::Footer, "Footer Placeholder"),
    ("slideNumber", PhType::SlideNum, "Slide Number Placeholder"),
];

fn insert_placeholder(s: &mut Session, p: &Value) -> Result<Value> {
    let asked = str_param(p, "kind").unwrap_or("content");
    let Some((_, kind, name)) = PLACEHOLDERS.iter().find(|(k, _, _)| k.eq_ignore_ascii_case(asked)) else {
        let kinds: Vec<&str> = PLACEHOLDERS.iter().map(|(k, _, _)| *k).collect();
        return Err(bad("master.insertPlaceholder", format!("no placeholder kind `{asked}`; the kinds are {}", kinds.join(", "))));
    };
    let st = s.doc()?;
    let Target::Layout { master, layout } = st.selection.target else {
        return Err(bad("master.insertPlaceholder", "placeholders go on a layout; view.slideMaster {\"layout\": index} selects one"));
    };
    let single = kind.is_title() || kind.is_footer_kind();
    if let Some(had) = st.shapes().iter().find(|sh| single && sh.ph_type().is_some_and(|k| k == *kind || (k.is_title() && kind.is_title()))) {
        return Err(bad("master.insertPlaceholder", format!("layout {layout} already has its {asked} placeholder, shape {}", had.id)));
    }
    let size = st.doc.slide_size;
    let on_master = st.doc.masters.get(master).is_some_and(|m| m.shapes.iter().any(|sh| sh.ph_type() == Some(*kind)));
    let xfrm = rect_param(p, "rect")
        .or_else(|| (!on_master).then(|| deckcraft_geom::Xfrm::new(size.width * 0.25, size.height * 0.3, size.width * 0.5, size.height * 0.4)));
    let idx = if kind.is_title() { 0 } else { st.shapes().iter().filter_map(|x| x.ph.as_ref().map(|p| p.idx)).max().unwrap_or(0) + 1 };
    let n = st.shapes().len() + 1;
    let prompt = str_param(p, "prompt").unwrap_or("");
    let text = match kind {
        PhType::Date | PhType::SlideNum if prompt.is_empty() => footer_body(*kind, &st.doc.header_footer),
        _ if prompt.is_empty() => TextBody::default(),
        _ => TextBody::from_text(prompt),
    };
    let shape = Shape {
        name: format!("{name} {n}"),
        xfrm,
        ph: Some(deckcraft_model::Placeholder {
            kind: *kind,
            idx,
            has_custom_prompt: !prompt.is_empty() && !kind.is_footer_kind(),
            ..Default::default()
        }),
        text: Some(text),
        ..Default::default()
    };
    let id = super::insert::add_shape(s, shape, true)?;
    Ok(json!({"id": id}))
}

#[cfg(test)]
mod tests {
    #[test]
    fn civil_dates() {
        assert_eq!(super::civil(0), (1970, 1, 1));
        assert_eq!(super::civil(20_734), (2026, 10, 8));
    }
}
