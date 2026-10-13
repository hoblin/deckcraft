//! A contact sheet: every slide of a deck small on one image, each under its index.

use deckcraft_color::Rgba;
use deckcraft_model::Presentation;
use deckcraft_model::resolve::Ctx;
use deckcraft_model::text::{Align, Anchor};
use kurbo::{Affine, Rect};
use vello_cpu::RenderContext;

use crate::{Image, MAX_SIDE, RENDERER, RenderOpts, Renderer, color, render_slide};

const COLUMNS: usize = 4;
const GAP: f64 = 16.0;
const LABEL: f64 = 24.0;

/// Every slide of `pres` `thumb` pixels wide, four to a row, each under its index from 0 ("hidden"
/// after the index of a hidden slide). A long deck gets narrower slides so the image fits
/// [`MAX_SIDE`]. A deck without slides is an empty image; a panic is a blank one (logged).
pub fn render_sheet(pres: &Presentation, thumb: u32) -> Image {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| sheet(pres, thumb))) {
        Ok(img) => img,
        Err(_) => {
            log::error!("rendering a contact sheet panicked; showing a blank image");
            RENDERER.with(|r| *r.borrow_mut() = Renderer::default());
            Image::default()
        }
    }
}

fn sheet(pres: &Presentation, thumb: u32) -> Image {
    let n = pres.slides.len();
    let Some(rctx) = pres.slides.first().and_then(|s| Ctx::for_slide(pres, s)) else { return Image::default() };
    let (cols, rows) = (n.min(COLUMNS), n.div_ceil(COLUMNS));
    let aspect = (pres.slide_size.height / pres.slide_size.width).clamp(0.05, 20.0);
    let fits = (MAX_SIDE as f64 - GAP - rows as f64 * (LABEL + GAP)) / (rows as f64 * aspect);
    let tw = (thumb as f64).min(fits).min((MAX_SIDE as f64 - GAP) / cols as f64 - GAP).floor().max(8.0);
    let th = (tw * aspect).round().max(1.0);
    let w = ((cols as f64 + 1.0) * GAP + cols as f64 * tw).clamp(1.0, MAX_SIDE as f64);
    let h = (GAP + rows as f64 * (LABEL + th + GAP)).clamp(1.0, MAX_SIDE as f64);
    let cell = |i: usize| (GAP + (i % COLUMNS) as f64 * (tw + GAP), GAP + (i / COLUMNS) as f64 * (LABEL + th + GAP) + LABEL);

    let mut ctx = RenderContext::new_with(w as u16, h as u16, vello_cpu::RenderSettings { num_threads: 0, ..Default::default() });
    ctx.set_paint(color(Rgba { r: 0xEE, g: 0xEE, b: 0xEE, a: 255 }, 1.0));
    ctx.fill_rect(&Rect::new(0.0, 0.0, w, h));
    for (i, slide) in pres.slides.iter().enumerate() {
        let (x, y) = cell(i);
        ctx.set_paint(color(Rgba { r: 0xB0, g: 0xB0, b: 0xB0, a: 255 }, 1.0));
        ctx.fill_rect(&Rect::new(x - 1.0, y - 1.0, x + tw + 1.0, y + th + 1.0));
        let text = if slide.hidden { format!("{i} hidden") } else { i.to_string() };
        let ink = Rgba { r: 0x33, g: 0x33, b: 0x33, a: 255 };
        crate::chart::label(
            &mut ctx,
            &rctx,
            Affine::IDENTITY,
            &text,
            Rect::new(x, y - LABEL, x + tw, y),
            13.0,
            Align::Left,
            Anchor::Middle,
            ink,
            true,
        );
    }
    let mut out = RENDERER.with(|r| r.borrow_mut().finish(ctx, w as u16, h as u16));

    let (tw, th) = (tw as usize, th as usize);
    let opts = RenderOpts { scale: tw as f64 / pres.slide_size.width.max(1.0), size: Some((tw as u32, th as u32)), ..Default::default() };
    for i in 0..n {
        let img = render_slide(pres, i, &opts);
        let (x, y) = cell(i);
        let (x, y) = (x as usize, y as usize);
        for row in 0..th {
            let src = img.pixels.get(row * tw * 4..(row + 1) * tw * 4);
            let at = ((y + row) * out.width as usize + x) * 4;
            if let (Some(src), Some(dst)) = (src, out.pixels.get_mut(at..at + tw * 4)) {
                dst.copy_from_slice(src);
            }
        }
    }
    out
}
