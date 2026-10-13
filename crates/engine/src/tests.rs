use serde_json::json;

use crate::*;

fn session() -> Session {
    Session::with_new()
}

#[test]
fn new_presentation_and_slides() {
    let mut s = session();
    assert_eq!(s.doc().unwrap().doc.slides.len(), 1);
    s.execute("slide.new", &json!({})).unwrap();
    s.execute("slide.new", &json!({"layout": "blank"})).unwrap();
    assert_eq!(s.doc().unwrap().doc.slides.len(), 3);
    assert_eq!(s.doc().unwrap().selection.slide, 2);
    s.execute("slide.duplicate", &json!({})).unwrap();
    assert_eq!(s.doc().unwrap().doc.slides.len(), 4);
    s.execute("slide.move", &json!({"from": 3, "to": 0})).unwrap();
    s.execute("slide.delete", &json!({"index": 0})).unwrap();
    assert_eq!(s.doc().unwrap().doc.slides.len(), 3);
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(s.doc().unwrap().doc.slides.len(), 4);
    s.execute("edit.redo", &json!({})).unwrap();
    assert_eq!(s.doc().unwrap().doc.slides.len(), 3);
}

#[test]
fn insert_shape_and_format() {
    let mut s = session();
    let r = s.execute("shape.insert", &json!({"preset": "roundRect", "rect": [100, 100, 200, 100], "text": "Hi"})).unwrap();
    let id = r["id"].as_u64().unwrap();
    s.execute("shape.fill", &json!({"color": "#ff0000"})).unwrap();
    s.execute("shape.line", &json!({"width": 3, "color": "accent2", "dash": "dash"})).unwrap();
    s.execute("shape.rotate", &json!({"deg": 30})).unwrap();
    s.execute("shape.move", &json!({"dx": 10, "dy": -5})).unwrap();
    s.execute("format.bold", &json!({})).unwrap();
    let v = s.execute("shape.inspect", &json!({"id": id})).unwrap();
    assert_eq!(v["box"]["x"], 110.0);
    assert_eq!(v["box"]["rot"], 30.0);
    let runs = &v["shape"]["text"]["paragraphs"][0]["runs"][0]["props"];
    assert_eq!(runs["bold"], true);
    // Undo restores step by step.
    for _ in 0..5 {
        s.execute("edit.undo", &json!({})).unwrap();
    }
    let v = s.execute("shape.inspect", &json!({"id": id})).unwrap();
    assert!(v["shape"]["fill"].is_null());
}

#[test]
fn text_editing_flow() {
    let mut s = session();
    let title = s.doc().unwrap().current_slide().unwrap().shapes[0].id.0;
    s.execute("text.edit", &json!({"id": title})).unwrap();
    s.execute("text.insert", &json!({"text": "Hello world"})).unwrap();
    s.execute("text.move", &json!({"to": "wordLeft", "extend": true})).unwrap();
    s.execute("format.italic", &json!({})).unwrap();
    s.execute("text.move", &json!({"to": "end"})).unwrap();
    s.execute("text.delete", &json!({"dir": "backward"})).unwrap();
    s.execute("text.insert", &json!({"text": "D\nSecond"})).unwrap();
    let t = s.execute("text.get", &json!({"id": title})).unwrap();
    assert_eq!(t["text"], "Hello worlD\nSecond");
    s.execute("text.exit", &json!({})).unwrap();
    assert!(s.doc().unwrap().selection.text.is_none());
    assert_eq!(s.doc().unwrap().selection.shapes.len(), 1);
}

#[test]
fn smart_quotes_and_autocorrect() {
    let mut s = session();
    let title = s.doc().unwrap().current_slide().unwrap().shapes[0].id.0;
    s.execute("text.edit", &json!({"id": title})).unwrap();
    for c in ["\"", "h", "i", "\"", " ", "t", "e", "h", " "] {
        s.execute("text.insert", &json!({"text": c})).unwrap();
    }
    let t = s.execute("text.get", &json!({"id": title})).unwrap();
    assert_eq!(t["text"], "“hi” the ");
}

#[test]
fn clipboard_copy_paste_shapes_and_slides() {
    let mut s = session();
    s.execute("shape.insert", &json!({"preset": "ellipse", "rect": [10, 10, 50, 50]})).unwrap();
    s.execute("edit.copy", &json!({})).unwrap();
    s.execute("edit.paste", &json!({})).unwrap();
    s.execute("edit.paste", &json!({})).unwrap();
    let n = s.doc().unwrap().current_slide().unwrap().shapes.len();
    assert_eq!(n, 2 + 3);
    s.execute("edit.copy", &json!({"scope": "slides"})).unwrap();
    s.execute("edit.paste", &json!({"scope": "slides"})).unwrap();
    assert_eq!(s.doc().unwrap().doc.slides.len(), 2);
    assert!(s.doc().unwrap().doc.validate().is_empty());
}

#[test]
fn group_ungroup_roundtrip_positions() {
    let mut s = session();
    let a = s.execute("shape.insert", &json!({"preset": "rect", "rect": [100, 100, 50, 50]})).unwrap()["id"].as_u64().unwrap();
    let b = s.execute("shape.insert", &json!({"preset": "rect", "rect": [300, 200, 50, 50]})).unwrap()["id"].as_u64().unwrap();
    s.execute("edit.select", &json!({"ids": [a, b]})).unwrap();
    let g = s.execute("arrange.group", &json!({})).unwrap()["id"].as_u64().unwrap();
    s.execute("shape.move", &json!({"id": g, "dx": 10})).unwrap();
    s.execute("arrange.ungroup", &json!({"ids": [g]})).unwrap();
    let v = s.execute("shape.inspect", &json!({"id": b})).unwrap();
    assert!((v["box"]["x"].as_f64().unwrap() - 310.0).abs() < 1e-6, "{v}");
}

#[test]
fn align_and_distribute() {
    let mut s = session();
    let mut ids = vec![];
    for (x, y) in [(10.0, 10.0), (200.0, 50.0), (500.0, 90.0)] {
        ids.push(s.execute("shape.insert", &json!({"preset": "rect", "rect": [x, y, 40, 40]})).unwrap()["id"].as_u64().unwrap());
    }
    s.execute("edit.select", &json!({"ids": ids})).unwrap();
    s.execute("arrange.align", &json!({"edge": "top"})).unwrap();
    s.execute("arrange.distribute", &json!({"dir": "horizontal"})).unwrap();
    let ys: Vec<f64> = ids.iter().map(|i| s.execute("shape.inspect", &json!({"id": i})).unwrap()["box"]["y"].as_f64().unwrap()).collect();
    assert!(ys.iter().all(|y| (*y - 10.0).abs() < 1e-6));
    let x1 = s.execute("shape.inspect", &json!({"id": ids[1]})).unwrap()["box"]["x"].as_f64().unwrap();
    assert!((x1 - 255.0).abs() < 1e-6, "{x1}");
}

#[test]
fn pointer_draw_move_resize() {
    let mut s = session();
    s.set_tool(ToolKind::Shape { preset: "rect".into() });
    let ev = |kind, x, y| PointerEvent { kind, x, y, mods: Mods::default(), tol: 3.0 };
    s.pointer(ev(PointerKind::Down, 100.0, 100.0)).unwrap();
    s.pointer(ev(PointerKind::Drag, 200.0, 180.0)).unwrap();
    let r = s.pointer(ev(PointerKind::Up, 200.0, 180.0)).unwrap();
    let id = r["id"].as_u64().unwrap();
    assert_eq!(s.tool.kind, ToolKind::Select);
    // Move by dragging the middle.
    s.prefs.snap_to_grid = false;
    s.prefs.smart_guides = false;
    s.pointer(ev(PointerKind::Down, 150.0, 140.0)).unwrap();
    s.pointer(ev(PointerKind::Drag, 170.0, 150.0)).unwrap();
    s.pointer(ev(PointerKind::Up, 170.0, 150.0)).unwrap();
    let v = s.execute("shape.inspect", &json!({"id": id})).unwrap();
    assert_eq!(v["box"]["x"], 120.0);
    assert_eq!(v["box"]["y"], 110.0);
    // One undo step per gesture.
    s.execute("edit.undo", &json!({})).unwrap();
    let v = s.execute("shape.inspect", &json!({"id": id})).unwrap();
    assert_eq!(v["box"]["x"], 100.0);
    // Resize from the SE handle.
    s.execute("edit.select", &json!({"ids": [id]})).unwrap();
    s.pointer(ev(PointerKind::Down, 200.0, 180.0)).unwrap();
    s.pointer(ev(PointerKind::Drag, 250.0, 200.0)).unwrap();
    s.pointer(ev(PointerKind::Up, 250.0, 200.0)).unwrap();
    let v = s.execute("shape.inspect", &json!({"id": id})).unwrap();
    assert_eq!(v["box"]["w"], 150.0);
}

#[test]
fn click_into_placeholder_starts_editing() {
    let mut s = session();
    let ev = |kind, x, y| PointerEvent { kind, x, y, mods: Mods::default(), tol: 3.0 };
    s.pointer(ev(PointerKind::Down, 480.0, 200.0)).unwrap();
    s.pointer(ev(PointerKind::Up, 480.0, 200.0)).unwrap();
    assert!(s.doc().unwrap().selection.text.is_some());
    s.execute("text.insert", &json!({"text": "Typed"})).unwrap();
    assert_eq!(s.doc().unwrap().current_slide().unwrap().title(), "Typed");
}

#[test]
fn sample_deck_builds_and_renders() {
    let mut s = Session::new();
    sample::open_sample(&mut s).unwrap();
    let st = s.doc().unwrap();
    assert!(st.doc.slides.len() >= 8);
    assert!(st.doc.validate().is_empty());
    assert!(!st.is_dirty());
    for i in 0..st.doc.slides.len() {
        let img = deckcraft_render::render_slide(&st.doc, i, &deckcraft_render::RenderOpts { scale: 0.2, ..Default::default() });
        assert_eq!(img.width, 192);
    }
    let v = s.execute("document.inspect", &json!({})).unwrap();
    assert!(v["slides"].as_array().unwrap().len() >= 8);
}

#[test]
fn save_and_reopen_native() {
    let mut s = Session::new();
    sample::open_sample(&mut s).unwrap();
    let bytes = s.execute("file.saveBytes", &json!({})).unwrap()["data"].as_str().unwrap().to_string();
    let n = s.doc().unwrap().doc.slides.len();
    s.execute("file.openBytes", &json!({"name": "x.deckcraft", "data": bytes})).unwrap();
    assert_eq!(s.doc().unwrap().doc.slides.len(), n);
    assert_eq!(s.documents().len(), 2);
}

#[test]
fn every_command_survives_empty_and_hostile_params() {
    let mut s = Session::new();
    sample::open_sample(&mut s).unwrap();
    s.execute("edit.select", &json!({"ids": [s.doc().unwrap().current_slide().unwrap().shapes[0].id.0]})).unwrap();
    let ids: Vec<&str> = command_specs()
        .iter()
        .map(|c| c.id)
        .filter(|id| !matches!(*id, "file.close" | "file.open" | "file.save" | "file.saveAs" | "file.export" | "file.saveTemplate"))
        .collect();
    for p in [
        json!({}),
        json!(null),
        json!([1, 2]),
        json!({"index": 99999, "ids": [999999], "id": -1, "text": "x", "rect": "bad", "color": 5, "size": 1e308}),
    ] {
        for id in &ids {
            let _ = s.execute(id, &p);
        }
    }
    assert!(s.active().is_some());
    for d in s.documents() {
        assert!(d.doc.validate().is_empty(), "{:?}", d.doc.validate());
    }
}

#[test]
fn base64_roundtrip() {
    for data in [vec![], vec![0u8], vec![1, 2], vec![1, 2, 3], (0..=255u8).collect::<Vec<_>>()] {
        assert_eq!(cmd::base64_decode(&cmd::base64_encode(&data)).unwrap(), data);
    }
    assert!(cmd::base64_decode("!!").is_none());
}

#[test]
fn layout_change_keeps_text() {
    let mut s = session();
    s.execute("slide.new", &json!({"layout": "titleAndContent", "title": "T", "body": "B"})).unwrap();
    s.execute("slide.layout", &json!({"layout": "twoContent"})).unwrap();
    let v = s.execute("slide.inspect", &json!({})).unwrap();
    let texts: Vec<String> = v["shapes"].as_array().unwrap().iter().filter_map(|x| x["text"].as_str().map(String::from)).collect();
    assert!(texts.contains(&"T".to_string()) && texts.contains(&"B".to_string()), "{v}");
    assert_eq!(v["layout"], "Two Content");
}

#[test]
fn themes_and_slide_size() {
    let mut s = session();
    s.execute("design.theme", &json!({"name": "Ember"})).unwrap();
    assert_eq!(s.doc().unwrap().doc.masters[0].theme.name, "Ember");
    s.execute("design.slideSize", &json!({"preset": "standard"})).unwrap();
    assert_eq!(s.doc().unwrap().doc.slide_size.width, 720.0);
    s.execute("design.headerFooter", &json!({"slideNumber": true, "footer": true, "footerText": "Hello"})).unwrap();
    let v = s.execute("slide.inspect", &json!({})).unwrap();
    assert!(v["shapes"].as_array().unwrap().iter().any(|x| x["placeholder"] == "sldNum"));
}

fn line_ends(s: &Session, id: u64) -> (deckcraft_geom::Point, deckcraft_geom::Point) {
    let st = s.doc().unwrap();
    let sh = st.shape(deckcraft_model::ShapeId(id as u32)).unwrap();
    connect::endpoints(&cmd::xfrm_of(&st.doc, &st.selection, sh))
}

#[test]
fn connectors_follow_glued_shapes() {
    let mut s = session();
    let a = s.execute("shape.insert", &json!({"preset": "rect", "rect": [100, 100, 100, 50]})).unwrap()["id"].as_u64().unwrap();
    let b = s.execute("shape.insert", &json!({"preset": "rect", "rect": [400, 300, 100, 50]})).unwrap()["id"].as_u64().unwrap();
    let r = s.execute("shape.connect", &json!({"from": a, "to": b, "preset": "bentConnector3"})).unwrap();
    let c = r["id"].as_u64().unwrap();
    let (p0, p1) = line_ends(&s, c);
    // Closest pair: right side of A → left side of B... or bottom/top; either way on the boxes.
    let sites_a: Vec<Vec<f64>> = serde_json::from_value(s.execute("shape.sites", &json!({"id": a})).unwrap()).unwrap();
    assert!(sites_a.iter().any(|q| (q[0] - p0.x).abs() < 1e-6 && (q[1] - p0.y).abs() < 1e-6));
    // Move B: the connector's end follows, its start stays.
    s.execute("edit.select", &json!({"ids": [b]})).unwrap();
    s.execute("shape.move", &json!({"dx": 50, "dy": 20})).unwrap();
    let (q0, q1) = line_ends(&s, c);
    assert!((q0 - p0).hypot() < 1e-6);
    assert!(((q1 - p1) - deckcraft_geom::Vec2::new(50.0, 20.0)).hypot() < 1e-6, "{p1:?} → {q1:?}");
    // Undo puts both back in one step.
    s.execute("edit.undo", &json!({})).unwrap();
    assert!((line_ends(&s, c).1 - p1).hypot() < 1e-6);
    // Deleting a glued shape unglues that end and leaves the line where it was.
    s.execute("edit.select", &json!({"ids": [a]})).unwrap();
    s.execute("edit.delete", &json!({})).unwrap();
    let st = s.doc().unwrap();
    let sh = st.shape(deckcraft_model::ShapeId(c as u32)).unwrap();
    assert!(matches!(sh.kind, deckcraft_model::ShapeKind::Connector { start: None, end: Some(_) }));
}

#[test]
fn drawing_a_line_between_shapes_glues_it() {
    let mut s = session();
    let a = s.execute("shape.insert", &json!({"preset": "ellipse", "rect": [100, 100, 100, 100]})).unwrap()["id"].as_u64().unwrap();
    let b = s.execute("shape.insert", &json!({"preset": "rect", "rect": [400, 100, 100, 100]})).unwrap()["id"].as_u64().unwrap();
    s.execute("edit.deselect", &json!({})).unwrap();
    s.tool.kind = tools::ToolKind::Shape { preset: "straightConnector1".into() };
    let ev = |kind, x: f64, y: f64| tools::PointerEvent { kind, x, y, mods: Default::default(), tol: 4.0 };
    // Right site of the ellipse (200,150) → left site of the rect (400,150), a little off.
    s.pointer(ev(tools::PointerKind::Down, 202.0, 151.0)).unwrap();
    s.pointer(ev(tools::PointerKind::Drag, 300.0, 150.0)).unwrap();
    assert!(!s.tool.sites.is_empty() || s.tool.glue.is_none());
    s.pointer(ev(tools::PointerKind::Drag, 398.0, 149.0)).unwrap();
    assert!(s.tool.glue.is_some());
    s.pointer(ev(tools::PointerKind::Up, 398.0, 149.0)).unwrap();
    let st = s.doc().unwrap();
    let line = st.shapes().iter().find(|x| x.is_line()).unwrap();
    match line.kind {
        deckcraft_model::ShapeKind::Connector { start: Some((x, _)), end: Some((y, _)) } => {
            assert_eq!((x.0 as u64, y.0 as u64), (a, b));
        }
        ref k => panic!("not glued: {k:?}"),
    }
    let id = line.id.0 as u64;
    let (p0, p1) = line_ends(&s, id);
    assert!((p0.x - 200.0).abs() < 1e-6 && (p1.x - 400.0).abs() < 1e-6, "{p0:?} {p1:?}");
}

#[test]
fn freeform_tools_and_command() {
    let mut s = session();
    let before = s.doc().unwrap().shapes().len();
    // Composite command: one undo step.
    let r = s.execute("shape.freeform", &json!({"points": [[100, 100], [200, 120], [150, 220]], "closed": true})).unwrap();
    assert!(r["id"].as_u64().is_some());
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(s.doc().unwrap().shapes().len(), before);
    s.execute("edit.redo", &json!({})).unwrap();
    assert_eq!(s.doc().unwrap().shapes().len(), before + 1);

    let ev = |kind, x: f64, y: f64| tools::PointerEvent { kind, x, y, mods: Default::default(), tol: 3.0 };
    // Polygon: click, click, click, click on the start → closed, filled.
    s.set_tool(tools::ToolKind::Shape { preset: "freeform".into() });
    for (x, y) in [(300.0, 300.0), (400.0, 300.0), (400.0, 400.0)] {
        s.pointer(ev(tools::PointerKind::Down, x, y)).unwrap();
        s.pointer(ev(tools::PointerKind::Up, x, y)).unwrap();
    }
    assert!(s.tool.drawing_freeform());
    s.pointer(ev(tools::PointerKind::Down, 301.0, 301.0)).unwrap();
    assert!(!s.tool.drawing_freeform());
    let st = s.doc().unwrap();
    let sh = st.shapes().last().unwrap();
    match &sh.geom {
        deckcraft_model::Geom::Custom { paths } => assert!(paths[0].d.ends_with('Z') && paths[0].d.matches(" L ").count() == 2, "{}", paths[0].d),
        g => panic!("{g:?}"),
    }
    let x = sh.xfrm.unwrap();
    assert_eq!((x.x, x.y, x.w, x.h), (300.0, 300.0, 100.0, 100.0));

    // Scribble: one drag, open path.
    s.set_tool(tools::ToolKind::Shape { preset: "scribble".into() });
    s.pointer(ev(tools::PointerKind::Down, 10.0, 10.0)).unwrap();
    for i in 1..20 {
        s.pointer(ev(tools::PointerKind::Drag, 10.0 + i as f64 * 5.0, 10.0 + (i as f64).sin() * 10.0)).unwrap();
    }
    let r = s.pointer(ev(tools::PointerKind::Up, 110.0, 10.0)).unwrap();
    assert!(r["id"].as_u64().is_some());
    assert_eq!(s.tool.kind, tools::ToolKind::Select);

    // Curve: clicks then Enter → smooth open path.
    s.set_tool(tools::ToolKind::Shape { preset: "curve".into() });
    for (x, y) in [(500.0, 100.0), (550.0, 50.0), (600.0, 100.0), (650.0, 50.0)] {
        s.pointer(ev(tools::PointerKind::Down, x, y)).unwrap();
        s.pointer(ev(tools::PointerKind::Up, x, y)).unwrap();
    }
    let r = s.key("Enter", Default::default()).unwrap();
    let id = deckcraft_model::ShapeId(r["id"].as_u64().unwrap() as u32);
    let st = s.doc().unwrap();
    match &st.shape(id).unwrap().geom {
        deckcraft_model::Geom::Custom { paths } => assert_eq!(paths[0].d.matches(" C ").count(), 3),
        g => panic!("{g:?}"),
    }
}

#[test]
fn double_click_in_table_uses_grown_row_heights() {
    // Rows stored 4 pt tall are drawn tall enough for their text, so 6 pt down is still row 0.
    let mut s = session();
    let data = json!([["a", "b"], ["c", "d"], ["e", "f"]]);
    let id = s.execute("insert.table", &json!({"rows": 3, "cols": 2, "rect": [100.0, 100.0, 400.0, 120.0], "data": data})).unwrap()["id"].clone();
    for row in 0..3 {
        s.execute("table.rowHeight", &json!({"id": id, "row": row, "height": 4.0})).unwrap();
    }
    let ev = |kind, x, y| PointerEvent { kind, x, y, mods: Mods::default(), tol: 3.0 };
    s.pointer(ev(PointerKind::DoubleClick, 120.0, 106.0)).unwrap();
    assert_eq!(s.doc().unwrap().selection.text.as_ref().and_then(|t| t.cell), Some((0, 0)));
}

#[test]
fn grown_table_frame_covers_its_drawn_rows() {
    // Rows stored 4 pt tall are drawn taller: the box, its handles and hit tests follow the drawn rows.
    let mut s = session();
    let data = json!([["a", "b"], ["c", "d"], ["e", "f"]]);
    let id = s.execute("insert.table", &json!({"rows": 3, "cols": 2, "rect": [100.0, 100.0, 400.0, 120.0], "data": data})).unwrap()["id"].clone();
    for row in 0..3 {
        s.execute("table.rowHeight", &json!({"id": id, "row": row, "height": 4.0})).unwrap();
    }
    let st = s.doc().unwrap();
    let sh = st.shapes().iter().find(|sh| json!(sh.id) == id).unwrap().clone();
    let deckcraft_model::ShapeKind::Table(t) = &sh.kind else { panic!("not a table") };
    let drawn: f64 = tools::table_rows(st, t).iter().sum();
    assert!(drawn > 12.0, "rows should grow: {drawn}");
    let x = cmd::xfrm_of(&st.doc, &st.selection, &sh);
    assert!((x.h - drawn).abs() < 1e-6, "frame {} vs drawn {drawn}", x.h);
    let below_stored = deckcraft_geom::Point::new(120.0, 100.0 + drawn - 2.0);
    assert!(matches!(s.hit_test(below_stored, 1.0), Some(tools::Hit::Shape { .. })));
}

#[test]
fn right_to_left_paragraphs() {
    let mut s = session();
    let title = s.doc().unwrap().current_slide().unwrap().shapes[0].id.0;
    s.execute("text.edit", &json!({"id": title})).unwrap();
    s.execute("text.insert", &json!({"text": "سنة 2024 (Retrait) فقط"})).unwrap();
    s.execute("format.rtl", &json!({})).unwrap();
    let para = |s: &Session| s.doc().unwrap().current_slide().unwrap().shapes[0].text.as_ref().unwrap().paragraphs[0].props.clone();
    assert_eq!(para(&s).rtl, Some(true));
    // Left (the default) becomes right: the text stays at its start edge.
    assert_eq!(para(&s).align, Some(deckcraft_model::text::Align::Right));
    s.execute("format.align", &json!({"align": "justifyLow"})).unwrap();
    assert_eq!(para(&s).align, Some(deckcraft_model::text::Align::JustLow));
    // Toggling back keeps a justified alignment.
    s.execute("format.rtl", &json!({})).unwrap();
    assert_eq!(para(&s).rtl, Some(false));
    assert_eq!(para(&s).align, Some(deckcraft_model::text::Align::JustLow));
    s.execute("format.rtl", &json!({"on": true})).unwrap();
    s.execute("format.alignLeft", &json!({})).unwrap();
    s.execute("format.rtl", &json!({"on": false})).unwrap();
    assert_eq!(para(&s).align, Some(deckcraft_model::text::Align::Right));
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(para(&s).rtl, Some(true));
}

#[test]
fn header_footer_fixed_date_is_the_slides_own_text() {
    use deckcraft_model::text::RunKind;
    let mut s = session();
    let date = |s: &Session| {
        let slide = s.doc().unwrap().current_slide().unwrap();
        slide.shapes.iter().find(|x| x.ph_type() == Some(deckcraft_model::PhType::Date)).and_then(|x| x.text.clone()).unwrap()
    };
    let kind = |s: &Session| date(s).paragraphs[0].runs[0].kind.clone();
    // Automatic: a date field, saved with today's date.
    s.execute("design.headerFooter", &json!({"date": true})).unwrap();
    assert_eq!(kind(&s), RunKind::Field { field: "datetime1".into() });
    assert_eq!(date(&s).text().matches('/').count(), 2, "M/D/YYYY");
    // Fixed: plain text in the slide's own placeholder (other slides' fields aren't affected).
    s.execute("design.headerFooter", &json!({"date": true, "dateText": "Launch day"})).unwrap();
    assert_eq!((kind(&s), date(&s).text()), (RunKind::Text, "Launch day".to_string()));
    // Back to automatic.
    s.execute("design.headerFooter", &json!({"date": true, "dateText": ""})).unwrap();
    assert!(matches!(kind(&s), RunKind::Field { .. }));
    // A new date field is saved with the date it shows.
    s.execute("text.edit", &json!({"id": s.doc().unwrap().current_slide().unwrap().shapes[0].id.0})).unwrap();
    s.execute("text.insert", &json!({"text": "Updated "})).unwrap();
    s.execute("insert.dateTime", &json!({"format": "datetime10"})).unwrap();
    let title = s.doc().unwrap().current_slide().unwrap().shapes[0].text.clone().unwrap();
    let run = title.paragraphs[0].runs.iter().find(|r| matches!(r.kind, RunKind::Field { .. })).cloned().unwrap();
    assert!(run.text.contains(':'), "H:mm {run:?}");
}

/// (text, linked?) of each run in the first paragraph of the current slide's title.
fn title_runs(s: &Session) -> Vec<(String, bool)> {
    let sh = &s.doc().unwrap().current_slide().unwrap().shapes[0];
    let p = &sh.text.as_ref().unwrap().paragraphs[0];
    p.runs.iter().map(|r| (r.text.clone(), r.props.link.is_some())).collect()
}

#[test]
fn hyperlink_with_a_bare_caret_inserts_the_address() {
    // #31: with the caret at the end of the text, Insert Hyperlink used to format an empty range
    // and change nothing.
    let mut s = session();
    let title = s.doc().unwrap().current_slide().unwrap().shapes[0].id.0;
    s.execute("text.edit", &json!({"id": title})).unwrap();
    s.execute("text.insert", &json!({"text": "See "})).unwrap();
    s.execute("insert.hyperlink", &json!({"url": "https://example.org"})).unwrap();
    s.execute("text.insert", &json!({"text": " now"})).unwrap();
    assert_eq!(
        title_runs(&s),
        vec![("See ".into(), false), ("https://example.org".into(), true), (" now".into(), false)],
        "the address is inserted as a link, and typing after it doesn't extend it"
    );
}

#[test]
fn hyperlink_display_text_and_targets() {
    let mut s = session();
    let title = s.doc().unwrap().current_slide().unwrap().shapes[0].id.0;
    s.execute("text.edit", &json!({"id": title})).unwrap();
    s.execute("insert.hyperlink", &json!({"url": "mailto:a@b.org"})).unwrap();
    assert_eq!(title_runs(&s), vec![("a@b.org".into(), true)], "email links show the address without mailto:");
    s.execute("text.insert", &json!({"text": " "})).unwrap();
    s.execute("insert.hyperlink", &json!({"slide": 0, "text": "start"})).unwrap();
    assert_eq!(title_runs(&s), vec![("a@b.org".into(), true), (" ".into(), false), ("start".into(), true)]);
}

#[test]
fn hyperlink_on_a_word_or_shape_links_it() {
    let mut s = session();
    let title = s.doc().unwrap().current_slide().unwrap().shapes[0].id.0;
    s.execute("text.edit", &json!({"id": title})).unwrap();
    s.execute("text.insert", &json!({"text": "Hello world"})).unwrap();
    s.execute("text.move", &json!({"to": "left"})).unwrap();
    s.execute("insert.hyperlink", &json!({"url": "https://example.org"})).unwrap();
    assert_eq!(title_runs(&s), vec![("Hello ".into(), false), ("world".into(), true)], "a caret inside a word links the word");
    s.execute("text.exit", &json!({})).unwrap();
    s.execute("insert.hyperlink", &json!({"url": "https://example.org"})).unwrap();
    assert!(s.doc().unwrap().current_slide().unwrap().shapes[0].click.is_some(), "a selected shape gets the link as its click action");
}

#[test]
fn clicks_in_flipped_shapes_hit_the_text_where_it_is_drawn() {
    // Flips don't mirror text, so a click lands on the character drawn under it.
    let mut s = session();
    let r = s.execute("shape.insert", &json!({"preset": "rect", "rect": [100, 100, 400, 100], "text": "Hello flipped world"})).unwrap();
    let id = deckcraft_model::ShapeId(r["id"].as_u64().unwrap() as u32);
    let pos = |s: &Session, x: f64, y: f64| {
        let st = s.doc().unwrap();
        tools::text_pos(st, st.shape(id).unwrap(), deckcraft_geom::Point::new(x, y))
    };
    let xs = [240.0, 300.0, 360.0];
    let plain = xs.map(|x| pos(&s, x, 150.0));
    assert!(plain[0].1 < plain[1].1 && plain[1].1 < plain[2].1, "{plain:?}");
    s.execute("shape.flip", &json!({"axis": "horizontal"})).unwrap();
    assert_eq!(xs.map(|x| pos(&s, x, 150.0)), plain);
    // flipV turns the text 180° about the box centre (300, 150).
    s.execute("shape.flip", &json!({"axis": "horizontal"})).unwrap();
    s.execute("shape.flip", &json!({"axis": "vertical"})).unwrap();
    assert_eq!(xs.map(|x| pos(&s, 600.0 - x, 150.0)), plain);
}

#[test]
fn inserted_hyperlinks_look_like_links() {
    let mut s = session();
    let title = s.doc().unwrap().current_slide().unwrap().shapes[0].id.0;
    s.execute("text.edit", &json!({"id": title})).unwrap();
    s.execute("text.insert", &json!({"text": "Read the docs"})).unwrap();
    s.execute("text.move", &json!({"to": "wordLeft", "extend": true})).unwrap();
    assert_eq!(s.execute("format.state", &json!({})).unwrap()["underline"], false);
    s.execute("insert.hyperlink", &json!({"url": "https://example.org"})).unwrap();
    // Underlined (and hlink-coloured: same resolution as files) without touching the run's own props.
    assert_eq!(s.execute("format.state", &json!({})).unwrap()["underline"], true);
    let body = s.doc().unwrap().current_slide().unwrap().shapes[0].text.clone().unwrap();
    let run = body.paragraphs[0].runs.iter().find(|r| r.props.link.is_some()).unwrap();
    assert!(run.props.fill.is_none() && run.props.underline.is_none(), "{run:?}");
    // Underline off is the run's own u="none", as in PowerPoint.
    s.execute("format.underline", &json!({})).unwrap();
    assert_eq!(s.execute("format.state", &json!({})).unwrap()["underline"], false);
}

fn said(s: &mut Session, id: &str, p: serde_json::Value) -> String {
    match s.call(id, &p) {
        Ok(serde_json::Value::String(t)) => t,
        Ok(v) => v.to_string(),
        Err(e) => e.to_string(),
    }
}

fn insert(s: &mut Session, text: &str) -> u64 {
    s.call("shape.insert", &json!({"preset": "roundRect", "rect": [80, 80, 300, 120], "text": text})).unwrap()["id"].as_u64().unwrap()
}

#[test]
fn every_params_doc_names_its_keys() {
    for c in command_specs() {
        assert!(cmd::param_keys(c.params).is_ok(), "{}: {:?}", c.id, cmd::param_keys(c.params));
    }
    assert_eq!(cmd::param_keys("{index} | {id}").unwrap(), ["index", "id"]);
    assert_eq!(cmd::param_keys("{color? | none?: true, kind?: a|b (x, y), ids?} → {id}").unwrap(), ["color", "none", "kind", "ids"]);
}

#[test]
fn a_params_doc_spells_out_each_parameter() {
    use cmd::Param;
    let p = |name, optional, about, or, object| Param { name, optional, about, or, object };
    assert_eq!(
        cmd::parse_params("{preset: rect|roundRect, rect?: [x,y,w,h] pt, index? (else selected shapes)} → {id}").unwrap(),
        [
            p("preset", false, "rect|roundRect", false, 0),
            p("rect", true, "[x,y,w,h] pt", false, 0),
            p("index", true, "(else selected shapes)", false, 0)
        ]
    );
    assert_eq!(
        cmd::parse_params("{color? | none?: true, gradient?: {stops, kind?: a|b}, ids?}").unwrap(),
        [
            p("color", true, "", false, 0),
            p("none", true, "true", true, 0),
            p("gradient", true, "{stops, kind?: a|b}", false, 0),
            p("ids", true, "", false, 0)
        ]
    );
    assert_eq!(
        cmd::parse_params("{name} | {major, minor}").unwrap(),
        [p("name", false, "", false, 0), p("major", false, "", false, 1), p("minor", false, "", false, 1)]
    );
}

#[test]
fn a_command_not_available_names_only_keys_it_takes() {
    let fresh = Session::with_new();
    for c in command_specs() {
        let Err(reason) = (c.enabled)(&fresh) else { continue };
        let keys = cmd::param_keys(c.params).unwrap();
        let err = EngineError::Disabled(c.id.into(), reason).to_string();
        assert!(!err.contains("`ids`") || keys.contains(&"ids"), "{err}");
        assert!(!err.contains("`id`") || keys.contains(&"id"), "{err}");
    }
    let mut s = session();
    assert_eq!(
        said(&mut s, "shape.setDefault", json!({})),
        "command `shape.setDefault` is not available right now: nothing selected. Acts on the selected shapes. edit.select {\"ids\": […]} selects shapes."
    );
    assert_eq!(
        said(&mut s, "table.style", json!({"style": "grid"})),
        "command `table.style` is not available right now: nothing selected. Acts on the shape in `id`, else on the selected shapes. \
         edit.select {\"ids\": […]} selects shapes."
    );
    assert_eq!(
        said(&mut s, "arrange.group", json!({})),
        "command `arrange.group` is not available right now: select two or more objects. Acts on two or more shapes: the shapes in \
         `ids`, else the selected ones. edit.select {\"ids\": […]} selects shapes."
    );
}

#[test]
fn what_a_command_acts_on_names_only_keys_it_takes() {
    for c in command_specs() {
        let keys = cmd::param_keys(c.params).unwrap();
        let said = c.acts_on().unwrap_or_default();
        assert!(!said.contains("`ids`") || keys.contains(&"ids"), "{}: {said}", c.id);
        assert!(!said.contains("`id`") || keys.contains(&"id"), "{}: {said}", c.id);
    }
    let acts = |id: &str| find_command(id).and_then(|c| c.acts_on());
    assert_eq!(acts("shape.resize").as_deref(), Some("Acts on the shapes in `ids`, else on the selected shapes."));
    assert_eq!(acts("table.style").as_deref(), Some("Acts on the shape in `id`, else on the selected shapes."));
    assert_eq!(acts("shape.setDefault").as_deref(), Some("Acts on the selected shapes."));
    assert_eq!(acts("format.size").as_deref(), Some("Acts on the selected text or shapes."));
    assert_eq!(acts("slide.new"), None);
}

#[test]
fn a_call_refuses_parameters_the_command_does_not_take() {
    let mut s = session();
    assert_eq!(
        said(&mut s, "shape.insert", json!({"shape": "roundRect", "rect": [0, 0, 10, 10]})),
        "`shape.insert` was given `shape`, which it doesn't take. `shape.insert` takes {preset: \
         rect|roundRect|ellipse|triangle|rightArrow|star5|… (see shape.presets), rect: [x,y,w,h] pt, text?: string, \
         adj?: [number] (preset adjustments), flip?: h|v|hv}."
    );
    assert_eq!(
        said(&mut s, "slide.next", json!({"index": 1})),
        "`slide.next` was given `index`, which it doesn't take. `slide.next` takes no parameters."
    );
    assert_eq!(
        said(&mut s, "slide.move", json!({"from": 0})),
        "invalid parameters for `slide.move`: missing `to`. `slide.move` takes {from?: index, to: index}."
    );
    assert!(
        said(&mut s, "slide.new", json!([1]))
            .starts_with("invalid parameters for `slide.new`: parameters are a JSON object, not an array. `slide.new` takes {layout?: ")
    );
    assert_eq!(s.doc().unwrap().current_slide().unwrap().shapes.len(), 2, "a refused call inserts nothing");
    let id = insert(&mut s, "Hi");
    assert_eq!(said(&mut s, "shape.resize", json!({"id": id, "w": 50})), format!("changed shape {id} on slide index 0"));
    assert_eq!(
        said(&mut s, "shape.inspect", json!({"ids": [id]})),
        "`shape.inspect` was given `ids`, which it doesn't take. `shape.inspect` takes {id} → full shape JSON plus its effective box."
    );
}

#[test]
fn an_unknown_command_names_the_commands_like_it() {
    let mut s = session();
    assert_eq!(said(&mut s, "new", json!({})), "unknown command `new`; commands named like it: file.new, slide.new");
    assert_eq!(said(&mut s, "shape.siz", json!({})), "unknown command `shape.siz`; commands named like it: format.size");
    assert_eq!(said(&mut s, "zzz", json!({})), "unknown command `zzz`");
}

#[test]
fn a_shape_off_the_current_slide_is_reported_where_it_is() {
    let mut s = session();
    let id = insert(&mut s, "Hi");
    s.call("slide.new", &json!({})).unwrap();
    let off_slide = format!(
        "shape {id} is on slide index 0, but shape ids are looked up on slide index 1, the one being edited; \
         slide.go {{\"index\": 0}} switches to slide index 0"
    );
    for (cmd, p) in [
        ("shape.resize", json!({"ids": [id], "w": 50})),
        ("edit.select", json!({"ids": [id]})),
        ("edit.delete", json!({"ids": [id]})),
        ("arrange.bringToFront", json!({"id": id})),
        ("arrange.reorder", json!({"id": id, "index": 0})),
        ("table.selectCells", json!({"id": id})),
        ("comment.add", json!({"text": "x", "id": id})),
        ("shape.connect", json!({"from": id, "to": id})),
        ("animation.painter", json!({"id": id})),
    ] {
        assert_eq!(said(&mut s, cmd, p), off_slide, "{cmd}");
    }
    let missing = "no shape 9999 in the presentation; slide.inspect lists the shapes of slide index 1, the one being edited";
    let line = insert(&mut s, "Box");
    for (cmd, p) in [
        ("arrange.reorder", json!({"id": 9999, "index": 0})),
        ("table.selectCells", json!({"id": 9999})),
        ("comment.add", json!({"text": "x", "id": 9999})),
        ("shape.connect", json!({"from": line, "to": 9999})),
        ("shape.connect", json!({"from": line, "to": line, "id": 9999})),
        ("animation.painter", json!({"id": 9999})),
    ] {
        assert_eq!(said(&mut s, cmd, p), missing, "{cmd}");
    }
    let st = s.doc().unwrap();
    assert!(st.doc.slides[1].comments.is_empty() && st.selection.cells.is_none(), "a refused call changes nothing");
    assert_eq!(
        said(&mut s, "shape.inspect", json!({"id": 9999})),
        "invalid parameters for `shape.inspect`: no shape 9999 in the presentation; slide.inspect lists the shapes of slide index 1, \
         the one being edited. `shape.inspect` takes {id} → full shape JSON plus its effective box."
    );
    let on_master = s.doc().unwrap().doc.masters[0].shapes[0].id;
    assert_eq!(
        said(&mut s, "shape.move", json!({"id": on_master.0, "dx": 1})),
        format!(
            "shape {on_master} is on slide master 0, but shape ids are looked up on slide index 1, the one being edited; \
             view.slideMaster {{\"master\": 0}} switches to slide master 0"
        )
    );
    s.call("slide.go", &json!({"index": 0})).unwrap();
    let before = s.doc().unwrap().doc.clone();
    assert_eq!(
        said(&mut s, "shape.move", json!({"ids": [id, 9999], "dx": 10})),
        "no shape 9999 in the presentation; slide.inspect lists the shapes of slide index 0, the one being edited"
    );
    assert_eq!(s.doc().unwrap().doc, before);
}

#[test]
fn a_call_that_returns_nothing_says_what_changed() {
    let mut s = session();
    let id = insert(&mut s, "Revenue");
    assert_eq!(said(&mut s, "format.size", json!({"size": 28})), format!("changed shape {id} on slide index 0"));
    assert_eq!(said(&mut s, "design.background", json!({"color": "#112233"})), "changed the background of slide index 0");
    assert_eq!(said(&mut s, "text.edit", json!({"at": [0, 3]})), format!("editing the text of shape {id}, paragraph 0, character 3"));
    assert_eq!(said(&mut s, "text.exit", json!({})), "stopped editing text");
    assert_eq!(said(&mut s, "edit.deselect", json!({})), "no shape is selected");
    assert_eq!(said(&mut s, "edit.deselect", json!({})), "nothing changed");
    s.call("slide.new", &json!({})).unwrap();
    assert_eq!(said(&mut s, "slide.notes", json!({"text": "Say hi", "index": 0})), "changed the notes of slide index 0");
    s.call("slide.new", &json!({})).unwrap();
    assert_eq!(said(&mut s, "design.background", json!({"color": "#445566", "all": true})), "changed the background of every slide");
    assert_eq!(said(&mut s, "design.background", json!({"reset": true, "index": 1})), "changed the background of slide index 1");
    assert_eq!(
        said(&mut s, "design.background", json!({"color": "#445566", "all": true})),
        "changed the background of slide index 1",
        "only the slide that was reset changes"
    );
}

fn template() -> Session {
    let mut s = Session::new();
    s.execute("file.new", &json!({"blank": true})).unwrap();
    s
}

fn placeholder(s: &Session, kind: deckcraft_model::PhType) -> u64 {
    s.doc().unwrap().shapes().iter().find(|sh| sh.ph_type() == Some(kind)).unwrap().id.0.into()
}

fn slide_title_state(s: &mut Session) -> serde_json::Value {
    let title = placeholder(s, deckcraft_model::PhType::Title);
    s.execute("edit.select", &json!({"ids": [title]})).unwrap();
    s.execute("format.state", &json!({})).unwrap()
}

#[test]
fn formatting_a_master_placeholder_sets_the_text_style_slides_inherit() {
    use deckcraft_model::PhType;
    let mut s = template();
    s.execute("view.slideMaster", &json!({"master": 0})).unwrap();
    let title = placeholder(&s, PhType::Title);
    s.execute("edit.select", &json!({"ids": [title]})).unwrap();
    assert_eq!(said(&mut s, "format.size", json!({"size": 30})), "changed the title text style of slide master 0");
    s.execute("format.bold", &json!({"on": true})).unwrap();
    let master = &s.doc().unwrap().doc.masters[0];
    let style = master.title_style.level(0).unwrap();
    assert_eq!((style.run.size, style.run.bold), (Some(30.0), Some(true)));
    let prompt = master.shapes.iter().find(|sh| sh.ph_type() == Some(PhType::Title)).unwrap();
    assert!(prompt.text.as_ref().unwrap().paragraphs.iter().flat_map(|p| &p.runs).all(|r| r.props.size.is_none()));

    s.execute("view.closeMaster", &json!({})).unwrap();
    s.execute("slide.new", &json!({"layout": "titleAndContent", "title": "Q3"})).unwrap();
    let state = slide_title_state(&mut s);
    assert_eq!((state["size"].as_f64(), state["bold"].as_bool()), (Some(30.0), Some(true)), "{state}");
}

#[test]
fn formatting_a_layout_placeholder_sets_the_style_its_slides_inherit() {
    use deckcraft_model::PhType;
    let mut s = template();
    s.execute("view.slideMaster", &json!({"layout": 1})).unwrap();
    let title = placeholder(&s, PhType::Title);
    s.execute("edit.select", &json!({"ids": [title]})).unwrap();
    assert_eq!(said(&mut s, "format.font", json!({"family": "Acme Sans"})), format!("changed shape {title} on layout 1 of slide master 0"));
    s.execute("format.alignCenter", &json!({})).unwrap();
    s.execute("view.closeMaster", &json!({})).unwrap();
    s.execute("slide.new", &json!({"layout": "titleAndContent", "title": "Q3"})).unwrap();
    let state = slide_title_state(&mut s);
    assert_eq!((state["font"].as_str(), state["align"].as_str()), (Some("Acme Sans"), Some("ctr")), "{state}");
}

#[test]
fn indenting_in_a_master_placeholder_still_moves_its_paragraphs() {
    let mut s = template();
    s.execute("view.slideMaster", &json!({"master": 0})).unwrap();
    let body = placeholder(&s, deckcraft_model::PhType::Body);
    s.execute("text.edit", &json!({"id": body, "at": [0, 0]})).unwrap();
    s.execute("format.indent", &json!({})).unwrap();
    let st = s.doc().unwrap();
    assert_eq!(st.shape(deckcraft_model::ShapeId(body as u32)).unwrap().text.as_ref().unwrap().paragraphs[0].level, 1);
}

#[test]
fn naming_the_slide_master_opens_it_even_beside_slides() {
    let mut s = session();
    assert_eq!(said(&mut s, "view.slideMaster", json!({"master": 0})), r#"{"kind":"master","master":0}"#);
    assert_eq!(said(&mut s, "view.slideMaster", json!({})), r#"{"kind":"layout","layout":0,"master":0}"#);
}

#[test]
fn a_layout_can_hide_the_slide_master_graphics() {
    let mut s = template();
    s.execute("view.slideMaster", &json!({"layout": 2})).unwrap();
    assert_eq!(said(&mut s, "design.hideBackgroundGraphics", json!({"hide": true})), "changed the background graphics of layout 2 of slide master 0");
    assert!(!s.doc().unwrap().doc.masters[0].layouts[2].show_master_shapes);
    s.execute("view.slideMaster", &json!({"master": 0})).unwrap();
    assert_eq!(
        said(&mut s, "design.hideBackgroundGraphics", json!({"hide": true})),
        "invalid parameters for `design.hideBackgroundGraphics`: a slide master's graphics are its own; view.slideMaster {\"master\": 0, \
         \"layout\": index} selects one of its layouts. `design.hideBackgroundGraphics` takes {hide?: bool}."
    );
}

#[test]
fn a_layout_takes_subtitle_date_footer_and_slide_number_placeholders() {
    use deckcraft_model::PhType;
    let mut s = template();
    s.execute("view.slideMaster", &json!({"master": 0})).unwrap();
    s.execute("master.insertLayout", &json!({"name": "Cover"})).unwrap();
    let mut add = |p: serde_json::Value| s.execute("master.insertPlaceholder", &p).unwrap()["id"].as_u64().unwrap();
    let subtitle = add(json!({"kind": "subtitle", "rect": [80, 300, 800, 60], "prompt": "Add the quarter"}));
    let number = add(json!({"kind": "slideNumber"}));
    let footer = add(json!({"kind": "footer", "prompt": "Acme"}));
    let date = add(json!({"kind": "date"}));
    let st = s.doc().unwrap();
    let shape = |id: u64| st.shape(deckcraft_model::ShapeId(id as u32)).unwrap();
    let ph = |id: u64| shape(id).ph.clone().unwrap();
    assert_eq!((ph(subtitle).kind, ph(subtitle).has_custom_prompt), (PhType::SubTitle, true));
    assert_eq!(shape(subtitle).text.as_ref().unwrap().text(), "Add the quarter");
    assert_eq!((ph(number).kind, shape(number).xfrm), (PhType::SlideNum, None));
    assert!(shape(number).name.starts_with("Slide Number Placeholder "));
    assert_eq!((ph(footer).kind, ph(footer).has_custom_prompt), (PhType::Footer, false));
    assert_eq!(shape(footer).text.as_ref().unwrap().text(), "Acme");
    assert_eq!(ph(date).kind, PhType::Date);
    assert!(said(&mut s, "master.insertPlaceholder", json!({"kind": "logo"})).starts_with(
        "invalid parameters for `master.insertPlaceholder`: no placeholder kind `logo`; the kinds are content, text, picture, chart, \
         table, media, title, subtitle, date, footer, slideNumber."
    ));
}

#[test]
fn typing_into_a_layout_placeholder_makes_it_the_prompt() {
    let mut s = template();
    s.execute("view.slideMaster", &json!({"layout": 1})).unwrap();
    let content = placeholder(&s, deckcraft_model::PhType::Obj);
    s.execute("text.set", &json!({"id": content, "text": "Three points, no more"})).unwrap();
    let st = s.doc().unwrap();
    assert!(st.shape(deckcraft_model::ShapeId(content as u32)).unwrap().ph.as_ref().unwrap().has_custom_prompt);
}

#[test]
fn new_slides_carry_the_footers_the_template_turns_on() {
    use deckcraft_model::PhType;
    let mut s = template();
    s.execute("design.headerFooter", &json!({"slideNumber": true, "footerText": "Acme", "hideOnTitle": true})).unwrap();
    let master_footer = s.doc().unwrap().doc.masters[0].shapes.iter().find(|sh| sh.ph_type() == Some(PhType::Footer)).cloned().unwrap();
    assert_eq!(master_footer.text.unwrap().text(), "Acme");
    s.execute("slide.new", &json!({"layout": "title"})).unwrap();
    s.execute("slide.new", &json!({"layout": "titleAndContent"})).unwrap();
    let st = s.doc().unwrap();
    let kinds = |i: usize| st.doc.slides[i].shapes.iter().filter_map(|sh| sh.ph_type()).filter(|k| k.is_footer_kind()).collect::<Vec<_>>();
    assert_eq!(kinds(0), []);
    assert_eq!(kinds(1), [PhType::Footer, PhType::SlideNum]);
    let footer = st.doc.slides[1].shapes.iter().find(|sh| sh.ph_type() == Some(PhType::Footer)).unwrap();
    assert_eq!(footer.text.as_ref().unwrap().text(), "Acme");
}

#[test]
fn slides_from_an_outline_carry_the_footers_too() {
    use deckcraft_model::PhType;
    let mut s = template();
    s.execute("design.headerFooter", &json!({"slideNumber": true, "footerText": "Acme"})).unwrap();
    s.execute("slide.fromOutline", &json!({"text": "Q3\n\tRevenue\nQ4\n\tCosts"})).unwrap();
    let st = s.doc().unwrap();
    assert_eq!(st.doc.slides.len(), 2);
    for sl in &st.doc.slides {
        let kinds: Vec<_> = sl.shapes.iter().filter_map(|sh| sh.ph_type()).filter(|k| k.is_footer_kind()).collect();
        assert_eq!(kinds, [PhType::Footer, PhType::SlideNum]);
    }
}

#[test]
fn a_dark_background_style_on_a_layout_turns_its_text_light() {
    use deckcraft_color::SchemeSlot;
    let mut s = template();
    s.execute("view.slideMaster", &json!({"layout": 2})).unwrap();
    assert_eq!(said(&mut s, "design.background", json!({"style": 4})), "changed the background, colour map of layout 2 of slide master 0");
    s.execute("view.closeMaster", &json!({})).unwrap();
    s.execute("slide.new", &json!({"layout": "sectionHeader"})).unwrap();
    s.execute("slide.new", &json!({"layout": "titleAndContent"})).unwrap();
    let st = s.doc().unwrap();
    let theme = &st.doc.masters[0].theme.colors;
    let text = |i: usize| {
        let ctx = deckcraft_model::resolve::Ctx::for_slide(&st.doc, &st.doc.slides[i]).unwrap();
        (ctx.color(&deckcraft_model::ColorRef::scheme(SchemeSlot::Tx1), None), deckcraft_model::resolve::background(&ctx, Some(&st.doc.slides[i])))
    };
    let (dark_text, (dark_bg, ph)) = text(0);
    assert_eq!(dark_text, theme.get(SchemeSlot::Lt1));
    assert_eq!(ph, Some(theme.get(SchemeSlot::Dk1)), "{dark_bg:?}");
    assert_eq!(text(1).0, theme.get(SchemeSlot::Dk1));
}

#[test]
fn a_light_style_on_a_layout_stays_light_under_a_dark_master() {
    use deckcraft_color::SchemeSlot;
    let mut s = template();
    s.execute("view.slideMaster", &json!({"master": 0})).unwrap();
    s.execute("design.background", &json!({"style": 4})).unwrap();
    s.execute("design.background", &json!({"reset": true})).unwrap();
    assert!(!s.doc().unwrap().doc.masters[0].color_map.is_empty());
    s.execute("view.slideMaster", &json!({"layout": 1})).unwrap();
    s.execute("design.background", &json!({"style": 1})).unwrap();
    s.execute("view.closeMaster", &json!({})).unwrap();
    s.execute("slide.new", &json!({"layout": "titleAndContent"})).unwrap();
    let st = s.doc().unwrap();
    let ctx = deckcraft_model::resolve::Ctx::for_slide(&st.doc, &st.doc.slides[0]).unwrap();
    let theme = &st.doc.masters[0].theme.colors;
    assert_eq!(ctx.color(&deckcraft_model::ColorRef::scheme(SchemeSlot::Tx1), None), theme.get(SchemeSlot::Dk1));
}

#[test]
fn clearing_formatting_in_master_view_leaves_the_text_styles() {
    let mut s = template();
    s.execute("view.slideMaster", &json!({"master": 0})).unwrap();
    let title = placeholder(&s, deckcraft_model::PhType::Title);
    s.execute("edit.select", &json!({"ids": [title]})).unwrap();
    let before = s.doc().unwrap().doc.masters[0].title_style.clone();
    s.execute("format.clear", &json!({})).unwrap();
    assert_eq!(s.doc().unwrap().doc.masters[0].title_style, before);
}

#[test]
fn a_slide_placeholder_set_to_a_rectangle_stays_one() {
    let mut s = template();
    s.execute("view.slideMaster", &json!({"layout": 1})).unwrap();
    let content = placeholder(&s, deckcraft_model::PhType::Obj);
    s.execute("shape.change", &json!({"ids": [content], "preset": "roundRect"})).unwrap();
    s.execute("view.closeMaster", &json!({})).unwrap();
    s.execute("slide.new", &json!({"layout": "titleAndContent"})).unwrap();
    let on_slide = placeholder(&s, deckcraft_model::PhType::Obj);
    s.execute("shape.change", &json!({"ids": [on_slide], "preset": "rect"})).unwrap();
    let v = s.execute("slide.inspect", &json!({})).unwrap();
    let presets: Vec<&str> = v["shapes"].as_array().unwrap().iter().filter_map(|sh| sh["preset"].as_str()).collect();
    assert_eq!(presets, ["rect", "rect"], "{v}");
}

#[test]
fn a_layout_has_one_footer_placeholder_of_each_kind() {
    let mut s = template();
    s.execute("view.slideMaster", &json!({"layout": 1})).unwrap();
    let footer = placeholder(&s, deckcraft_model::PhType::Footer);
    assert_eq!(
        said(&mut s, "master.insertPlaceholder", json!({"kind": "footer"})),
        format!(
            "invalid parameters for `master.insertPlaceholder`: layout 1 already has its footer placeholder, shape {footer}. \
             `master.insertPlaceholder` takes {}.",
            find_command("master.insertPlaceholder").unwrap().params
        )
    );
}

#[test]
fn slide_inspect_in_master_view_inspects_the_layout_or_master() {
    let mut s = template();
    s.execute("view.slideMaster", &json!({"layout": 1})).unwrap();
    let v = s.execute("slide.inspect", &json!({})).unwrap();
    assert_eq!((v["layout"].as_u64(), v["name"].as_str()), (Some(1), Some("Title and Content")), "{v}");
    let ids: Vec<u64> = v["shapes"].as_array().unwrap().iter().filter_map(|sh| sh["id"].as_u64()).collect();
    assert_eq!(ids.len(), 5, "{v}");
    s.execute("view.slideMaster", &json!({"master": 0})).unwrap();
    let v = s.execute("slide.inspect", &json!({})).unwrap();
    assert_eq!((v["slideMaster"].as_u64(), v["layout"].as_u64()), (Some(0), None), "{v}");
    assert_eq!(
        said(&mut s, "slide.inspect", json!({"index": 0})),
        "invalid parameters for `slide.inspect`: no slide at index 0; there are none. `slide.inspect` takes {index?} → the slide's shapes with \
         ids, kinds, boxes, text, fills, animations, transition, notes (in Slide Master view without `index`: the slide master or layout being \
         edited)."
    );
}

#[test]
fn slides_take_the_shape_of_their_layout_placeholders() {
    let mut s = template();
    s.execute("view.slideMaster", &json!({"layout": 1})).unwrap();
    let content = placeholder(&s, deckcraft_model::PhType::Obj);
    s.execute("shape.change", &json!({"ids": [content], "preset": "roundRect"})).unwrap();
    s.execute("view.closeMaster", &json!({})).unwrap();
    s.execute("slide.new", &json!({"layout": "titleAndContent"})).unwrap();
    let v = s.execute("slide.inspect", &json!({})).unwrap();
    let presets: Vec<&str> = v["shapes"].as_array().unwrap().iter().filter_map(|sh| sh["preset"].as_str()).collect();
    assert_eq!(presets, ["rect", "roundRect"], "{v}");
}

#[test]
fn a_layout_set_back_to_a_rectangle_stays_one_under_a_rounded_master() {
    use deckcraft_model::PhType;
    let mut s = template();
    s.execute("view.slideMaster", &json!({"master": 0})).unwrap();
    let body = placeholder(&s, PhType::Body);
    s.execute("shape.change", &json!({"ids": [body], "preset": "roundRect"})).unwrap();
    s.execute("view.slideMaster", &json!({"layout": 1})).unwrap();
    let content = placeholder(&s, PhType::Obj);
    s.execute("shape.change", &json!({"ids": [content], "preset": "rect"})).unwrap();
    s.execute("view.closeMaster", &json!({})).unwrap();
    s.execute("slide.new", &json!({"layout": "titleAndContent"})).unwrap();
    let v = s.execute("slide.inspect", &json!({})).unwrap();
    let presets: Vec<&str> = v["shapes"].as_array().unwrap().iter().filter_map(|sh| sh["preset"].as_str()).collect();
    assert_eq!(presets, ["rect", "rect"], "{v}");
}

#[test]
fn growing_a_layout_placeholder_steps_each_level_from_its_own_size() {
    let mut s = template();
    s.execute("view.slideMaster", &json!({"layout": 1})).unwrap();
    let content = placeholder(&s, deckcraft_model::PhType::Obj);
    s.execute("edit.select", &json!({"ids": [content]})).unwrap();
    let st = s.doc().unwrap();
    let before: Vec<f64> = (0..3).map(|l| st.doc.masters[0].body_style.level(l).unwrap().run.size.unwrap()).collect();
    s.execute("format.grow", &json!({})).unwrap();
    let st = s.doc().unwrap();
    let style = &st.shape(deckcraft_model::ShapeId(content as u32)).unwrap().text.as_ref().unwrap().list_style;
    let after: Vec<f64> = (0..3).map(|l| style.level(l).unwrap().run.size.unwrap()).collect();
    assert!(after.iter().zip(&before).all(|(a, b)| a > b), "{before:?} → {after:?}");
    assert!(after[0] > after[1] && after[1] > after[2], "{after:?}");
}

#[test]
fn growing_several_placeholders_steps_each_from_its_own_size() {
    use deckcraft_model::PhType;
    let mut s = template();
    s.execute("view.slideMaster", &json!({"master": 0})).unwrap();
    let (title, body) = (placeholder(&s, PhType::Title), placeholder(&s, PhType::Body));
    s.execute("edit.select", &json!({"ids": [title, body]})).unwrap();
    let size = |s: &Session| {
        let m = &s.doc().unwrap().doc.masters[0];
        (m.title_style.level(0).unwrap().run.size.unwrap(), m.body_style.level(0).unwrap().run.size.unwrap())
    };
    let (title_before, body_before) = size(&s);
    s.execute("format.grow", &json!({})).unwrap();
    let (title_after, body_after) = size(&s);
    assert!(title_after > title_before && body_after > body_before && body_after < title_before, "{body_before} → {body_after}");
}

#[test]
fn save_template_writes_a_potx_only() {
    let mut s = template();
    assert_eq!(
        said(&mut s, "file.saveTemplate", json!({"path": "/tmp/brand.pptx"})),
        "invalid parameters for `file.saveTemplate`: a template is a .potx file, and `/tmp/brand.pptx` isn't one. `file.saveTemplate` takes \
         {path: .potx}."
    );
    assert_eq!(cmd::file::format_for_path("Brand.POTX"), "potx");
    s.ui_requests.clear();
    s.execute("file.saveTemplate", &json!({})).unwrap();
    assert!(matches!(s.ui_requests.as_slice(), [crate::UiRequest::PickFile { purpose }] if purpose == "saveTemplate"), "{:?}", s.ui_requests);
}

#[test]
fn deleting_slide_ids_that_dont_exist_deletes_nothing() {
    let mut s = session();
    s.call("slide.new", &json!({})).unwrap();
    s.call("slide.new", &json!({})).unwrap();
    let first = s.doc().unwrap().doc.slides[0].id.0;
    let takes = "`slide.delete` takes {index?, ids?: [slide id]}.";
    assert_eq!(
        said(&mut s, "slide.delete", json!({"ids": [first, 9999]})),
        format!("invalid parameters for `slide.delete`: no slide has id 9999; document.inspect lists the slide ids. {takes}")
    );
    assert_eq!(
        said(&mut s, "slide.delete", json!({"ids": [9999, 8888]})),
        format!("invalid parameters for `slide.delete`: no slides have ids 9999, 8888; document.inspect lists the slide ids. {takes}")
    );
    assert_eq!(
        said(&mut s, "slide.delete", json!({"ids": []})),
        format!("invalid parameters for `slide.delete`: `ids` is empty; document.inspect lists the slide ids. {takes}")
    );
    assert_eq!(s.doc().unwrap().doc.slides.len(), 3, "a refused delete deletes nothing");
    s.call("slide.delete", &json!({"ids": [first]})).unwrap();
    assert_eq!(s.doc().unwrap().doc.slides.len(), 2);
}

#[test]
fn an_index_out_of_range_names_the_range() {
    let mut s = session();
    assert_eq!(
        said(&mut s, "slide.go", json!({"index": 7})),
        "invalid parameters for `slide.go`: no slide at index 7; indices run 0–0. `slide.go` takes {index} | {id}."
    );
    assert_eq!(
        said(&mut s, "slide.delete", json!({"index": 7})),
        "invalid parameters for `slide.delete`: no slide at index 7; indices run 0–0. `slide.delete` takes {index?, ids?: [slide id]}."
    );
    assert_eq!(
        said(&mut s, "slide.layout", json!({"layout": "blank", "index": 7})),
        "invalid parameters for `slide.layout`: no slide at index 7; indices run 0–0. `slide.layout` takes {layout: name or kind, index?}."
    );
    assert_eq!(
        said(&mut s, "section.rename", json!({"index": 2, "name": "A"})),
        "invalid parameters for `section.rename`: no section at index 2; there are none. `section.rename` takes {index, name}."
    );
    assert_eq!(
        said(&mut s, "comment.reply", json!({"index": 0, "text": "ok"})),
        "invalid parameters for `comment.reply`: no comment on slide index 0 at index 0; there are none. `comment.reply` takes {index, text}."
    );
    assert_eq!(
        said(&mut s, "text.insert", json!({"text": "x"})),
        "command `text.insert` is not available right now: no text insertion point. text.edit {\"id\": …} starts editing a shape's text."
    );
    assert!(
        said(&mut s, "slide.new", json!({"layout": "nope"}))
            .starts_with("invalid parameters for `slide.new`: no layout \"nope\" in slide master 0; its layouts are Title Slide, ")
    );
}
