use std::io::{Cursor, Read};

use deckcraft_engine::Session;
use deckcraft_model::resolve::{self, Ctx};
use deckcraft_model::{Geom, PhType, Presentation};
use serde_json::json;

fn run(s: &mut Session, id: &str, p: serde_json::Value) -> serde_json::Value {
    s.execute(id, &p).unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn placeholder(s: &Session, kind: PhType) -> u32 {
    s.doc().expect("doc").shapes().iter().find(|sh| sh.ph_type() == Some(kind)).expect("placeholder").id.0
}

fn brand_template() -> Presentation {
    let mut s = Session::new();
    run(&mut s, "file.new", json!({"blank": true}));
    run(&mut s, "view.slideMaster", json!({"master": 0}));
    let title = placeholder(&s, PhType::Title);
    run(&mut s, "edit.select", json!({"ids": [title]}));
    run(&mut s, "format.font", json!({"family": "Supera Gothic Bold"}));
    run(&mut s, "view.slideMaster", json!({"layout": 2}));
    run(&mut s, "design.hideBackgroundGraphics", json!({"hide": true}));
    run(&mut s, "design.background", json!({"style": 4}));
    run(&mut s, "view.slideMaster", json!({"layout": 1}));
    let content = placeholder(&s, PhType::Obj);
    run(&mut s, "shape.change", json!({"ids": [content], "preset": "roundRect"}));
    run(&mut s, "text.set", json!({"id": content, "text": "Three points, no more"}));
    run(&mut s, "view.slideMaster", json!({"layout": 6}));
    run(&mut s, "master.insertPlaceholder", json!({"kind": "subtitle", "rect": [80, 300, 800, 60], "prompt": "Add the quarter"}));
    run(&mut s, "view.closeMaster", json!({}));
    run(&mut s, "design.headerFooter", json!({"slideNumber": true, "footerText": "Bondata", "hideOnTitle": true}));
    s.doc().expect("doc").doc.as_ref().clone()
}

fn main_content_type(bytes: &[u8]) -> String {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).expect("zip");
    let mut xml = String::new();
    zip.by_name("[Content_Types].xml").expect("content types").read_to_string(&mut xml).expect("read");
    let at = xml.find("PartName=\"/ppt/presentation.xml\"").expect("presentation override");
    let tail = &xml[at..];
    let ct = &tail[tail.find("ContentType=\"").expect("content type") + 13..];
    ct[..ct.find('"').expect("quote")].to_string()
}

fn edited(bytes: &[u8], edits: &[(&str, &str, &str)]) -> Vec<u8> {
    use std::io::Write;
    let mut src = zip::ZipArchive::new(Cursor::new(bytes)).expect("zip");
    let mut out = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for i in 0..src.len() {
        let mut entry = src.by_index(i).expect("entry");
        let name = entry.name().to_string();
        let mut data = String::new();
        if entry.read_to_string(&mut data).is_err() {
            continue;
        }
        for (part, from, to) in edits.iter().filter(|(part, _, _)| *part == name) {
            assert!(data.contains(from), "{part} has no {from}");
            data = data.replacen(from, to, 1);
        }
        out.start_file(name, zip::write::SimpleFileOptions::default()).expect("start");
        out.write_all(data.as_bytes()).expect("write");
    }
    out.finish().expect("finish").into_inner()
}

#[test]
fn footer_settings_and_layout_colour_maps_written_by_another_app_are_read() {
    let bytes = deckcraft_pptx::export(&Presentation::default()).expect("export");
    let bytes = edited(
        &bytes,
        &[
            ("ppt/slideMasters/slideMaster1.xml", "<p:txStyles>", "<p:hf sldNum=\"0\" hdr=\"0\" dt=\"0\"/><p:txStyles>"),
            (
                "ppt/slideLayouts/slideLayout3.xml",
                "<a:masterClrMapping/>",
                "<a:overrideClrMapping bg1=\"dk1\" tx1=\"lt1\" bg2=\"dk2\" tx2=\"lt2\" accent1=\"accent1\" accent2=\"accent2\" \
                 accent3=\"accent3\" accent4=\"accent4\" accent5=\"accent5\" accent6=\"accent6\" hlink=\"hlink\" folHlink=\"folHlink\"/>",
            ),
        ],
    );
    let p = deckcraft_pptx::import(&bytes).expect("import");
    assert_eq!((p.header_footer.footer, p.header_footer.slide_number, p.header_footer.date), (true, false, false));
    let map = p.masters[0].layouts[2].color_map.as_ref().expect("override");
    assert!(map.contains(&("tx1".into(), "lt1".into())), "{map:?}");
    assert_eq!(p.masters[0].layouts[1].color_map, None);
}

#[test]
fn a_template_saves_with_the_template_content_type() {
    let p = brand_template();
    let potx = deckcraft_pptx::export_as(&p, deckcraft_pptx::Package::Template).expect("export");
    assert_eq!(main_content_type(&potx), "application/vnd.openxmlformats-officedocument.presentationml.template.main+xml");
    let ppsx = deckcraft_pptx::export_as(&p, deckcraft_pptx::Package::Show).expect("export");
    assert_eq!(main_content_type(&ppsx), "application/vnd.openxmlformats-officedocument.presentationml.slideshow.main+xml");
    let pptx = deckcraft_pptx::export(&p).expect("export");
    assert_eq!(main_content_type(&pptx), "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml");
    if let Ok(dir) = std::env::var("DECKCRAFT_PPTX_OUT") {
        std::fs::write(format!("{dir}/brand.potx"), &potx).expect("write");
    }
}

#[test]
fn a_template_keeps_what_its_author_set() {
    let p = brand_template();
    let bytes = deckcraft_pptx::export_as(&p, deckcraft_pptx::Package::Template).expect("export");
    let q = deckcraft_pptx::import(&bytes).expect("import");
    let (pm, qm) = (&p.masters[0], &q.masters[0]);
    assert_eq!(qm.title_style.level(0).and_then(|l| l.run.font.as_deref()), Some("Supera Gothic Bold"));
    assert!(!qm.layouts[2].show_master_shapes);
    assert_eq!(qm.layouts[2].color_map.as_ref().map(|m| m.iter().find(|(k, _)| k == "tx1").map(|(_, v)| v.as_str())), Some(Some("lt1")));
    assert_eq!(qm.layouts[1].color_map, None);
    let content = qm.layouts[1].shapes.iter().find(|sh| sh.ph_type() == Some(PhType::Obj)).expect("content");
    assert_eq!(content.geom, Geom::preset("roundRect"));
    assert!(content.ph.as_ref().is_some_and(|ph| ph.has_custom_prompt));
    let subtitle = qm.layouts[6].shapes.iter().find(|sh| sh.ph_type() == Some(PhType::SubTitle)).expect("subtitle");
    assert_eq!(subtitle.text.as_ref().map(|t| t.text()).as_deref(), Some("Add the quarter"));
    assert_eq!(
        (q.header_footer.slide_number, q.header_footer.footer, q.header_footer.date, q.header_footer.hide_on_title),
        (true, true, false, true)
    );
    assert_eq!(q.header_footer.footer_text, "Bondata");
    assert_eq!(pm.layouts.len(), qm.layouts.len());
}

#[test]
fn a_deck_started_from_the_template_follows_it() {
    let bytes = deckcraft_pptx::export_as(&brand_template(), deckcraft_pptx::Package::Template).expect("export");
    let mut s = Session::new();
    run(&mut s, "file.openBytes", json!({"name": "brand.potx", "data": deckcraft_engine::cmd::base64_encode(&bytes)}));
    run(&mut s, "slide.new", json!({"layout": "titleAndContent"}));
    let st = s.doc().expect("doc");
    let slide = &st.doc.slides[0];
    let footers: Vec<PhType> = slide.shapes.iter().filter_map(|sh| sh.ph_type()).filter(|k| k.is_footer_kind()).collect();
    assert_eq!(footers, [PhType::Footer, PhType::SlideNum]);
    let ctx = Ctx::for_slide(&st.doc, slide).expect("ctx");
    let content = slide.shapes.iter().find(|sh| sh.ph_type() == Some(PhType::Obj)).expect("content");
    assert_eq!(resolve::inherited_geom(&ctx, content), Some(&Geom::preset("roundRect")));
}
