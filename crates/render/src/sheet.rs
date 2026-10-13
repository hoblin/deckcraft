//! A contact sheet: every slide of a deck small on one image, each under its index.

use deckcraft_color::Rgba;
use deckcraft_model::Presentation;
use deckcraft_model::resolve::Ctx;
use deckcraft_model::text::{Align, Anchor};
use kurbo::{Affine, Rect};
use vello_cpu::RenderContext;

use crate::{Image, RENDERER, RenderOpts, Renderer, color, render_slide};

/// The longest side of a contact sheet: what an agent can still read without it being scaled down.
pub const CONTACT_SHEET_SIDE: u32 = 2048;
const THUMB: f64 = 320.0;
const MIN_THUMB: f64 = 96.0;
const COLUMNS: usize = 4;
const GAP: f64 = 16.0;
const LABEL: f64 = 24.0;
const LABEL_SIZE: f64 = 13.0;
const BACKDROP: Rgba = Rgba { r: 0xEE, g: 0xEE, b: 0xEE, a: 255 };
const FRAME: Rgba = Rgba { r: 0xB0, g: 0xB0, b: 0xB0, a: 255 };
const INK: Rgba = Rgba { r: 0x33, g: 0x33, b: 0x33, a: 255 };

/// Why a contact sheet wasn't drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ContactSheetError {
    NoSlides,
    /// The slides would be narrower than [`MIN_THUMB`] to fit [`CONTACT_SHEET_SIDE`].
    TooMany(usize),
    /// The renderer panicked (logged).
    Failed,
}

impl std::fmt::Display for ContactSheetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoSlides => write!(f, "the presentation has no slides"),
            Self::TooMany(n) => write!(f, "{n} slides are too many for one readable contact sheet"),
            Self::Failed => write!(f, "the contact sheet could not be drawn"),
        }
    }
}

/// Every slide of `pres` on one image no larger than [`CONTACT_SHEET_SIDE`] on either side, each
/// under its index from 0 ("hidden" after the index of a hidden slide): four to a row at 320 px,
/// more and narrower ones as the deck grows.
pub fn render_contact_sheet(pres: &Presentation) -> Result<Image, ContactSheetError> {
    let n = pres.slides.len();
    if n == 0 {
        return Err(ContactSheetError::NoSlides);
    }
    let aspect = (pres.slide_size.height / pres.slide_size.width).clamp(0.05, 20.0);
    let grid = grid(n, aspect).ok_or(ContactSheetError::TooMany(n))?;
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| draw(pres, grid))) {
        Ok(img) => Ok(img),
        Err(_) => {
            log::error!("rendering a contact sheet panicked");
            RENDERER.with(|r| *r.borrow_mut() = Renderer::default());
            Err(ContactSheetError::Failed)
        }
    }
}

#[derive(Clone, Copy)]
struct Grid {
    cols: usize,
    tw: f64,
    th: f64,
    w: f64,
    h: f64,
}

fn grid(n: usize, aspect: f64) -> Option<Grid> {
    let side = CONTACT_SHEET_SIDE as f64;
    (COLUMNS..=COLUMNS.max(n)).find_map(|cols| {
        let cols = cols.min(n);
        let tw = THUMB.min(((side - GAP) / cols as f64 - GAP).floor());
        let th = (tw * aspect).floor().max(1.0);
        let w = (cols as f64 + 1.0) * GAP + cols as f64 * tw;
        let h = GAP + n.div_ceil(cols) as f64 * (LABEL + th + GAP);
        (tw >= MIN_THUMB && h <= side).then_some(Grid { cols, tw, th, w, h })
    })
}

fn draw(pres: &Presentation, g: Grid) -> Image {
    let Some(rctx) = pres.slides.first().and_then(|s| Ctx::for_slide(pres, s)) else { return Image::default() };
    let cell = |i: usize| (GAP + (i % g.cols) as f64 * (g.tw + GAP), GAP + (i / g.cols) as f64 * (LABEL + g.th + GAP) + LABEL);
    let mut ctx = RenderContext::new_with(g.w as u16, g.h as u16, vello_cpu::RenderSettings { num_threads: 0, ..Default::default() });
    ctx.set_paint(color(BACKDROP, 1.0));
    ctx.fill_rect(&Rect::new(0.0, 0.0, g.w, g.h));
    for (i, slide) in pres.slides.iter().enumerate() {
        let (x, y) = cell(i);
        ctx.set_paint(color(FRAME, 1.0));
        ctx.fill_rect(&Rect::new(x - 1.0, y - 1.0, x + g.tw + 1.0, y + g.th + 1.0));
        let text = if slide.hidden { format!("{i} hidden") } else { i.to_string() };
        crate::chart::label(
            &mut ctx,
            &rctx,
            Affine::IDENTITY,
            &text,
            Rect::new(x, y - LABEL, x + g.tw, y),
            LABEL_SIZE,
            Align::Left,
            Anchor::Middle,
            INK,
            true,
        );
    }
    let mut out = RENDERER.with(|r| r.borrow_mut().finish(ctx, g.w as u16, g.h as u16));

    let (tw, th) = (g.tw as usize, g.th as usize);
    let opts = RenderOpts { scale: g.tw / pres.slide_size.width.max(1.0), size: Some((tw as u32, th as u32)), ..Default::default() };
    for i in 0..pres.slides.len() {
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
