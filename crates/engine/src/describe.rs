use std::sync::Arc;

use deckcraft_model::{Master, Presentation, Shape, ShapeId, Slide, find_shape};

use crate::{DocState, Selection, Target, UiRequest};

pub(crate) fn place(target: Target, slide: usize) -> String {
    match target {
        Target::Slides => format!("slide index {slide}"),
        Target::Master { master } => format!("slide master {master}"),
        Target::Layout { master, layout } => format!("layout {layout} of slide master {master}"),
    }
}

fn locate(doc: &Presentation, id: ShapeId) -> Option<(String, String)> {
    if let Some(i) = doc.slides.iter().position(|s| find_shape(&s.shapes, id).is_some()) {
        return Some((place(Target::Slides, i), format!("slide.go {{\"index\": {i}}}")));
    }
    doc.masters.iter().enumerate().find_map(|(m, master)| {
        if find_shape(&master.shapes, id).is_some() {
            return Some((place(Target::Master { master: m }, 0), format!("view.slideMaster {{\"master\": {m}}}")));
        }
        master
            .layouts
            .iter()
            .position(|l| find_shape(&l.shapes, id).is_some())
            .map(|l| (place(Target::Layout { master: m, layout: l }, 0), format!("view.slideMaster {{\"master\": {m}, \"layout\": {l}}}")))
    })
}

pub(crate) fn no_shape(st: &DocState, id: ShapeId) -> String {
    let here = place(st.selection.target, st.selection.slide);
    match locate(&st.doc, id) {
        Some((there, go)) => {
            format!("shape {id} is on {there}, but shape ids are looked up on {here}, the one being edited; {go} switches to {there}")
        }
        None => format!("no shape {id} in the presentation; slide.inspect lists the shapes of {here}, the one being edited"),
    }
}

pub(crate) fn none_at(what: &str, i: usize, n: usize) -> String {
    match n {
        0 => format!("no {what} at index {i}; there are none"),
        n => format!("no {what} at index {i}; indices run 0–{}", n - 1),
    }
}

fn shapes(ids: &[ShapeId]) -> String {
    let list = ids.iter().map(ToString::to_string).collect::<Vec<_>>().join(", ");
    if ids.len() == 1 { format!("shape {list}") } else { format!("shapes {list}") }
}

fn slides(indices: &[usize]) -> String {
    let list = indices.iter().map(ToString::to_string).collect::<Vec<_>>().join(", ");
    if indices.len() == 1 { format!("slide index {list}") } else { format!("slides at index {list}") }
}

pub(crate) fn changes(before: Option<(u64, Arc<Presentation>, Selection)>, after: Option<&DocState>, requests: &[UiRequest]) -> String {
    let mut said = match (before, after) {
        (_, None) => vec!["no presentation is open".to_string()],
        (Some((uid, doc, sel)), Some(st)) if uid == st.uid => {
            let mut said = if Arc::ptr_eq(&doc, &st.doc) { vec![] } else { document(&doc, &st.doc) };
            said.extend(selection(&sel, &st.selection, &st.doc));
            said
        }
        (_, Some(st)) => vec![format!("the active presentation is now {}", st.title())],
    };
    said.extend(requests.iter().map(request));
    if said.is_empty() { "nothing changed".into() } else { said.join("; ") }
}

fn document(old: &Presentation, new: &Presentation) -> Vec<String> {
    let mut said = vec![];
    let missing_from = |a: &Presentation, b: &Presentation| -> Vec<usize> {
        a.slides.iter().enumerate().filter(|(_, s)| !b.slides.iter().any(|o| o.id == s.id)).map(|(i, _)| i).collect()
    };
    let (added, deleted) = (missing_from(new, old), missing_from(old, new));
    if !added.is_empty() {
        said.push(format!("added {}", slides(&added)));
    }
    if !deleted.is_empty() {
        said.push(format!("deleted {} (as numbered before)", slides(&deleted)));
    }
    let kept_old: Vec<_> = old.slides.iter().map(|s| s.id).filter(|id| new.slides.iter().any(|n| n.id == *id)).collect();
    let kept_new: Vec<_> = new.slides.iter().map(|s| s.id).filter(|id| kept_old.contains(id)).collect();
    if kept_old != kept_new {
        said.push("reordered the slides".into());
    }
    let mut parts: Vec<(String, Vec<usize>)> = vec![];
    for (i, s) in new.slides.iter().enumerate() {
        if let Some(o) = old.slides.iter().find(|o| o.id == s.id)
            && !Arc::ptr_eq(o, s)
            && o != s
        {
            let (shape_edits, slide_parts) = slide(i, o, s);
            said.extend(shape_edits);
            match parts.iter_mut().find(|(p, _)| *p == slide_parts) {
                Some((_, at)) => at.push(i),
                None if !slide_parts.is_empty() => parts.push((slide_parts, vec![i])),
                None => {}
            }
        }
    }
    for (p, at) in parts {
        let which = if at.len() == new.slides.len() && at.len() > 1 { "every slide".into() } else { slides(&at) };
        said.push(format!("changed the {p} of {which}"));
    }
    if old.masters.len() != new.masters.len() {
        said.push("changed the slide masters".into());
    } else {
        for (mi, (a, b)) in old.masters.iter().zip(&new.masters).enumerate() {
            if !Arc::ptr_eq(a, b) && a != b {
                said.extend(master(mi, a, b));
            }
        }
    }
    let settings: Vec<&str> = [
        (old.slide_size != new.slide_size, "slide size"),
        (old.notes_size != new.notes_size, "notes page size"),
        (old.sections != new.sections, "sections"),
        (old.custom_shows != new.custom_shows, "custom shows"),
        (old.show != new.show, "slide show settings"),
        (old.header_footer != new.header_footer, "headers and footers"),
        (old.props != new.props, "document properties"),
        (old.default_text_style != new.default_text_style, "default text style"),
        (old.first_slide_number != new.first_slide_number, "first slide number"),
        (old.embedded_fonts != new.embedded_fonts, "embedded fonts"),
        (old.notes_master != new.notes_master, "notes master"),
        (old.handout_master != new.handout_master, "handout master"),
    ]
    .into_iter()
    .filter_map(|(changed, name)| changed.then_some(name))
    .collect();
    if !settings.is_empty() {
        said.push(format!("changed the presentation's {}", settings.join(", ")));
    }
    said
}

fn master(mi: usize, old: &Master, new: &Master) -> Vec<String> {
    let on = place(Target::Master { master: mi }, 0);
    let mut said = shape_changes(&on, &old.shapes, &new.shapes);
    let parts = [
        (old.theme != new.theme, "theme"),
        (old.background != new.background, "background"),
        (old.color_map != new.color_map, "colour map"),
        (old.title_style != new.title_style, "title text style"),
        (old.body_style != new.body_style, "body text style"),
        (old.other_style != new.other_style, "other text style"),
        (old.name != new.name, "name"),
    ];
    said.extend(changed_parts(&parts).map(|p| format!("changed the {p} of {on}")));
    let ids = |m: &Master| m.layouts.iter().map(|l| l.id).collect::<Vec<_>>();
    let (old_ids, new_ids) = (ids(old), ids(new));
    for (li, l) in new.layouts.iter().enumerate() {
        let at = place(Target::Layout { master: mi, layout: li }, 0);
        let Some(o) = old.layouts.iter().find(|o| o.id == l.id) else {
            said.push(format!("added {at}"));
            continue;
        };
        if o == l {
            continue;
        }
        said.extend(shape_changes(&at, &o.shapes, &l.shapes));
        let parts = [
            (o.name != l.name, "name"),
            (o.background != l.background, "background"),
            (o.show_master_shapes != l.show_master_shapes, "background graphics"),
            (o.color_map != l.color_map, "colour map"),
        ];
        said.extend(changed_parts(&parts).map(|p| format!("changed the {p} of {at}")));
    }
    let deleted = old_ids.iter().filter(|id| !new_ids.contains(id)).count();
    if deleted > 0 {
        said.push(format!("deleted {deleted} layout{} of {on}", if deleted == 1 { "" } else { "s" }));
    }
    if said.is_empty() {
        said.push(format!("changed {on}"));
    }
    said
}

fn changed_parts<'a>(parts: &'a [(bool, &'a str)]) -> impl Iterator<Item = String> + 'a {
    let list: Vec<&str> = parts.iter().filter_map(|(changed, name)| changed.then_some(*name)).collect();
    (!list.is_empty()).then(|| list.join(", ")).into_iter()
}

fn shape_changes(on: &str, old: &[Shape], new: &[Shape]) -> Vec<String> {
    let ids = |v: &[Shape]| v.iter().map(|s| s.id).collect::<Vec<_>>();
    let (old_ids, new_ids) = (ids(old), ids(new));
    let added: Vec<ShapeId> = new_ids.iter().copied().filter(|id| !old_ids.contains(id)).collect();
    let deleted: Vec<ShapeId> = old_ids.iter().copied().filter(|id| !new_ids.contains(id)).collect();
    let changed: Vec<ShapeId> = new.iter().filter(|s| old.iter().any(|o| o.id == s.id && o != *s)).map(|s| s.id).collect();
    let mut said = vec![];
    for (verb, list) in [("added", &added), ("deleted", &deleted), ("changed", &changed)] {
        if !list.is_empty() {
            said.push(format!("{verb} {} on {on}", shapes(list)));
        }
    }
    let kept_old: Vec<_> = old_ids.iter().filter(|id| new_ids.contains(id)).collect();
    let kept_new: Vec<_> = new_ids.iter().filter(|id| old_ids.contains(id)).collect();
    if kept_old != kept_new {
        said.push(format!("reordered the shapes on {on}"));
    }
    said
}

fn slide(i: usize, old: &Slide, new: &Slide) -> (Vec<String>, String) {
    let said = shape_changes(&slides(&[i]), &old.shapes, &new.shapes);
    let parts = [
        (old.background != new.background, "background"),
        (old.transition != new.transition, "transition"),
        (old.animations != new.animations, "animations"),
        (old.notes != new.notes, "notes"),
        (old.comments != new.comments, "comments"),
        (old.layout != new.layout, "layout"),
        (old.name != new.name, "name"),
        (old.hidden != new.hidden, "hidden state"),
        (old.show_master_shapes != new.show_master_shapes, "background graphics"),
    ]
    .into_iter()
    .filter_map(|(changed, name)| changed.then_some(name))
    .collect::<Vec<_>>()
    .join(", ");
    (said, parts)
}

fn selection(old: &Selection, new: &Selection, doc: &Presentation) -> Vec<String> {
    let mut said = vec![];
    if old.slide != new.slide || old.target != new.target {
        said.push(format!("now editing {}", place(new.target, new.slide)));
    }
    if old.text != new.text {
        said.push(match &new.text {
            Some(t) if t.notes => format!("editing the notes of {}, paragraph {}, character {}", place(new.target, new.slide), t.caret.0, t.caret.1),
            Some(t) => format!("editing the text of shape {}, paragraph {}, character {}", t.shape, t.caret.0, t.caret.1),
            None => "stopped editing text".into(),
        });
    }
    if old.shapes != new.shapes {
        said.push(if new.shapes.is_empty() { "no shape is selected".into() } else { format!("selected {}", shapes(&new.shapes)) });
    }
    if old.slides != new.slides {
        let indices: Vec<usize> = new.slides.iter().filter_map(|id| doc.slides.iter().position(|s| s.id == *id)).collect();
        said.push(if indices.is_empty() { "no slides are selected".into() } else { format!("selected {}", slides(&indices)) });
    }
    if old.cells != new.cells {
        said.push(match new.cells {
            Some((table, (r0, c0), (r1, c1))) => format!("selected cells [{r0}, {c0}] to [{r1}, {c1}] of table {table}"),
            None => "no table cells are selected".into(),
        });
    }
    said
}

fn request(r: &UiRequest) -> String {
    match r {
        UiRequest::Dialog { id, .. } => format!("asked the app to open the {id} dialog"),
        UiRequest::Pane { id } => format!("asked the app to show the {id} pane"),
        UiRequest::StartShow { from } => format!("asked the app to start the slide show at slide index {from}"),
        UiRequest::Message { text } => text.clone(),
        UiRequest::PickFile { purpose } => format!("asked the app to pick a file ({purpose})"),
        UiRequest::Media { action, shape, .. } => format!("asked the app to {action} media shape {shape}"),
    }
}
