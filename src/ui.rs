use crate::controls::{Action, Device};
use crate::game::{
    Card, DAMAGE_MARK_LIFE, Ending, Game, HIT_MARKER_LIFE, HitKind, JournalPage, Mode, PastRun,
    Preferences,
};
use crate::gamepad::{Button, PadAction, Target};
use crate::weapons::Effect;
use egui::{Align2, Color32 as C, FontFamily, FontId, Id, Pos2, Rect, Sense, Shape, Stroke, Vec2};
use glam::Mat4;
const GOLD: C = C::from_rgb(226, 188, 119);
const IVORY: C = C::from_rgb(250, 242, 220);
const MUTED: C = C::from_rgb(207, 199, 180);
const INK: C = C::from_rgb(35, 24, 17);
const PAPER: C = C::from_rgb(231, 211, 173);
const HUD_SURFACE: C = C::from_rgb(19, 22, 21);
const RED: C = C::from_rgb(110, 30, 32);
/// Legible red for text on dark surfaces.
const BLOOD: C = C::from_rgb(226, 108, 94);
fn hash(i: u32) -> f32 {
    let n = i.wrapping_mul(747796405).wrapping_add(2891336453);
    let n = ((n >> ((n >> 28) + 4)) ^ n).wrapping_mul(277803737);
    ((n >> 22) ^ n) as f32 / u32::MAX as f32
}
pub fn configure(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    // One embedded house face, derived from the title's exact letterforms.
    // Fallbacks only cover characters outside the shipped Latin/symbol set.
    let fallback = fonts.families[&FontFamily::Proportional].clone();
    fonts.font_data.insert(
        "gravewake".into(),
        egui::FontData::from_static(include_bytes!(
            "../assets/fonts/GravewakeGothic-Regular.ttf"
        ))
        .into(),
    );
    let mut family = vec!["gravewake".into()];
    family.extend(fallback);
    fonts
        .families
        .insert(FontFamily::Name("gravewake".into()), family);
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        fonts
            .families
            .get_mut(&family)
            .unwrap()
            .insert(0, "gravewake".into());
    }
    for (name, bytes) in [
        (
            "button_plate",
            include_bytes!("../assets/button-plaque.png").as_slice(),
        ),
        (
            "pack_art",
            include_bytes!("../assets/armory-pack.png").as_slice(),
        ),
        (
            "house_art",
            include_bytes!("../assets/house-backdrop.png").as_slice(),
        ),
        (
            "card_front",
            include_bytes!("../assets/card-front.png").as_slice(),
        ),
        (
            "card_back",
            include_bytes!("../assets/card-back.png").as_slice(),
        ),
        (
            "brand_emblem",
            include_bytes!("../assets/gravewake-emblem.png").as_slice(),
        ),
    ] {
        let image = image::load_from_memory(bytes)
            .expect("embedded art")
            .thumbnail(2048, 2048)
            .to_rgba8();
        let size = [image.width() as usize, image.height() as usize];
        let texture = ctx.load_texture(
            name,
            egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw()),
            egui::TextureOptions::LINEAR,
        );
        ctx.data_mut(|d| d.insert_temp(Id::new(name), texture));
    }
    ctx.set_fonts(fonts);
    let mut style = (*ctx.style()).clone();
    style.animation_time = 0.2;
    style.visuals = egui::Visuals::dark();
    style.visuals.override_text_color = Some(IVORY);
    style.visuals.selection.bg_fill = RED;
    style.visuals.widgets.active.bg_fill = RED;
    style.spacing.slider_width = 250.;
    style
        .text_styles
        .insert(egui::TextStyle::Body, TypeRole::Reading.font(18.));
    style
        .text_styles
        .insert(egui::TextStyle::Small, TypeRole::Reading.font(16.));
    style
        .text_styles
        .insert(egui::TextStyle::Button, TypeRole::Strong.font(22.));
    ctx.set_style(style);
    for name in ["paper", "felt"] {
        let mut pixels = Vec::new();
        for y in 0..512 {
            for x in 0..512 {
                let n = hash(x + y * 512);
                let clouds = ((x as f32 * 0.055).sin() * (y as f32 * 0.039).sin() + 1.) * 0.5;
                let v = if name == "paper" {
                    (205. + n * 35. + clouds * 15.) as u8
                } else {
                    (80. + n * 85. + clouds * 55.) as u8
                };
                pixels.push(C::from_gray(v));
            }
        }
        let texture = ctx.load_texture(
            name,
            egui::ColorImage {
                size: [512, 512],
                pixels,
                source_size: Vec2::splat(512.),
            },
            egui::TextureOptions::LINEAR,
        );
        ctx.data_mut(|d| d.insert_temp(Id::new(name), texture));
    }
}
#[derive(Clone, Copy)]
enum TypeRole {
    Display,
    Engraved,
    Reading,
    Strong,
}
impl TypeRole {
    fn font(self, size: f32) -> FontId {
        FontId::new(size, FontFamily::Name("gravewake".into()))
    }
}
/// HUD panel rectangles drawn this frame, recorded in tests only.
const HUD_LAYOUT: &str = "hud_layout";
/// Controls drawn this frame that the controller's D-pad can reach. Each
/// frame starts empty, and a dialog or the journal empties it again, so
/// controls it hides can't be reached.
const PAD_TARGETS: &str = "pad_targets";
/// Sounds the interface asks for, played by `main.rs` after each frame.
const CUES: &str = "interface_cues";
fn cue(ctx: &egui::Context, name: &'static str) {
    ctx.data_mut(|d| {
        d.get_temp_mut_or_default::<Vec<&'static str>>(Id::new(CUES))
            .push(name)
    });
}
/// The sounds the interface asked for since the last call.
pub fn take_cues(ctx: &egui::Context) -> Vec<&'static str> {
    ctx.data_mut(|d| std::mem::take(d.get_temp_mut_or_default::<Vec<&'static str>>(Id::new(CUES))))
}
fn clear_pad_targets(ctx: &egui::Context) {
    ctx.data_mut(|d| d.insert_temp(Id::new(PAD_TARGETS), Vec::<Target>::new()));
}
/// The controls the last frame drew, for the controller cursor.
pub fn pad_targets(ctx: &egui::Context) -> Vec<Target> {
    ctx.data(|d| d.get_temp(Id::new(PAD_TARGETS)))
        .unwrap_or_default()
}
struct Canvas<'a> {
    ui: &'a egui::Ui,
    p: egui::Painter,
    s: f32,
    offset: Vec2,
    h: f32,
    time: f32,
    /// Smallest text size in logical points.
    floor: f32,
}
impl<'a> Canvas<'a> {
    fn new(ui: &'a egui::Ui, time: f32) -> Self {
        let r = ui.max_rect();
        let s = (r.width() / 1440.).min(r.height() / 900.);
        let w = r.width() / s;
        let h = r.height() / s;
        Self {
            ui,
            p: ui.painter().clone(),
            s,
            offset: r.min.to_vec2() + Vec2::new((w - 1440.) * s * 0.5, 0.),
            h,
            time,
            floor: 13.5,
        }
    }
    /// The same canvas scaled by `k` around the design point (`ax`, `ay`),
    /// so a HUD group grows or shrinks from its own corner. Below full size
    /// the text floor shrinks with it, so labels stay inside their panels.
    fn anchored(&self, k: f32, ax: f32, ay: f32) -> Canvas<'a> {
        Canvas {
            ui: self.ui,
            p: self.p.clone(),
            s: self.s * k,
            offset: self.offset + Vec2::new(ax, ay) * self.s * (1. - k),
            h: self.h,
            time: self.time,
            floor: self.floor * k.min(1.),
        }
    }
    /// Note a HUD panel's rectangle so tests can check that none overlap.
    fn layout(&self, name: &'static str, x: f32, y: f32, w: f32, h: f32) {
        if cfg!(test) {
            let rect = self.rect(x, y, w, h);
            self.ui.ctx().data_mut(|d| {
                d.get_temp_mut_or_default::<Vec<(&'static str, Rect)>>(Id::new(HUD_LAYOUT))
                    .push((name, rect))
            });
        }
    }
    /// Let the D-pad reach a control drawn this frame.
    fn target(&self, target: Target) {
        if self.ui.ctx().screen_rect().intersects(target.rect) {
            self.ui.ctx().data_mut(|d| {
                d.get_temp_mut_or_default::<Vec<Target>>(Id::new(PAD_TARGETS))
                    .push(target)
            });
        }
    }
    /// The same canvas moved by design units.
    fn shifted(&self, dx: f32, dy: f32) -> Canvas<'a> {
        Canvas {
            ui: self.ui,
            p: self.p.clone(),
            offset: self.offset + Vec2::new(dx, dy) * self.s,
            ..*self
        }
    }
    fn pt(&self, x: f32, y: f32) -> Pos2 {
        Pos2::new(x * self.s + self.offset.x, y * self.s + self.offset.y)
    }
    fn rect(&self, x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::from_min_size(self.pt(x, y), Vec2::new(w, h) * self.s)
    }
    fn fill(&self, x: f32, y: f32, w: f32, h: f32, c: C) {
        self.p.rect_filled(self.rect(x, y, w, h), 0., c);
    }
    fn line(&self, a: (f32, f32), b: (f32, f32), color: C, width: f32) {
        self.p.line_segment(
            [self.pt(a.0, a.1), self.pt(b.0, b.1)],
            Stroke::new(width * self.s, color),
        );
    }
    fn text(
        &self,
        x: f32,
        y: f32,
        text: impl Into<String>,
        size: f32,
        color: C,
        serif: bool,
        align: Align2,
    ) {
        let text = text.into();
        let role = if serif && size >= 28. {
            TypeRole::Display
        } else if size >= 19.
            && text.chars().any(char::is_alphabetic)
            && !text.chars().any(char::is_lowercase)
        {
            TypeRole::Engraved
        } else {
            TypeRole::Reading
        };
        let size = size.max(18.);
        self.text_role(x, y, text, size, color, role, align);
    }
    fn text_role(
        &self,
        x: f32,
        y: f32,
        text: impl Into<String>,
        size: f32,
        color: C,
        role: TypeRole,
        align: Align2,
    ) {
        let text = text.into();
        let font = role.font((size * self.s).max(self.floor));
        let ppp = self.ui.ctx().pixels_per_point();
        let anchor = self.pt(x, y);
        let anchor = Pos2::new(
            (anchor.x * ppp).round() / ppp,
            (anchor.y * ppp).round() / ppp,
        );
        // One device-pixel shadow separates light glyphs from engraved textures.
        // Weight comes from real static font outlines, never offset faux-bold copies.
        if color.r() > 160 && color.g() > 135 {
            self.p.text(
                anchor + Vec2::new(0., 1. / ppp),
                align,
                &text,
                font.clone(),
                C::from_black_alpha((color.a() as u16 * 3 / 4) as u8),
            );
        }
        self.p.text(anchor, align, &text, font, color);
    }
    /// The width in design units of `text` as `Canvas::text` draws it.
    fn text_width(&self, text: &str, size: f32) -> f32 {
        let galley = self.p.layout_no_wrap(
            text.into(),
            TypeRole::Reading.font((size.max(18.) * self.s).max(self.floor)),
            IVORY,
        );
        galley.size().x / self.s
    }
    fn fit_type(&self, text: &str, role: TypeRole, size: f32, width: f32) -> f32 {
        let galley =
            self.p
                .layout_no_wrap(text.into(), role.font((size * self.s).max(self.floor)), IVORY);
        size * (width * self.s / galley.size().x.max(1.)).min(1.)
    }
    fn paragraph(
        &self,
        x: f32,
        y: f32,
        text: &str,
        width: f32,
        size: f32,
        color: C,
        centered: bool,
    ) -> f32 {
        self.paragraph_role(x, y, text, width, size, color, centered, TypeRole::Reading)
    }
    fn paragraph_role(
        &self,
        x: f32,
        y: f32,
        text: &str,
        width: f32,
        size: f32,
        color: C,
        centered: bool,
        role: TypeRole,
    ) -> f32 {
        let mut job = egui::text::LayoutJob::simple(
            text.into(),
            role.font((size.max(18.) * self.s).max(self.floor)),
            color,
            width * self.s,
        );
        job.halign = if centered {
            egui::Align::Center
        } else {
            egui::Align::LEFT
        };
        let galley = self.p.layout_job(job);
        let height = galley.size().y / self.s;
        self.p.galley(self.pt(x, y), galley, color);
        height
    }
    fn inset(&self, x: f32, y: f32, w: f32, h: f32, color: C) {
        self.p
            .rect_filled(self.rect(x, y, w, h), 4. * self.s, color);
    }
    fn hud_panel(&self, x: f32, y: f32, w: f32, h: f32) {
        self.layout("panel", x, y, w, h);
        self.inset(x, y, w, h, HUD_SURFACE);
        self.border(x, y, w, h, C::from_rgb(111, 87, 51));
    }
    fn center(&self, x: f32, y: f32, text: impl Into<String>, size: f32, color: C) {
        self.text(x, y, text, size, color, false, Align2::CENTER_CENTER);
    }
    fn diamond(&self, x: f32, y: f32, r: f32, c: C) {
        self.p.add(Shape::convex_polygon(
            vec![
                self.pt(x, y - r),
                self.pt(x + r * 0.65, y),
                self.pt(x, y + r),
                self.pt(x - r * 0.65, y),
            ],
            c,
            Stroke::NONE,
        ));
    }
    fn border(&self, x: f32, y: f32, w: f32, h: f32, color: C) {
        self.p.rect_stroke(
            self.rect(x, y, w, h),
            0.,
            Stroke::new(self.s, color),
            egui::StrokeKind::Inside,
        );
        for (xx, yy, dx, dy) in [
            (x, y, 1., 1.),
            (x + w, y, -1., 1.),
            (x, y + h, 1., -1.),
            (x + w, y + h, -1., -1.),
        ] {
            self.line(
                (xx + dx * 4., yy + dy * 12.),
                (xx + dx * 12., yy + dy * 4.),
                color,
                0.8,
            );
            self.diamond(xx + dx * 5., yy + dy * 5., 2., color);
        }
    }
    fn button(&self, id: &str, x: f32, y: f32, w: f32, h: f32, label: &str, primary: bool) -> bool {
        let r = self
            .ui
            .interact(self.rect(x, y, w, h), Id::new(id), Sense::click());
        self.target(Target::button(r.rect));
        // A tick when the pointer or the focus frame first lands on the
        // button, and a clack when it's pressed.
        let ctx = self.ui.ctx();
        let hot = r.hovered() || r.has_focus();
        let key = Id::new((id, "hot"));
        if ctx.data(|d| d.get_temp::<bool>(key)).unwrap_or(false) != hot {
            ctx.data_mut(|d| d.insert_temp(key, hot));
            if hot {
                cue(ctx, "ui_hover");
            }
        }
        if r.clicked() {
            cue(ctx, "ui_press");
        }
        let hover = self
            .ui
            .ctx()
            .animate_bool(Id::new((id, "hover")), hot);
        let press = if r.is_pointer_button_down_on() {
            2.0
        } else {
            0.
        };
        let y = y + press - hover * 1.2;
        let tint = if primary {
            C::from_rgb(255, 240, 214)
        } else {
            C::from_rgb(
                (185. + hover * 70.) as u8,
                (180. + hover * 60.) as u8,
                (168. + hover * 46.) as u8,
            )
        };
        if let Some(t) = self
            .ui
            .ctx()
            .data(|d| d.get_temp::<egui::TextureHandle>(Id::new("button_plate")))
        {
            let cap = h * 0.63;
            let yy = y - h * 0.17;
            let hh = h * 1.34;
            for (xx, ww, u0, u1) in [
                (x, cap, 0., 0.18),
                (x + cap, w - cap * 2., 0.18, 0.82),
                (x + w - cap, cap, 0.82, 1.),
            ] {
                self.p.image(
                    t.id(),
                    self.rect(xx, yy, ww, hh),
                    Rect::from_min_max(Pos2::new(u0, 0.), Pos2::new(u1, 1.)),
                    tint,
                );
            }
        }
        // Measure the same house font on every control, preserving a readable
        // minimum rather than squeezing letters into the metal end caps.
        let width = (w - h * 1.15).max(12.);
        let role = TypeRole::Strong;
        let size = self.fit_type(label, role, 22., width);
        self.inset(
            x + h * 0.57,
            y + h * 0.16,
            w - h * 1.14,
            h * 0.69,
            C::from_black_alpha(100),
        );
        self.text_role(
            x + w * 0.5,
            y + h * 0.51,
            label,
            size,
            if hover > 0.1 { C::WHITE } else { IVORY },
            role,
            Align2::CENTER_CENTER,
        );
        if hover > 0.01 {
            self.line(
                (x + h * 0.8, y + h * 0.80),
                (x + w - h * 0.8, y + h * 0.80),
                GOLD.gamma_multiply(hover * 0.7),
                0.7,
            );
            self.ui
                .ctx()
                .set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        r.clicked()
    }
    fn medallion(&self, x: f32, y: f32, r: f32, active: bool) {
        self.p.circle_filled(
            self.pt(x + 2., y + 3.),
            (r + 2.) * self.s,
            C::from_black_alpha(180),
        );
        if let Some(t) = self
            .ui
            .ctx()
            .data(|d| d.get_temp::<egui::TextureHandle>(Id::new("button_plate")))
        {
            let mut mesh = egui::Mesh::with_texture(t.id());
            mesh.vertices.push(egui::epaint::Vertex {
                pos: self.pt(x, y),
                uv: Pos2::new(0.5, 0.5),
                color: if active { C::WHITE } else { C::from_gray(110) },
            });
            for i in 0..=40 {
                let a = i as f32 / 40. * std::f32::consts::TAU;
                mesh.vertices.push(egui::epaint::Vertex {
                    pos: self.pt(x + a.cos() * r, y + a.sin() * r),
                    uv: Pos2::new(0.5 + a.cos() * 0.22, 0.5 + a.sin() * 0.30),
                    color: if active { C::WHITE } else { C::from_gray(110) },
                });
                if i < 40 {
                    mesh.add_triangle(0, i + 1, i + 2);
                }
            }
            self.p.add(Shape::mesh(mesh));
        }
        self.p
            .circle_stroke(self.pt(x, y), r * self.s, Stroke::new(2. * self.s, GOLD));
        self.p.circle_stroke(
            self.pt(x, y),
            (r - 3.) * self.s,
            Stroke::new(0.7 * self.s, GOLD.gamma_multiply(0.7)),
        );
        for i in 0..20 {
            let a = i as f32 / 20. * std::f32::consts::TAU;
            self.diamond(x + a.cos() * (r + 1.), y + a.sin() * (r + 1.), 1.6, GOLD);
        }
    }
    fn folio(&self, x: f32, y: f32, w: f32, h: f32) {
        self.p.rect_filled(
            self.rect(x + 10., y + 13., w, h),
            10. * self.s,
            C::from_black_alpha(160),
        );
        self.texture("card_front", x, y, w, h, C::from_rgb(241, 225, 194));
        self.inset(
            x + 28.,
            y + 32.,
            w - 56.,
            h - 54.,
            C::from_rgba_unmultiplied(239, 218, 181, 235),
        );
        self.line(
            (x + w * 0.5, y + 18.),
            (x + w * 0.5, y + h - 18.),
            C::from_rgba_unmultiplied(90, 60, 30, 12),
            1.,
        );
    }
    fn flourish(&self, x: f32, y: f32, w: f32, color: C) {
        self.diamond(x, y, 5., color);
        for side in [-1., 1.] {
            self.line((x + side * 12., y), (x + side * w * 0.5, y), color, 0.7);
            for j in 0..4 {
                let a = x + side * (18. + j as f32 * 8.);
                self.line((a, y), (a + side * 6., y - 4. + j as f32), color, 0.8);
            }
        }
    }
    fn slider(&self, id: &str, x: f32, y: f32, w: f32, value: &mut f32, min: f32, max: f32) {
        self.stepped_slider(id, x, y, w, value, min, max, crate::gamepad::SLIDER_STEP);
    }
    /// A slider whose D-pad presses move `step` of its track.
    #[allow(clippy::too_many_arguments)]
    fn stepped_slider(
        &self,
        id: &str,
        x: f32,
        y: f32,
        w: f32,
        value: &mut f32,
        min: f32,
        max: f32,
        step: f32,
    ) {
        let r = self.ui.interact(
            self.rect(x - 12., y - 15., w + 24., 30.),
            Id::new(id),
            Sense::click_and_drag(),
        );
        if r.dragged() || r.clicked() {
            if let Some(p) = r.interact_pointer_pos() {
                *value =
                    (min + (max - min) * ((p.x - self.pt(x, y).x) / (w * self.s))).clamp(min, max);
            }
        }
        let t = (*value - min) / (max - min);
        // The D-pad rests on the knob, so A there keeps the value.
        self.target(Target {
            rect: r.rect,
            point: self.pt(x + w * t, y),
            track: Some((self.pt(x, y).x, self.pt(x + w, y).x)),
            step,
        });
        self.line((x, y), (x + w, y), C::from_rgb(91, 66, 40), 5.);
        self.line((x, y - 1.), (x + w * t, y - 1.), GOLD, 2.);
        // A stepped slider marks its stops; a smooth one, tenths.
        let ticks = if step == crate::gamepad::SLIDER_STEP {
            10
        } else {
            (1. / step).round() as usize
        };
        for i in 0..=ticks {
            let xx = x + i as f32 * w / ticks as f32;
            self.line((xx, y + 8.), (xx, y + 11.), INK.gamma_multiply(0.45), 0.7);
        }
        self.seal(x + w * t, y, 10., INK);
        self.diamond(x + w * t, y, 5., GOLD);
    }
    fn texture(&self, name: &str, x: f32, y: f32, w: f32, h: f32, tint: C) {
        if let Some(t) = self
            .ui
            .ctx()
            .data(|d| d.get_temp::<egui::TextureHandle>(Id::new(name)))
        {
            self.p.image(
                t.id(),
                self.rect(x, y, w, h),
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1., 1.)),
                tint,
            );
        }
    }
    fn coin(&self, x: f32, y: f32, r: f32) {
        self.p
            .circle_filled(self.pt(x, y), r * self.s, C::from_rgb(115, 76, 28));
        self.p.circle_stroke(
            self.pt(x, y),
            (r - 1.) * self.s,
            Stroke::new(1.7 * self.s, GOLD),
        );
        self.p.circle_stroke(
            self.pt(x, y),
            (r - 4.) * self.s,
            Stroke::new(self.s, GOLD.gamma_multiply(0.6)),
        );
        self.diamond(x, y, r * 0.45, GOLD);
    }
    fn seal(&self, x: f32, y: f32, r: f32, c: C) {
        for radius in [r, r * 0.91, r * 0.68] {
            self.p
                .circle_stroke(self.pt(x, y), radius * self.s, Stroke::new(self.s * 0.7, c));
        }
        for i in 0..48 {
            let a = i as f32 * std::f32::consts::TAU / 48.;
            let a2 = a + 0.01;
            let inner = if i % 4 == 0 { r * 0.74 } else { r * 0.84 };
            self.line(
                (x + a.cos() * inner, y + a.sin() * inner),
                (x + a2.cos() * r, y + a2.sin() * r),
                c,
                0.7,
            );
        }
        self.diamond(x, y, r * 0.35, c);
    }
}
fn backdrop(c: &Canvas, table: bool) {
    c.texture("house_art", 0., 0., 1440., c.h, C::WHITE);
    if !table {
        c.fill(0., 0., 1440., c.h, C::from_black_alpha(211));
    }
}
fn weapon_art(c: &Canvas, x: f32, y: f32, size: f32, card: &Card, angle: f32) {
    let _ = angle;
    let kind = card.kind as usize;
    if let Some(id) =
        c.ui.ctx()
            .data(|d| d.get_temp::<egui::TextureId>(Id::new(("weapon_preview", kind, card.rarity))))
    {
        c.p.image(
            id,
            c.rect(x - size * 0.58, y - size * 0.48, size * 1.28, size * 0.96),
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1., 1.)),
            C::WHITE,
        );
    }
}
fn chalice_art(c: &Canvas, x: f32, y: f32, s: f32) {
    if let Some(id) = c.ui.ctx().data(|d| {
        d.get_temp::<egui::TextureId>(Id::new((
            "weapon_preview",
            crate::weapons::WeaponKind::ALL.len(),
            0usize,
        )))
    }) {
        c.p.image(
            id,
            c.rect(x - 116. * s, y - 96. * s, 232. * s, 174. * s),
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1., 1.)),
            C::WHITE,
        );
    }
}
fn card_base(c: &Canvas, x: f32, y: f32, w: f32, h: f32, rarity: usize, back: bool) {
    c.p.rect_filled(
        c.rect(x + 7., y + 9., w, h),
        8. * c.s,
        C::from_black_alpha(130),
    );
    let tint = if back {
        C::WHITE
    } else {
        [
            C::WHITE,
            C::from_rgb(119, 183, 224),
            C::from_rgb(255, 221, 122),
            C::from_rgb(222, 126, 70),
        ][rarity.min(3)]
    };
    c.texture(
        if back { "card_back" } else { "card_front" },
        x,
        y,
        w,
        h,
        tint,
    );
}
fn weapon_card(
    c: &Canvas,
    id: &str,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    card: &Card,
    action: &str,
    equipped: Option<&Card>,
) -> bool {
    let hover =
        c.ui.interact(c.rect(x, y, w, h), Id::new((id, "card")), Sense::hover())
            .hovered();
    let lift = c.ui.ctx().animate_bool(Id::new((id, "lift")), hover) * 9.;
    let y = y - lift;
    card_base(c, x, y, w, h, card.rarity, false);
    // Rarity remains in the frame and model field. Reading surfaces
    // stay neutral so every affix, stat and price has the same strong contrast.
    let ink = INK;
    c.inset(x + 22., y + 21., w - 44., 28., PAPER);
    c.center(
        x + w * 0.5,
        y + 35.,
        format!(
            "{}  /  {}",
            rarity_name(card),
            crate::weapons::GROUPS[card.kind.spec().group].to_uppercase()
        ),
        10.,
        ink,
    );
    weapon_art(
        c,
        x + w * 0.44,
        y + h * 0.31,
        (w * 0.78).min(h * 0.37),
        card,
        if hover {
            (c.time * 0.7).sin() * 0.04
        } else {
            0.
        },
    );
    c.inset(x + 18., y + h * 0.495, w - 36., h * 0.345, PAPER);
    let name = card.name();
    c.paragraph_role(
        x + w * 0.5,
        y + h * 0.512,
        &name,
        w - 50.,
        21.,
        ink,
        true,
        TypeRole::Engraved,
    );
    let traits = trait_line(card);
    let size = c.fit_type(&traits, TypeRole::Reading, 18., w - 40.);
    c.text_role(
        x + w * 0.5,
        y + h * 0.663,
        traits,
        size,
        ink,
        TypeRole::Reading,
        Align2::CENTER_CENTER,
    );
    let damage = if card.kind.melee() {
        format!(
            "{:.0} DAMAGE  /  {:.1}m REACH",
            card.damage(),
            card.kind.spec().range
        )
    } else if card.pellets() > 1 {
        format!(
            "{:.0} x {}  /  {} ROUNDS",
            card.damage(),
            card.pellets(),
            card.capacity()
        )
    } else {
        format!("{:.0} DAMAGE  /  {} ROUNDS", card.damage(), card.capacity())
    };
    c.center(x + w * 0.5, y + h * 0.72, damage, 16., ink);
    c.center(
        x + w * 0.5,
        y + h * 0.768,
        if card.kind.melee() {
            format!("{:.1} SWINGS / SECOND", 1. / card.interval())
        } else {
            format!(
                "{:.1}/s  /  {:.2}s RELOAD",
                1. / card.interval(),
                card.reload_time()
            )
        },
        10.5,
        ink,
    );
    if let Some(equipped) = equipped {
        let (line, color) = dps_comparison(card, equipped);
        c.center(x + w * 0.5, y + h * 0.808, line, 11.5, color);
    }
    if !action.is_empty() {
        c.button(id, x + 22., y + h - 61., w - 44., 36., action, true)
    } else {
        false
    }
}
fn rarity_name(card: &Card) -> &'static str {
    ["COMMON", "UNCOMMON", "RARE", "LEGENDARY"][card.rarity.min(3)]
}
/// The weapon's trait, what it does beyond a plain hit, and how many
/// creatures one attack reaches: "BURN + 2.4m SPLASH", "PIERCES 4",
/// "139° SWEEP", or "ONE TARGET".
fn trait_line(card: &Card) -> String {
    let kind = card.kind;
    let effect = kind.spec().effect;
    // Piercing, chains and blasts are named by their reach instead.
    let tag = effect
        .tag()
        .filter(|_| !matches!(effect, Effect::Pierce | Effect::Chain | Effect::Blast));
    let parts: Vec<String> = tag.map(String::from).into_iter().chain(kind.reach()).collect();
    if parts.is_empty() {
        "ONE TARGET".into()
    } else {
        parts.join(" + ")
    }
}
/// Rarity, then the trait line.
fn rarity_line(card: &Card) -> String {
    format!("{}  /  {}", rarity_name(card), trait_line(card))
}
// Estimated damage per second, inked green or red against the equipped card.
fn dps_comparison(card: &Card, equipped: &Card) -> (String, C) {
    let dps = card.estimated_dps();
    let delta = (dps - equipped.estimated_dps()).round();
    let (versus, color) = if delta > 0. {
        (format!("+{delta:.0} VS EQUIPPED"), C::from_rgb(34, 92, 46))
    } else if delta < 0. {
        (format!("{delta:.0} VS EQUIPPED"), C::from_rgb(138, 30, 26))
    } else {
        ("SAME AS EQUIPPED".into(), INK)
    };
    (format!("EST. {dps:.0} DPS  /  {versus}"), color)
}
fn header(c: &Canvas, title: &str, sub: &str, gold: Option<u32>) {
    c.text(65., 54., title, 42., IVORY, true, Align2::LEFT_CENTER);
    c.text(67., 96., sub, 12., GOLD, false, Align2::LEFT_CENTER);
    c.flourish(210., 124., 288., GOLD.gamma_multiply(0.5));
    if let Some(gold) = gold {
        c.texture(
            "card_front",
            1175.,
            23.,
            205.,
            82.,
            C::from_rgb(220, 186, 123),
        );
        c.coin(1205., 64., 13.);
        c.center(1291., 64., format!("{gold} GOLD"), 20., INK);
    }
}
fn title(c: &Canvas, g: &mut Game) {
    let mut shade = egui::Mesh::default();
    for (x, alpha) in [(-100., 215), (400., 190), (750., 80), (1050., 0)] {
        shade.colored_vertex(c.pt(x, 0.), C::from_black_alpha(alpha));
        shade.colored_vertex(c.pt(x, c.h), C::from_black_alpha(alpha));
    }
    for i in 0..3 {
        let k = i * 2;
        shade.add_triangle(k, k + 1, k + 2);
        shade.add_triangle(k + 1, k + 3, k + 2);
    }
    c.p.add(Shape::mesh(shade));
    // Keep the bell and wordmark on the same axis as the main menu.
    let brand_center = 98. + 355. * 0.5;
    c.texture(
        "brand_emblem",
        brand_center - 87.,
        121.,
        174.,
        174.,
        C::WHITE,
    );
    c.text(
        brand_center,
        331.,
        "GRAVEWAKE",
        88.,
        IVORY,
        true,
        Align2::CENTER_CENTER,
    );
    c.line((98., 429.), (485., 429.), GOLD, 1.);
    c.diamond(508., 429., 6., C::from_rgb(116, 166, 139));
    c.text(
        98.,
        394.,
        "T H E   H O L L O W   T I T H E",
        17.,
        IVORY,
        false,
        Align2::LEFT_CENTER,
    );
    c.text(
        98.,
        481.,
        "The dead rise. The living owe.",
        20.,
        MUTED,
        true,
        Align2::LEFT_CENTER,
    );
    if c.button("new", 98., 548., 355., 55., "ANSWER THE BELL  >", true) {
        if g.has_save {
            g.confirm_new_run = true;
        } else {
            g.new_run();
        }
    }
    if g.has_save {
        if c.button(
            "continue",
            98.,
            615.,
            355.,
            44.,
            "CONTINUE YOUR DESCENT",
            false,
        ) {
            g.resume_save();
        }
    } else {
        c.text(
            98.,
            633.,
            "Twelve descents. A debt in souls.",
            13.,
            MUTED,
            false,
            Align2::LEFT_CENTER,
        );
    }
    if c.button("collection", 98., 680., 168., 41., "ARMORY", false) {
        g.open_book(Mode::Collection);
    }
    if c.button("bestiary", 280., 680., 173., 41., "BESTIARY", false) {
        g.open_book(Mode::Bestiary);
    }
    if c.button(
        "settings",
        98.,
        734.,
        355.,
        41.,
        "SETTINGS & CONTROLS",
        false,
    ) {
        g.settings = true;
    }
    if c.button("quit_title", 98., 789., 355., 41., "QUIT GAME", false) {
        g.quit_requested = true;
    }
    let r = &g.records;
    if r.runs > 0 {
        let fastest = r.fastest_victory.map_or("NONE YET".into(), |t| {
            format!("{:02}:{:02}", t as u32 / 60, t as u32 % 60)
        });
        c.text(
            98.,
            856.,
            format!(
                "DEEPEST DESCENT {:02}  /  MOST SOULS {}  /  FASTEST VICTORY {fastest}  /  {} {}",
                r.deepest,
                r.most_souls,
                r.runs,
                if r.runs == 1 { "RUN" } else { "RUNS" }
            ),
            13.,
            MUTED,
            false,
            Align2::LEFT_CENTER,
        );
    }
    chronicle(c, &r.history);
}
/// Runs the title's chronicle shows.
const CHRONICLE_SHOWN: usize = 5;
/// The chronicle panel: left edge, width and height of each run's rows.
const CHRONICLE_X: f32 = 898.;
const CHRONICLE_W: f32 = 490.;
const CHRONICLE_ROW: f32 = 58.;
/// How a past run ended, and its colour.
fn ending_line(past: &PastRun) -> (String, C) {
    match past.ending {
        Ending::Slain(Some(cause)) => (format!("SLAIN BY {}", cause.describe()), BLOOD),
        Ending::Slain(None) => ("SLAIN".into(), BLOOD),
        Ending::Victory => ("THE DEBT IS PAID".into(), GOLD),
        Ending::Abandoned => ("ABANDONED".into(), MUTED),
    }
}
/// A past run's time, souls, level and weapon, dropping the level and then
/// the weapon until the line fits `width`.
fn chronicle_details(c: &Canvas, past: &PastRun, width: f32) -> String {
    let time = format!("{:02}:{:02}", past.time as u32 / 60, past.time as u32 % 60);
    let souls = format!("{} SOULS", grouped(past.souls));
    let level = format!("LEVEL {}", past.level);
    let options = [
        vec![time.clone(), souls.clone(), level.clone(), past.weapon.clone()],
        vec![time.clone(), souls.clone(), past.weapon.clone()],
        vec![time.clone(), souls.clone(), level],
        vec![time, souls],
    ];
    let lines: Vec<String> = options.iter().map(|parts| parts.join("  /  ")).collect();
    lines
        .iter()
        .find(|line| c.text_width(line, 18.) <= width)
        .unwrap_or(&lines[3])
        .clone()
}
/// The latest runs, newest first, in a panel beside the scene: how each
/// ended and the descent it reached, then its time, souls, level and weapon.
fn chronicle(c: &Canvas, history: &[PastRun]) {
    if history.is_empty() {
        return;
    }
    let shown = &history[..history.len().min(CHRONICLE_SHOWN)];
    let (x, w, row) = (CHRONICLE_X, CHRONICLE_W, CHRONICLE_ROW);
    let height = 66. + shown.len() as f32 * row;
    let y = 838. - height;
    c.hud_panel(x, y, w, height);
    let right = x + w - 20.;
    c.text(x + 20., y + 28., "THE CHRONICLE", 19., GOLD, false, Align2::LEFT_CENTER);
    c.text(right, y + 28., "LAST RUNS", 18., MUTED, false, Align2::RIGHT_CENTER);
    c.line((x + 20., y + 50.), (right, y + 50.), GOLD.gamma_multiply(0.5), 1.);
    for (i, past) in shown.iter().enumerate() {
        let ty = y + 68. + i as f32 * row;
        let descent = if past.endless && past.descent > crate::survival::DESCENTS {
            format!("ENDLESS {:02}", past.descent)
        } else {
            format!("DESCENT {:02}", past.descent)
        };
        let (ending, color) = ending_line(past);
        let details = chronicle_details(c, past, w - 40.);
        // Each line's extent, for the layout test.
        let dw = c.text_width(&descent, 18.);
        c.layout("chronicle_descent", right - dw, ty - 10., dw, 20.);
        c.layout("chronicle_line", x + 20., ty - 10., c.text_width(&ending, 18.), 20.);
        c.layout("chronicle_line", x + 20., ty + 14., c.text_width(&details, 18.), 20.);
        c.text(right, ty, descent, 18., IVORY, false, Align2::RIGHT_CENTER);
        c.text(x + 20., ty, ending, 18., color, false, Align2::LEFT_CENTER);
        c.text(x + 20., ty + 24., details, 18., MUTED, false, Align2::LEFT_CENTER);
        if i + 1 < shown.len() {
            let ly = ty + row - 18.;
            c.line((x + 20., ly), (right, ly), GOLD.gamma_multiply(0.18), 0.8);
        }
    }
}
fn house(c: &Canvas, g: &mut Game) {
    backdrop(c, true);
    header(
        c,
        "THE COLLECTOR",
        &format!(
            "DESCENT {:02} CLEARED  /  FORTUNE FAVOURS THE DAMNED",
            g.run.wave
        ),
        Some(g.run.gold),
    );
    let y = 205.;
    let h = 425.;
    let w = 280.;
    card_base(c, 240., y, w, h, 0, true);
    c.text(
        380.,
        y + 286.,
        "WEAPON DRAW",
        25.,
        IVORY,
        true,
        Align2::CENTER_CENTER,
    );
    c.center(380., y + 317., "ONE WEAPON. ANOTHER CHANCE.", 10., GOLD);
    c.center(380., y + 340., "58% COMMON  /  2% LEGENDARY", 10., GOLD);
    if c.button(
        "draw",
        262.,
        y + h - 61.,
        236.,
        36.,
        &format!("DRAW    {} GOLD", 45 + g.run.draws * 10),
        true,
    ) {
        g.draw();
    }
    let offer = g.run.offer.clone();
    if weapon_card(
        c,
        "offer",
        580.,
        y,
        w,
        h,
        &offer,
        &format!("EQUIP    {} GOLD", 35 + offer.rarity as u32 * 20),
        Some(&g.run.weapon),
    ) {
        g.buy_offer();
    }
    card_base(c, 920., y, w, h, 0, false);
    c.center(1060., y + 35., "A R T I F A C T", 11., INK);
    chalice_art(c, 1060., y + 125., 1.05);
    c.inset(938., y + 210., 244., 146., PAPER);
    c.text(
        1060.,
        y + 240.,
        "HOLLOW CHALICE",
        23.,
        INK,
        true,
        Align2::CENTER_CENTER,
    );
    c.center(
        1060.,
        y + 278.,
        "EMBERBOUND",
        11.,
        C::from_rgb(106, 64, 111),
    );
    c.center(1060., y + 314., "UNLOCK EMBER BOLT", 12., INK);
    c.center(1060., y + 337., "95 DAMAGE  /  12 MANA", 11., INK);
    if c.button(
        "chalice",
        942.,
        y + h - 61.,
        236.,
        36.,
        if g.run.chalice {
            "BOUND TO YOUR SOUL"
        } else {
            "BIND    65 GOLD"
        },
        true,
    ) && !g.run.chalice
        && g.spend(65)
    {
        g.run.chalice = true;
        let key = g.prompt(Action::Bolt);
        g.notify(&format!("Ember Bolt awakened. Press {key} in the arena."));
        g.save();
    }
    c.text(
        244.,
        665.,
        "DRAW ODDS",
        14.,
        IVORY,
        true,
        Align2::LEFT_CENTER,
    );
    for (i, (label, pct, color)) in [
        ("Common", "58%", MUTED),
        ("Uncommon", "28%", C::from_rgb(86, 153, 180)),
        ("Rare", "12%", GOLD),
        ("Legendary", "2%", C::from_rgb(213, 116, 48)),
    ]
    .iter()
    .enumerate()
    {
        let x = 244. + (i % 2) as f32 * 171.;
        let y = 697. + (i / 2) as f32 * 35.;
        c.text(x, y, *label, 16., MUTED, false, Align2::LEFT_CENTER);
        c.text_role(
            x + 145.,
            y,
            *pct,
            17.,
            IVORY,
            TypeRole::Strong,
            Align2::RIGHT_CENTER,
        );
        c.line((x, y + 16.), (x + 145., y + 16.), *color, 1.);
    }
    c.texture("pack_art", 596., 648., 200., 104., C::WHITE);
    c.text(
        808.,
        682.,
        "ARMORY PACK",
        22.,
        IVORY,
        true,
        Align2::LEFT_CENTER,
    );
    c.text(
        808.,
        713.,
        "Reveal three. Keep one.",
        12.,
        MUTED,
        false,
        Align2::LEFT_CENTER,
    );
    if c.button(
        "pack",
        1036.,
        674.,
        164.,
        53.,
        &format!("OPEN  /  {} G", 30 + g.run.pack_buys * 15),
        true,
    ) {
        g.open_pack();
    }
    deck(c, g);
    // Over the Collector's robe, between the title and the purse.
    if let Some(active) = g.tip.filter(|t| t.tip.in_shop()) {
        field_tip(c, &active.tip, &active.tip.text(g), 14., 600., active.alpha());
    }
}
fn deck(c: &Canvas, g: &mut Game) {
    let y = c.h - 135.;
    c.fill(
        -100.,
        y,
        1640.,
        135.,
        C::from_rgba_unmultiplied(6, 9, 8, 248),
    );
    c.line((42., y), (1398., y), GOLD.gamma_multiply(0.6), 1.);
    c.text(
        65.,
        y + 35.,
        "YOUR DECK",
        20.,
        IVORY,
        true,
        Align2::LEFT_CENTER,
    );
    c.text(
        65.,
        y + 66.,
        format!("VITALITY  {:.0}", g.run.hp),
        11.,
        MUTED,
        false,
        Align2::LEFT_CENTER,
    );
    c.text(
        65.,
        y + 88.,
        format!("ARMOR     {:.0}", g.run.armor),
        11.,
        MUTED,
        false,
        Align2::LEFT_CENTER,
    );
    let card = g.run.weapon.clone();
    card_base(c, 280., y + 16., 70., 103., card.rarity, false);
    weapon_art(c, 311., y + 69., 54., &card, 0.);
    let equipped = c.ui.interact(
        c.rect(280., y + 16., 70., 103.),
        Id::new("equipped"),
        Sense::click(),
    );
    c.target(Target::button(equipped.rect));
    if equipped.clicked() {
        g.mode = Mode::Tree;
    }
    c.paragraph_role(
        380.,
        y + 15.,
        &card.name(),
        373.,
        18.,
        IVORY,
        false,
        TypeRole::Strong,
    );
    if c.button("upgrade", 380., y + 76., 164., 35., "UPGRADE CARD", false) {
        g.mode = Mode::Tree;
    }
    if c.button("armor", 795., y + 81., 230., 35., "ARMOR +30: 40G", false) && g.spend(40) {
        g.run.armor += 30.;
        g.save();
    }
    if c.button("codex", 578., y + 76., 175., 35., "ARMORY", false) {
        g.open_book(Mode::Collection);
    }
    c.text(
        795.,
        y + 26.,
        if g.run.chalice {
            "EMBER BOLT"
        } else {
            "NO PACT BOUND"
        },
        12.,
        if g.run.chalice { GOLD } else { MUTED },
        false,
        Align2::LEFT_CENTER,
    );
    c.text(
        795.,
        y + 53.,
        format!("{} SOULS RETURNED", g.run.kills),
        11.,
        MUTED,
        false,
        Align2::LEFT_CENTER,
    );
    if c.button(
        "next",
        1080.,
        y + 14.,
        292.,
        56.,
        &format!("ENTER DESCENT {:02}  >", g.run.wave + 1),
        true,
    ) {
        g.next_wave();
    }
    // What waits below: the count, the Tithekeeper, and first-time species.
    let next = crate::survival::preview(g.run.wave + 1);
    let mut stakes = format!("{} FOES", next.foes);
    if next.boss {
        stakes += if next.boss_returns {
            "  /  THE TITHEKEEPER RETURNS"
        } else {
            "  /  THE TITHEKEEPER WAITS"
        };
    }
    c.center(1226., y + 88., stakes, 11., if next.boss { BLOOD } else { GOLD });
    let names: Vec<&str> = next
        .new
        .iter()
        .map(|&k| crate::encounters::species(k).name)
        .collect();
    let new = if names.is_empty() {
        "SAME FOREST. HIGHER STAKES.".into()
    } else {
        format!("NEW: {}", names.join(", "))
    };
    c.center(1226., y + 110., new, 9., MUTED);
}
#[derive(Clone)]
struct PackUi {
    opened_at: Option<f32>,
    last_age: f32,
    revealed_at: [f32; 3],
    revealed: [bool; 3],
    played_flip: [bool; 3],
    selected: Option<usize>,
    take_at: Option<f32>,
}
// Tessellate the actual card (including type, weapon and text), then rotate
// that complete surface in perspective. This avoids swapping/fading two images.
fn moving_card(
    c: &Canvas,
    card: &Card,
    back: bool,
    center: Vec2,
    scale: f32,
    yaw: f32,
    roll: f32,
    opacity: f32,
    id: usize,
) {
    let layer = egui::LayerId::new(egui::Order::Foreground, Id::new(("motion_card", id)));
    let mut surface = Canvas::new(c.ui, c.time);
    surface.p = egui::Painter::new(c.ui.ctx().clone(), layer, c.p.clip_rect());
    if back {
        card_base(&surface, 0., 0., 280., 425., 0, true);
        surface.text(
            140.,
            310.,
            "GRAVEWAKE",
            26.,
            IVORY,
            true,
            Align2::CENTER_CENTER,
        );
    } else {
        weapon_card(
            &surface,
            &format!("motion_face_{id}"),
            0.,
            0.,
            280.,
            425.,
            card,
            "",
            None,
        );
    }
    let shapes = c.ui.ctx().graphics_mut(|g| {
        let list = g.entry(layer);
        let shapes = list.all_entries().cloned().collect::<Vec<_>>();
        *list = Default::default();
        shapes
    });
    let origin = c.pt(140., 212.5);
    let destination = c.pt(center.x, center.y);
    for primitive in c.ui.ctx().tessellate(shapes, c.ui.ctx().pixels_per_point()) {
        if let egui::epaint::Primitive::Mesh(mut mesh) = primitive.primitive {
            for v in &mut mesh.vertices {
                let local = (v.pos - origin) / c.s;
                let perspective = 1100. / (1100. + local.x * yaw.sin());
                let x = local.x * yaw.cos().abs() * perspective * scale;
                let y = local.y * perspective * scale;
                v.pos = destination
                    + Vec2::new(
                        x * roll.cos() - y * roll.sin(),
                        x * roll.sin() + y * roll.cos(),
                    ) * c.s;
                v.color = v
                    .color
                    .gamma_multiply(opacity * (0.7 + 0.3 * yaw.cos().abs()));
            }
            c.p.add(Shape::mesh(mesh));
        }
    }
}
fn torn_wrapper(c: &Canvas, age: f32) {
    use crate::motion::{out, smooth};
    let Some(texture) =
        c.ui.ctx()
            .data(|d| d.get_temp::<egui::TextureHandle>(Id::new("pack_art")))
    else {
        return;
    };
    let tear = smooth(0.18, 0.91, age);
    let discard = smooth(1.0, 1.95, age);
    let scale = 1. - discard * 0.70;
    let center = Vec2::new(720. - discard * 565., 425. + discard * 307.);
    let roll = -discard * 0.17;
    let tension = (age * 58.).sin() * crate::motion::window(0., 0.12, 0.19, 0.32, age) * 4.;
    let project = |p: Vec2| {
        let q = (p - Vec2::new(285., 145.)) * scale;
        let q = Vec2::new(
            q.x * roll.cos() - q.y * roll.sin(),
            q.x * roll.sin() + q.y * roll.cos(),
        );
        c.pt(center.x + q.x + tension, center.y + q.y)
    };
    for strip in [false, true] {
        let mut mesh = egui::Mesh::with_texture(texture.id());
        for i in 0..=64 {
            let u = i as f32 / 64.;
            let seam = 0.218 + (hash(i + 40) - 0.5) * 0.025;
            let peeled = out(((tear - u) * 5.).clamp(0., 1.));
            for edge in 0..2 {
                let v = if strip {
                    if edge == 0 { 0. } else { seam }
                } else if edge == 0 {
                    seam
                } else {
                    1.
                };
                let mut p = Vec2::new(u * 570., v * 290.);
                if strip {
                    let detach = smooth(0.91, 1.6, age);
                    p.x += peeled * 24. + detach * 170.;
                    p.y -= peeled * (40. + (u * 11. + age * 8.).sin() * 10.);
                    p.y += detach * detach * 370.;
                    p.x += (v - seam) * detach * 140.;
                } else {
                    p.y += peeled * (1. - edge as f32) * 4.;
                }
                mesh.vertices.push(egui::epaint::Vertex {
                    pos: project(p),
                    uv: Pos2::new(u, v),
                    color: C::WHITE.gamma_multiply(if strip {
                        1. - smooth(1.25, 1.65, age)
                    } else {
                        1.
                    }),
                });
            }
            if i < 64 {
                let k = i * 2;
                mesh.add_triangle(k, k + 1, k + 2);
                mesh.add_triangle(k + 1, k + 3, k + 2);
            }
        }
        c.p.add(Shape::mesh(mesh));
    }
    if age < 1.2 && age > 0.18 {
        let front = project(Vec2::new(tear * 570., 63.));
        for r in (1..8).rev() {
            c.p.circle_filled(
                front,
                r as f32 * 4. * c.s,
                C::from_rgba_unmultiplied(255, 210, 125, 6),
            );
        }
        for i in 0..32 {
            let birth = 0.18 + i as f32 / 32. * 0.73;
            let life = age - birth;
            if !(0.0..0.5).contains(&life) {
                continue;
            }
            let p = project(Vec2::new(
                i as f32 / 32. * 570. + life * (hash(i + 4) - 0.5) * 160.,
                65. - life * 160. + life * life * 260.,
            ));
            c.p.add(Shape::convex_polygon(
                vec![p, p + Vec2::new(5., -3.) * c.s, p + Vec2::new(2., 5.) * c.s],
                C::from_rgba_unmultiplied(221, 193, 241, ((1. - life * 2.) * 220.) as u8),
                Stroke::NONE,
            ));
        }
    }
}
fn pack(c: &Canvas, g: &mut Game) {
    use crate::motion::{out, smooth};
    backdrop(c, true);
    header(
        c,
        "ARMORY PACK",
        "THREE CARDS. ONE PACT. YOUR FATE.",
        Some(g.run.gold),
    );
    let key = Id::new(("pack_state", g.run.seed));
    let mut state =
        c.ui.ctx()
            .data_mut(|d| d.get_temp::<PackUi>(key))
            .unwrap_or(PackUi {
                opened_at: None,
                last_age: -1.,
                revealed_at: [-100.; 3],
                revealed: [false; 3],
                played_flip: [false; 3],
                selected: None,
                take_at: None,
            });
    let age = state.opened_at.map(|t| c.time - t).unwrap_or(-1.);
    if age < 0. {
        let bob = (c.time * 1.2).sin() * 4.;
        c.texture("pack_art", 435., 280. + bob, 570., 290., C::WHITE);
        c.center(
            720.,
            613.,
            "SEALED WITH A PROMISE. PAID FOR IN BLOOD.",
            11.,
            GOLD,
        );
        if c.button("tear_open", 565., 665., 310., 50., "TEAR OPEN", true) {
            state.opened_at = Some(c.time);
            g.sound_events.push("pack_tear");
        }
    } else {
        for threshold in [0.95, 1.10, 1.25] {
            if state.last_age < threshold && age >= threshold {
                g.sound_events.push("card_deal");
            }
        }
        state.last_age = age;
        for i in 0..3 {
            if state.revealed[i] && !state.played_flip[i] && c.time >= state.revealed_at[i] {
                g.sound_events.push("card_flip");
                state.played_flip[i] = true;
            }
        }
        for i in 0..3 {
            let card = g.run.choices.get(i).cloned().unwrap_or_else(Card::starter);
            let x = 240. + i as f32 * 340.;
            let y = 205.;
            if let Some(take_at) = state.take_at {
                let t = ((c.time - take_at) / 0.7).clamp(0., 1.);
                if state.selected == Some(i) {
                    let t = out(t);
                    moving_card(
                        c,
                        &card,
                        false,
                        Vec2::new(x + 140., y + 212.5) * (1. - t) + Vec2::new(318., 825.) * t,
                        1. - t * 0.78,
                        0.,
                        -0.1 * (t * std::f32::consts::PI).sin(),
                        1.,
                        i,
                    );
                } else {
                    moving_card(
                        c,
                        &card,
                        true,
                        Vec2::new(x + 140., y + 212.5 + t * t * 450.),
                        1. - t * 0.25,
                        t * 0.4,
                        (i as f32 - 1.) * t * 0.25,
                        1. - t,
                        i,
                    );
                }
                continue;
            }
            let deal = ((age - 0.95 - i as f32 * 0.15) / 0.70).clamp(0., 1.);
            if deal <= 0. {
                continue;
            }
            if deal < 1. {
                let travel = out(deal);
                let center = Vec2::new(720., 430.) * (1. - travel)
                    + Vec2::new(x + 140., y + 212.5) * travel
                    - Vec2::Y * (deal * std::f32::consts::PI).sin() * 105.;
                moving_card(
                    c,
                    &card,
                    true,
                    center,
                    0.48 + travel * 0.52,
                    (1. - travel) * 0.25,
                    (i as f32 - 1.) * (1. - travel) * 0.35,
                    1.,
                    i,
                );
            } else if state.revealed[i] {
                let t = ((c.time - state.revealed_at[i]) / 0.56).clamp(0., 1.);
                if t < 1. {
                    let turn = smooth(0., 1., t) * std::f32::consts::PI;
                    moving_card(
                        c,
                        &card,
                        t < 0.5,
                        Vec2::new(x + 140., y + 212.5 - (t * std::f32::consts::PI).sin() * 22.),
                        1. + (t * std::f32::consts::PI).sin() * 0.035,
                        turn,
                        0.,
                        1.,
                        i,
                    );
                    if t > 0.48 && t < 0.9 {
                        let glow = (1. - ((t - 0.55) / 0.35).abs()).max(0.);
                        c.border(x - 3., y - 3., 286., 431., GOLD.gamma_multiply(glow));
                    }
                } else {
                    if weapon_card(
                        c,
                        &format!("choose{i}"),
                        x,
                        y,
                        280.,
                        425.,
                        &card,
                        if state.selected == Some(i) {
                            "SELECTED"
                        } else {
                            "CHOOSE CARD"
                        },
                        Some(&g.run.weapon),
                    ) {
                        state.selected = Some(i);
                        g.sound_events.push("card_deal");
                    }
                    if state.selected == Some(i) {
                        c.border(x - 5., y - 5., 290., 435., IVORY);
                    }
                }
            } else {
                card_base(c, x, y, 280., 425., 0, true);
                c.text(
                    x + 140.,
                    y + 306.,
                    "GRAVEWAKE",
                    26.,
                    IVORY,
                    true,
                    Align2::CENTER_CENTER,
                );
                if age >= 2.
                    && c.button(
                        &format!("reveal{i}"),
                        x + 22.,
                        y + 364.,
                        236.,
                        36.,
                        "TURN CARD",
                        true,
                    )
                {
                    state.revealed[i] = true;
                    state.revealed_at[i] = c.time;
                }
            }
        }
        // Cards emerge from behind the packet; discarded wrapper stays on the table.
        torn_wrapper(c, age);
        if age >= 2. && state.take_at.is_none() {
            c.center(
                720.,
                672.,
                format!(
                    "{} / 3 REVEALED",
                    state.revealed.iter().filter(|&&x| x).count()
                ),
                11.,
                MUTED,
            );
            if state.revealed.iter().any(|x| !*x) {
                if c.button("reveal_all", 565., 703., 310., 50., "REVEAL ALL", true) {
                    for i in 0..3 {
                        if !state.revealed[i] {
                            state.revealed_at[i] = c.time + i as f32 * 0.18;
                            state.revealed[i] = true;
                        }
                    }
                }
            } else if let Some(_) = state.selected {
                if c.button("take", 565., 703., 310., 50., "TAKE & EQUIP  >", true) {
                    state.take_at = Some(c.time);
                    g.sound_events.push("card_take");
                }
            } else {
                c.center(
                    720.,
                    726.,
                    "Choose the weapon that will carry you deeper.",
                    13.,
                    IVORY,
                );
            }
        }
        if state.take_at.is_some_and(|t| c.time - t >= 0.7) {
            if let Some(i) = state.selected {
                g.select_pack(i);
            }
        }
    }
    c.ui.ctx().data_mut(|d| d.insert_temp(key, state));
    c.text(
        65.,
        c.h - 44.,
        "PURCHASED PACK  /  NO ADDITIONAL COST TO EQUIP",
        10.,
        MUTED,
        false,
        Align2::LEFT_CENTER,
    );
}

fn tree(c: &Canvas, g: &mut Game) {
    backdrop(c, false);
    c.border(28., 26., 1384., c.h - 52., GOLD.gamma_multiply(0.5));
    header(
        c,
        "THE BINDING",
        "INVEST IN THE WEAPON THAT BROUGHT YOU THIS FAR.",
        Some(g.run.gold),
    );
    let card = g.run.weapon.clone();
    let start =
        c.ui.ctx()
            .data(|d| d.get_temp::<(Mode, f32)>(Id::new("mode_transition")))
            .map(|(_, t)| t)
            .unwrap_or(c.time);
    let t = ((c.time - start) / 0.4).clamp(0., 1.);
    let t = 1. - (1. - t).powi(3);
    weapon_card(
        c,
        "focused",
        280. + (74. - 280.) * t,
        (c.h - 119.) + (240. - (c.h - 119.)) * t,
        70. + 222. * t,
        103. + 342. * t,
        &card,
        "",
        None,
    );
    c.center(220., 732., "UPGRADES STAY WITH THIS CARD", 10., MUTED);
    c.line(
        (423., 175.),
        (423., c.h - 86.),
        GOLD.gamma_multiply(0.3),
        1.,
    );
    let names = ["RUIN", "HASTE", "EMBER"];
    let descriptions = [
        "+15% weapon damage",
        if card.kind.melee() {
            "+10% swing & recovery speed"
        } else {
            "+10% fire & reload speed"
        },
        "+1 mana / second",
    ];
    c.seal(821., 186., 30., GOLD);
    for path in 0..3 {
        let x = 587. + path as f32 * 234.;
        c.line((821., 217.), (x, 257.), GOLD.gamma_multiply(0.35), 1.);
        c.center(x, 274., names[path], 13., GOLD);
        c.paragraph(x, 291., descriptions[path], 205., 16., MUTED, true);
        for level in 0..5 {
            let y = 352. + level as f32 * 73.;
            let bought = card.paths[path] > level;
            let available = card.paths[path] == level;
            let col = if bought {
                GOLD
            } else if available {
                IVORY
            } else {
                C::from_rgb(70, 74, 65)
            };
            if level > 0 {
                c.line(
                    (x, y - 48.),
                    (x, y - 23.),
                    if bought {
                        GOLD
                    } else {
                        col.gamma_multiply(0.6)
                    },
                    1.5,
                );
            }
            let r = c.ui.interact(
                c.rect(x - 28., y - 25., 56., 50.),
                Id::new(("node", path, level)),
                Sense::click(),
            );
            c.target(Target::button(r.rect));
            c.medallion(x, y, 23., bought || available || r.hovered());
            match path {
                0 => {
                    c.line((x - 6., y + 9.), (x + 7., y - 9.), col, 2.);
                    c.line((x - 8., y + 1.), (x + 1., y + 8.), col, 1.5);
                    c.diamond(x + 7., y - 9., 3., col);
                }
                1 => {
                    c.p.circle_stroke(c.pt(x, y), 10. * c.s, Stroke::new(c.s, col));
                    c.line((x, y), (x, y - 7.), col, 1.5);
                    c.line((x, y), (x + 5., y + 2.), col, 1.5);
                }
                _ => {
                    c.p.add(Shape::convex_polygon(
                        vec![
                            c.pt(x, y - 12.),
                            c.pt(x + 8., y + 3.),
                            c.pt(x + 3., y + 10.),
                            c.pt(x - 5., y + 9.),
                            c.pt(x - 8., y + 1.),
                        ],
                        col,
                        Stroke::NONE,
                    ));
                }
            }
            if available {
                c.center(
                    x + 67.,
                    y,
                    format!("{} G", 15 + level as u32 * 12),
                    11.,
                    GOLD,
                );
            }
            if r.hovered() {
                c.ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                r.clone().on_hover_text(if bought {
                    "Already bound to this card".into()
                } else if available {
                    format!(
                        "{}\nCost: {} gold",
                        descriptions[path],
                        15 + level as u32 * 12
                    )
                } else {
                    "Purchase the preceding node first".into()
                });
            }
            if r.clicked() && available {
                g.upgrade(path);
            }
        }
        c.line((x, 668.), (821., 721.), GOLD.gamma_multiply(0.35), 1.);
    }
    c.seal(821., 754., 31., if card.major { IVORY } else { GOLD });
    c.text(
        903.,
        746.,
        "SOUL SIPHON",
        19.,
        IVORY,
        true,
        Align2::LEFT_CENTER,
    );
    c.text(
        903.,
        773.,
        "Complete a path. Damage restores life.",
        10.,
        MUTED,
        false,
        Align2::LEFT_CENTER,
    );
    if card.paths.contains(&5)
        && !card.major
        && c.button("ascend", 708., 809., 226., 39., "BIND  /  120 GOLD", true)
    {
        g.ascend();
    }
    if c.button(
        "treeback",
        1171.,
        c.h - 79.,
        194.,
        39.,
        "BACK TO SHOP",
        false,
    ) {
        g.mode = Mode::Shop;
    }
}
fn collection(c: &Canvas, g: &mut Game) {
    use crate::weapons::{GROUPS, WeaponKind};
    backdrop(c, false);
    header(
        c,
        "THE ARMORY",
        "33 WEAPONS  /  EVERY IMPLEMENT HAS ITS OWN PURPOSE.",
        None,
    );
    for (i, group) in GROUPS.iter().enumerate() {
        if c.button(
            &format!("family{i}"),
            62. + i as f32 * 221.,
            144.,
            207.,
            48.,
            group,
            g.collection_group == i,
        ) {
            g.collection_group = i;
            g.collection_kind = WeaponKind::ALL
                .iter()
                .position(|k| k.spec().group == i)
                .unwrap();
            g.sound_events.push("card_deal");
        }
    }
    let kind = WeaponKind::ALL[g.collection_kind.min(32)];
    let spec = kind.spec();
    c.text(
        75.,
        231.,
        "Choose an implement",
        21.,
        IVORY,
        true,
        Align2::LEFT_CENTER,
    );
    for (row, k) in WeaponKind::ALL
        .iter()
        .filter(|k| k.spec().group == g.collection_group)
        .enumerate()
    {
        let y = 269. + row as f32 * 61.;
        let selected = *k == kind;
        if c.button(
            &format!("armory_{}", *k as usize),
            66.,
            y,
            356.,
            48.,
            k.spec().name,
            selected,
        ) {
            g.collection_kind = *k as usize;
            g.sound_events.push("card_deal");
        }
    }
    let card = Card {
        kind,
        rarity: g.collection_rarity,
        ..Card::starter()
    };
    let start =
        c.ui.ctx()
            .data(|d| d.get_temp::<(usize, f32)>(Id::new("armory_motion")));
    if start.is_none_or(|(i, _)| i != g.collection_kind) {
        c.ui.ctx()
            .data_mut(|d| d.insert_temp(Id::new("armory_motion"), (g.collection_kind, c.time)));
    }
    let age = start
        .filter(|(i, _)| *i == g.collection_kind)
        .map(|(_, t)| c.time - t)
        .unwrap_or(0.);
    if age < 0.3 {
        moving_card(
            c,
            &card,
            false,
            Vec2::new(662., 497.),
            1.13,
            (1. - crate::motion::out(age / 0.3)) * 0.65,
            0.,
            crate::motion::out(age / 0.15),
            70,
        );
    } else {
        weapon_card(c, "armory_card", 493., 240., 338., 514., &card, "", None);
    }
    c.folio(882., 238., 487., 510.);
    c.text(
        1125.,
        292.,
        GROUPS[spec.group],
        28.,
        INK,
        true,
        Align2::CENTER_CENTER,
    );
    c.flourish(1125., 329., 325., INK);
    c.paragraph(923., 353., spec.description, 400., 20., INK, false);
    for (i, (label, value)) in [
        ("Damage", format!("{:.0}", card.damage())),
        ("Reach", format!("{:.1} metres", spec.range)),
        ("Rhythm", format!("{:.2}s / attack", card.interval())),
        ("Trait", format!("{:?}", spec.effect)),
    ]
    .iter()
    .enumerate()
    {
        let y = 476. + i as f32 * 34.;
        c.text(923., y, *label, 16., INK, true, Align2::LEFT_CENTER);
        c.text(1323., y, value, 16., INK, true, Align2::RIGHT_CENTER);
        c.line(
            (923., y + 17.),
            (1323., y + 17.),
            GOLD.gamma_multiply(0.5),
            0.6,
        );
    }
    if c.button(
        "try_weapon",
        927.,
        656.,
        400.,
        52.,
        "Try in the practice grounds",
        true,
    ) {
        g.practice(card.clone());
    }
    c.center(1125., 718., "Your current run stays untouched.", 11., INK);
    for (i, label) in ["Common", "Uncommon", "Rare", "Legendary"]
        .iter()
        .enumerate()
    {
        if c.button(
            &format!("rarity{i}"),
            676. + i as f32 * 128.,
            780.,
            125.,
            32.,
            label,
            g.collection_rarity == i,
        ) {
            g.collection_rarity = i;
        }
    }
    c.text(
        68.,
        c.h - 43.,
        "Acquire weapons through Collector offers, draws and Armory Packs.",
        13.,
        MUTED,
        true,
        Align2::LEFT_CENTER,
    );
    if c.button(
        "bookback",
        1170.,
        c.h - 79.,
        195.,
        44.,
        "Close the armory",
        false,
    ) {
        g.back();
    }
}
fn bestiary(c: &Canvas, g: &mut Game) {
    c.fill(
        -100.,
        0.,
        560.,
        c.h,
        C::from_rgba_unmultiplied(5, 10, 10, 245),
    );
    c.fill(
        1020.,
        0.,
        520.,
        c.h,
        C::from_rgba_unmultiplied(5, 10, 10, 220),
    );
    c.folio(1022., 188., 386., 533.);
    header(c, "THE BESTIARY", "THE UNQUIET OF MOURNHOLLOW", None);
    let names: Vec<_> = crate::encounters::ROSTER.iter().map(|s| s.name).collect();
    for (i, n) in names.iter().enumerate() {
        if c.button(
            &format!("beast{i}"),
            45.,
            168. + i as f32 * 43.,
            365.,
            37.,
            n,
            g.bestiary_index == i,
        ) {
            g.bestiary_index = i;
        }
    }
    c.text(
        1060.,
        230.,
        format!("{:02}  /  UNQUIET SOUL", g.bestiary_index + 1),
        12.,
        INK,
        false,
        Align2::LEFT_CENTER,
    );
    for (i, line) in names[g.bestiary_index].split(' ').enumerate() {
        c.text(
            1060.,
            284. + i as f32 * 43.,
            line,
            29.,
            INK,
            true,
            Align2::LEFT_CENTER,
        );
    }
    let desc = crate::encounters::species(g.bestiary_index).lore.join(" ");
    c.paragraph(1060., 435., &desc, 309., 18., INK, false);
    c.line((1060., 603.), (1369., 603.), GOLD.gamma_multiply(0.5), 1.);
    c.text(
        1060.,
        638.,
        crate::encounters::species(g.bestiary_index).role,
        11.,
        INK,
        false,
        Align2::LEFT_CENTER,
    );
    if c.button("bestiaryback", 65., c.h - 105., 323., 45., "BACK", false) {
        g.back();
    }
}
fn relative_bearing(bearing: f32, yaw: f32) -> f32 {
    let angle = bearing - yaw;
    angle.sin().atan2(angle.cos())
}
fn cardinal(bearing: f32) -> &'static str {
    const NAMES: [&str; 8] = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];
    NAMES[((bearing.to_degrees().rem_euclid(360.) + 22.5) / 45.) as usize % 8]
}
fn direction_pointer(c: &Canvas, x: f32, y: f32, angle: f32, color: C) {
    let (s, co) = angle.sin_cos();
    let point = |a: f32, b: f32| c.pt(x + a * co - b * s, y + a * s + b * co);
    c.p.add(Shape::convex_polygon(
        vec![point(0., -8.), point(5., 6.), point(0., 3.), point(-5., 6.)],
        color,
        Stroke::NONE,
    ));
}
/// One quiet orientation strip for the larger grounds. Cardinal ticks turn with
/// the camera; a diamond and explicit bearing point toward the next district.
fn world_compass(c: &Canvas, g: &Game, y: f32, half: f32) {
    use crate::world_layout::{DISTRICTS, district_at};
    let here = district_at(g.run.pos);
    let next = DISTRICTS
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != here)
        .min_by(|(_, a), (_, b)| {
            a.center
                .distance_squared(g.run.pos)
                .total_cmp(&b.center.distance_squared(g.run.pos))
        })
        .map(|(_, district)| district)
        .unwrap();
    let direction = next.center - g.run.pos;
    let bearing = direction.x.atan2(-direction.z);
    let relative = relative_bearing(bearing, g.run.yaw);
    let distance = glam::Vec2::new(direction.x, direction.z).length();
    // Laid out at y 120; `y` moves it down.
    let c = &c.shifted(0., y - 120.);
    let (left, right) = (720. - half, 720. + half);
    c.hud_panel(left, 120., half * 2., 70.);
    c.line((left + 23., 152.), (right - 23., 152.), C::from_rgb(78, 70, 51), 1.);
    for tick in 0..16 {
        let angle = relative_bearing(tick as f32 * std::f32::consts::TAU / 16., g.run.yaw);
        if angle.abs() <= 1.47 {
            let x = 720. + angle / 1.47 * 192.;
            c.line(
                (x, 148.),
                (x, if tick % 2 == 0 { 144. } else { 146. }),
                GOLD,
                1.,
            );
            if tick % 2 == 0 {
                c.text_role(
                    x,
                    136.,
                    cardinal(tick as f32 * std::f32::consts::TAU / 16.),
                    16.,
                    if angle.abs() < 0.25 { IVORY } else { MUTED },
                    TypeRole::Strong,
                    Align2::CENTER_CENTER,
                );
            }
        }
    }
    c.diamond(
        720. + relative.clamp(-1.47, 1.47) / 1.47 * 192.,
        153.,
        5.,
        GOLD,
    );
    c.line((720., 121.), (720., 126.), GOLD, 2.);
    direction_pointer(c, left + 23., 174., relative, GOLD);
    c.text_role(
        left + 48.,
        174.,
        next.name,
        16.,
        IVORY,
        TypeRole::Strong,
        Align2::LEFT_CENTER,
    );
    c.text(
        right - 19.,
        174.,
        format!("{}  /  {:.0} m", cardinal(bearing), distance),
        16.,
        GOLD,
        false,
        Align2::RIGHT_CENTER,
    );
}
fn last_threat(c: &Canvas, g: &Game, y: f32) -> bool {
    let living = g.run.enemies.iter().filter(|enemy| enemy.hp > 0.).count();
    if g.run.survival.remaining != 0 || living == 0 || living > 3 {
        return false;
    }
    let enemy = g
        .run
        .enemies
        .iter()
        .filter(|enemy| enemy.hp > 0.)
        .min_by(|a, b| {
            a.pos
                .distance_squared(g.run.pos)
                .total_cmp(&b.pos.distance_squared(g.run.pos))
        })
        .unwrap();
    let direction = enemy.pos - g.run.pos;
    let bearing = direction.x.atan2(-direction.z);
    let distance = glam::Vec2::new(direction.x, direction.z).length();
    // Laid out at y 198; `y` moves it down.
    let c = &c.shifted(0., y - 198.);
    c.hud_panel(535., 198., 370., 35.);
    direction_pointer(c, 555., 216., relative_bearing(bearing, g.run.yaw), GOLD);
    c.text_role(
        578.,
        216.,
        if living == 1 {
            "LAST THREAT".into()
        } else {
            format!("{living} THREATS LEFT")
        },
        17.,
        IVORY,
        TypeRole::Strong,
        Align2::LEFT_CENTER,
    );
    c.text(
        887.,
        216.,
        format!("{} / {:.0} m", cardinal(bearing), distance),
        16.,
        GOLD,
        false,
        Align2::RIGHT_CENTER,
    );
    true
}

/// The aiming reticle: four ticks and a centre dot, `size` times the
/// original, each over a dark outline so it reads on pale stone, bone and
/// firelight as well as the night. `spread` widens the gap while firing.
fn reticle(c: &Canvas, cx: f32, cy: f32, size: f32, spread: f32, color: C) {
    // Whole-pixel widths, centred on a pixel (odd widths) or between two
    // (even), so a thin tick is one sharp row rather than two faint ones.
    let to_px = c.s * c.ui.ctx().pixels_per_point();
    let stroke_px = (size * to_px).round().max(1.);
    let pad_px = to_px.round().max(1.);
    let snap = |v: f32, offset: f32| {
        let px = v * to_px + offset * to_px / c.s;
        let snapped = if stroke_px as i32 % 2 == 1 {
            (px - 0.5).round() + 0.5
        } else {
            px.round()
        };
        v + (snapped - px) / to_px
    };
    let (cx, cy) = (snap(cx, c.offset.x), snap(cy, c.offset.y));
    let (width, pad) = (stroke_px / to_px, pad_px / to_px);
    let gap = 5. * size + spread;
    let length = 6. * size;
    let outline = C::from_black_alpha(200);
    // The outline first, a pixel wider on every side, then the colour.
    for (paint, pad) in [(outline, pad), (color, 0.)] {
        let (near, far) = (gap - pad, gap + length + pad);
        for side in [-1., 1.] {
            c.line((cx + side * near, cy), (cx + side * far, cy), paint, width + 2. * pad);
            c.line((cx, cy + side * near), (cx, cy + side * far), paint, width + 2. * pad);
        }
        c.p.circle_filled(c.pt(cx, cy), (width + pad) * c.s, paint);
    }
}
// Reticle ticks for the player's own hits and arcs toward recent damage.
fn hit_feedback(c: &Canvas, g: &Game, cx: f32, cy: f32) {
    for mark in &g.damage_marks {
        let fade = (mark.life / DAMAGE_MARK_LIFE).clamp(0., 1.).powf(0.7);
        let centre = relative_bearing(mark.bearing, g.run.yaw);
        let point = |a: f32, r: f32| (cx + a.sin() * r, cy - a.cos() * r);
        for (width, color) in [
            (7., C::from_black_alpha((150. * fade) as u8)),
            (
                4.,
                C::from_rgba_unmultiplied(214, 46, 34, (235. * fade) as u8),
            ),
        ] {
            for k in 0..8 {
                let a0 = centre - 0.3 + 0.6 * k as f32 / 8.;
                let a1 = centre - 0.3 + 0.6 * (k + 1) as f32 / 8.;
                c.line(point(a0, 96.), point(a1, 96.), color, width);
            }
        }
        let tip = point(centre, 108.);
        let left = point(centre - 0.07, 101.);
        let right = point(centre + 0.07, 101.);
        c.p.add(Shape::convex_polygon(
            vec![
                c.pt(tip.0, tip.1),
                c.pt(right.0, right.1),
                c.pt(left.0, left.1),
            ],
            C::from_rgba_unmultiplied(214, 46, 34, (235. * fade) as u8),
            Stroke::NONE,
        ));
    }
    let Some(marker) = g.hit_marker else { return };
    let fade = (marker.life / HIT_MARKER_LIFE).clamp(0., 1.);
    let (inner, outer, width, color) = match marker.kind {
        HitKind::Body => (9., 16., 1.6, IVORY),
        HitKind::Head => (9., 18., 2.2, GOLD),
        HitKind::Kill => (10., 22., 2.8, C::from_rgb(222, 70, 52)),
    };
    let alpha = |c: C| c.gamma_multiply(fade);
    for (dx, dy) in [(-1., -1.), (1., -1.), (-1., 1.), (1., 1.)] {
        let d = std::f32::consts::FRAC_1_SQRT_2;
        let a = (cx + dx * d * inner, cy + dy * d * inner);
        let b = (cx + dx * d * outer, cy + dy * d * outer);
        c.line(a, b, alpha(C::from_black_alpha(160)), width + 2.);
        c.line(a, b, alpha(color), width);
    }
}
/// Amber chevrons outside the damage arcs, pointing toward special attacks
/// winding up out of view. They grow as the attack nears.
fn threat_pointers(c: &Canvas, g: &Game, cx: f32, cy: f32) {
    let screen = c.ui.max_rect();
    let aspect = screen.width() / screen.height().max(1.);
    let half_view = ((g.prefs.fov.to_radians() * 0.5).tan() * aspect).atan();
    // Slightly inside the frame edge: a creature at the border is easy to miss.
    for threat in g.unseen_threats(half_view * 0.92) {
        let angle = relative_bearing(threat.bearing, g.run.yaw);
        let pulse = if g.prefs.reduce_flashes {
            1.
        } else {
            0.7 + 0.3 * (c.time * (7. + 9. * threat.urgency)).sin()
        };
        let size = 1.5 + 0.7 * threat.urgency;
        let (s, co) = angle.sin_cos();
        // Local frame: `a` along the bearing (outward), `b` across it.
        let point = |a: f32, b: f32| {
            let r = 132. + a * size;
            c.pt(cx + s * r + co * b * size, cy - co * r + s * b * size)
        };
        let (tip, left, notch, right) =
            (point(14., 0.), point(-2., 13.), point(6., 0.), point(-2., -13.));
        c.p.add(Shape::closed_line(
            vec![tip, left, notch, right],
            Stroke::new(4. * c.s, C::from_black_alpha((180. * pulse) as u8)),
        ));
        let amber = C::from_rgba_unmultiplied(255, 178, 52, (240. * pulse) as u8);
        for wing in [left, right] {
            c.p.add(Shape::convex_polygon(vec![tip, wing, notch], amber, Stroke::NONE));
        }
    }
}
/// HUD groups scale with the HUD size preference around their own anchors:
/// the screen corners, the top and bottom centres and the tip panel. The
/// reticle group, the hurt vignette and floating numbers' positions don't.
fn hud(base: &Canvas, g: &mut Game, vp: Mat4) {
    if cfg!(test) {
        base.ui
            .ctx()
            .data_mut(|d| d.insert_temp(Id::new(HUD_LAYOUT), Vec::<(&'static str, Rect)>::new()));
    }
    let h = base.h;
    let k = g.prefs.hud_scale;
    // Above 110% a compact layout keeps the larger groups apart: two power
    // columns, narrower top-centre panels with the boss inside the descent
    // panel, armor inside the vitality plate, and a narrower dodge readout.
    let compact = k > Preferences::COMPACT_ABOVE;
    let c = &base.anchored(k, 0., 0.);
    let xp = &g.run.survival;
    let power_count = xp.ranks.iter().filter(|r| **r > 0).count();
    let power_rows = if compact {
        power_count.div_ceil(2)
    } else {
        power_count
    };
    c.hud_panel(22., 18., 310., 154. + power_rows as f32 * 24.);
    c.diamond(43., 47., 6., GOLD);
    c.text(
        62.,
        47.,
        crate::world_layout::DISTRICTS[crate::world_layout::district_at(g.run.pos)].name,
        19.,
        IVORY,
        true,
        Align2::LEFT_CENTER,
    );
    c.line((40., 74.), (314., 74.), GOLD, 1.);
    c.text_role(
        40.,
        97.,
        format!("SOUL LEVEL {}", xp.level),
        18.,
        IVORY,
        TypeRole::Strong,
        Align2::LEFT_CENTER,
    );
    c.text(
        314.,
        97.,
        format!("{} / {}", xp.xp, xp.threshold()),
        16.,
        GOLD,
        false,
        Align2::RIGHT_CENTER,
    );
    c.fill(40., 118., 274., 6., C::from_rgb(53, 63, 58));
    c.fill(
        40.,
        118.,
        274. * (xp.xp as f32 / xp.threshold() as f32).clamp(0., 1.),
        6.,
        C::from_rgb(105, 225, 188),
    );
    c.text(
        40.,
        143.,
        "Collect souls. Gain powers.",
        16.,
        MUTED,
        false,
        Align2::LEFT_CENTER,
    );
    for (row, (i, rank)) in xp
        .ranks
        .iter()
        .enumerate()
        .filter(|(_, r)| **r > 0)
        .enumerate()
    {
        let (column, row) = if compact { (row % 2, row / 2) } else { (0, row) };
        let x = 40. + column as f32 * 141.;
        let y = 178. + row as f32 * 24.;
        c.text(
            x,
            y,
            crate::survival::POWERS[i].0,
            16.,
            IVORY,
            false,
            Align2::LEFT_CENTER,
        );
        c.text_role(
            if compact { x + 133. } else { 314. },
            y,
            format!("{rank}/5"),
            16.,
            GOLD,
            TypeRole::Strong,
            Align2::RIGHT_CENTER,
        );
    }
    let c = &base.anchored(k, 720., 0.);
    let boss = g.run.enemies.iter().find(|e| e.kind == 3 && e.hp > 0.);
    // Half the width of the descent panel and compass.
    let half = if compact { 210. } else { 227. };
    let boss_inside = compact && boss.is_some();
    c.hud_panel(720. - half, 18., half * 2., if boss_inside { 104. } else { 94. });
    c.text_role(
        720.,
        46.,
        if xp.endless {
            format!("ENDLESS / {:02}", g.run.wave)
        } else {
            format!("DESCENT {:02} / 12", g.run.wave)
        },
        24.,
        IVORY,
        TypeRole::Strong,
        Align2::CENTER_CENTER,
    );
    let hunting = format!(
        "{} HUNTING  /  {} APPROACHING",
        g.run.enemies.len(),
        xp.remaining
    );
    if let Some(e) = boss.filter(|_| boss_inside) {
        let (left, right) = (720. - half + 20., 720. + half - 20.);
        c.text(left, 78., "THE TITHEKEEPER", 16., GOLD, false, Align2::LEFT_CENTER);
        c.text(right, 78., hunting, 16., MUTED, false, Align2::RIGHT_CENTER);
        boss_bar(c, left, 98., right - left, e);
    } else {
        c.center(720., 84., hunting, 18., MUTED);
    }
    let c = &base.anchored(k, 1440., 0.);
    c.hud_panel(1188., 18., 200., if g.show_fps { 85. } else { 56. });
    c.coin(1212., 46., 10.);
    c.text_role(
        1370.,
        46.,
        format!("{}", g.run.gold),
        25.,
        IVORY,
        TypeRole::Strong,
        Align2::RIGHT_CENTER,
    );
    if g.show_fps {
        c.text(
            1370.,
            81.,
            format!(
                "{:.0} FPS / {}",
                g.fps,
                if g.vsync { "VSYNC" } else { "UNLOCKED" }
            ),
            16.,
            MUTED,
            false,
            Align2::RIGHT_CENTER,
        );
    }
    let c = &base.anchored(k, 720., 0.);
    let below = if boss_inside { 10. } else { 0. };
    world_compass(c, g, 120. + below, half);
    let threat_shown = last_threat(c, g, 198. + below);
    let status_y = if threat_shown { 243. } else { 200. };
    if let Some(e) = boss.filter(|_| !boss_inside) {
        c.hud_panel(480., status_y, 480., 65.);
        c.center(720., status_y + 21., "THE TITHEKEEPER", 18., GOLD);
        boss_bar(c, 494., status_y + 45., 452., e);
    }
    let c = base;
    // The damage arcs reach 108 units from the reticle.
    c.layout("reticle", 610., h * 0.5 - 110., 220., 220.);
    let cx = 720.;
    let cy = h * 0.5;
    let [r, gr, b] = g.prefs.reticle_color.rgb();
    reticle(c, cx, cy, g.prefs.reticle_size, g.flash * 38., C::from_rgb(r, gr, b));
    let c = &base.anchored(k, 0., h);
    // Keep the sculpted end caps away from the labels and digits.
    c.texture("button_plate", 16., h - 134., 395., 108., C::WHITE);
    // The plate's artwork, without the texture's transparent margins.
    c.layout("vitality", 16., h - 124., 395., 98.);
    c.inset(78., h - 106., 271., 55., C::from_rgb(43, 12, 16));
    c.text_role(
        87.,
        h - 96.,
        "VITALITY",
        16.,
        MUTED,
        TypeRole::Strong,
        Align2::LEFT_CENTER,
    );
    c.text_role(
        87.,
        h - 70.,
        format!("{} / {:.0}", g.run.hp.max(0.).ceil() as u32, g.max_hp()),
        28.,
        IVORY,
        TypeRole::Strong,
        Align2::LEFT_CENTER,
    );
    c.fill(86., h - 51., 255., 5., C::from_rgb(62, 24, 26));
    c.fill(
        86.,
        h - 51.,
        255. * (g.run.hp / g.max_hp()).clamp(0., 1.),
        5.,
        C::from_rgb(234, 84, 71),
    );
    if compact {
        // Armor shares the plate, right-aligned opposite vitality.
        c.text_role(340., h - 96., "ARMOR", 16., MUTED, TypeRole::Strong, Align2::RIGHT_CENTER);
        c.text_role(
            340.,
            h - 70.,
            format!("{:.0}", g.run.armor),
            28.,
            IVORY,
            TypeRole::Strong,
            Align2::RIGHT_CENTER,
        );
    } else {
        c.hud_panel(414., h - 111., 136., 75.);
        c.center(482., h - 92., "ARMOR", 16., MUTED);
        c.text_role(
            482.,
            h - 64.,
            format!("{:.0}", g.run.armor),
            28.,
            IVORY,
            TypeRole::Strong,
            Align2::CENTER_CENTER,
        );
    }
    // The vitality group's right edge: the plate, or the armor panel.
    let group_right = if compact { 411. } else { 550. };
    if g.run.chalice {
        c.hud_panel(24., h - 166., group_right - 24., 34.);
        c.text(
            40.,
            h - 149.,
            format!("{} / EMBER BOLT", g.prompt(Action::Bolt)),
            17.,
            GOLD,
            false,
            Align2::LEFT_CENTER,
        );
        c.text_role(
            group_right - 20.,
            h - 149.,
            format!("{:.0} MANA", g.run.mana),
            18.,
            IVORY,
            TypeRole::Strong,
            Align2::RIGHT_CENTER,
        );
    }
    // The dodge readout stays centred unless a larger HUD crowds it toward
    // the weapon panel; 16 units of the vitality group's gap are kept.
    let dodge_half = if compact { 100. } else { 119. };
    let dodge_x = 720_f32
        .max((group_right + 16. + dodge_half) * k)
        .min(1440. - (427. + 16. + dodge_half) * k);
    let c = &base.anchored(k, 720., h).shifted(dodge_x - 720., 0.);
    c.hud_panel(720. - dodge_half, h - 91., dodge_half * 2., 66.);
    let bar = dodge_half * 2. - 24.;
    c.fill(732. - dodge_half, h - 73., bar, 6., C::from_rgb(53, 63, 58));
    c.fill(
        732. - dodge_half,
        h - 73.,
        bar * (1. - g.dash_cd / 1.5).clamp(0., 1.),
        6.,
        C::from_rgb(105, 225, 188),
    );
    c.center(
        720.,
        h - 48.,
        if g.dash_cd > 0. {
            "DODGE RECOVERING".into()
        } else {
            format!("{} / DODGE", g.prompt(Action::Dodge))
        },
        16.,
        IVORY,
    );
    let c = &base.anchored(k, 1440., h);
    c.hud_panel(1013., h - 179., 375., 155.);
    c.paragraph_role(
        1030.,
        h - 165.,
        &g.run.weapon.name(),
        224.,
        18.,
        GOLD,
        false,
        TypeRole::Strong,
    );
    c.text_role(
        1030.,
        h - 86.,
        if g.run.weapon.kind.melee() {
            "MELEE".into()
        } else if g.reload > 0. {
            "RELOADING".into()
        } else {
            format!("{:02} / {:02}", g.run.ammo, g.run.weapon.capacity())
        },
        30.,
        IVORY,
        TypeRole::Strong,
        Align2::LEFT_CENTER,
    );
    c.text(
        1030.,
        h - 49.,
        if g.run.weapon.kind.melee() {
            format!("{} / {}  STRIKE", g.fire_prompt(), g.prompt(Action::Melee))
        } else {
            format!("{}  RELOAD", g.prompt(Action::Reload))
        },
        16.,
        MUTED,
        false,
        Align2::LEFT_CENTER,
    );
    card_base(c, 1270., h - 167., 106., 131., g.run.weapon.rarity, false);
    weapon_art(c, 1314., h - 97., 83., &g.run.weapon, 0.);
    for f in &g.floaters {
        let p = vp * f.pos.extend(1.);
        if p.w > 0. {
            let p = p.truncate() / p.w;
            let (x, y) = ((p.x * 0.5 + 0.5) * 1440., (0.5 - p.y * 0.5) * h);
            base.anchored(k, x, y).text_role(
                x,
                y,
                &f.text,
                23.,
                IVORY,
                TypeRole::Strong,
                Align2::CENTER_CENTER,
            );
        }
    }
    let c = base;
    if g.hurt > 0. {
        let strength = if g.prefs.reduce_flashes { 0.35 } else { 1. };
        let alpha = (g.hurt / 0.35 * 130. * strength) as u8;
        for i in 0..9 {
            let a = (alpha as f32 * (1. - i as f32 / 9.)) as u8;
            c.p.rect_stroke(
                c.rect(
                    i as f32 * 7.,
                    i as f32 * 7.,
                    1440. - i as f32 * 14.,
                    h - i as f32 * 14.,
                ),
                0.,
                Stroke::new(c.s * 8., C::from_rgba_unmultiplied(125, 10, 10, a)),
                egui::StrokeKind::Inside,
            );
        }
    }
    // Field tips and the opening reminder share one panel below the reticle,
    // clear of the crowd, the top-centre stack and the vitality plate.
    let c = &tip_canvas(base, k);
    if let Some(active) = g.tip.filter(|t| !t.tip.in_shop()) {
        let y = arena_tip_top(c, g).unwrap_or(c.h - TIP_FROM_BOTTOM);
        field_tip(c, &active.tip, &active.tip.text(g), y, ARENA_TIP_WIDTH, active.alpha());
    } else if g.run.time < 7. {
        let y = c.h - TIP_FROM_BOTTOM;
        let (fire, pause) = match g.device {
            Device::Keyboard if g.fire_prompt() == "LMB" => ("Mouse".into(), "Esc"),
            Device::Keyboard => (g.fire_prompt(), "Esc"),
            Device::Controller => (g.fire_prompt(), "Start"),
        };
        c.hud_panel(338., y + 20., 764., 35.);
        c.center(
            720.,
            y + 37.,
            format!(
                "{} Move  /  {fire} Fire  /  {} Reload  /  {} Melee  /  {pause} Pause",
                g.movement_prompt(),
                g.prompt(Action::Reload),
                g.prompt(Action::Melee)
            ),
            17.,
            IVORY,
        );
    }
}
/// Hit marks, damage arcs and off-screen warnings, drawn after field notes
/// and notices: a threat matters more than a tutorial, and at larger HUD
/// sizes an arc pointing behind reaches the notes.
fn combat_feedback(c: &Canvas, g: &Game) {
    hit_feedback(c, g, 720., c.h * 0.5);
    threat_pointers(c, g, 720., c.h * 0.5);
}
/// The Tithekeeper's health bar.
fn boss_bar(c: &Canvas, x: f32, y: f32, w: f32, e: &crate::game::Enemy) {
    c.fill(x, y, w, 7., C::from_rgb(54, 30, 28));
    c.fill(
        x,
        y,
        w * (e.hp / e.max_hp).clamp(0., 1.),
        7.,
        C::from_rgb(209, 65, 52),
    );
}
/// Distance from the bottom of the screen to the top of the arena tip panel.
const TIP_FROM_BOTTOM: f32 = 272.;
/// The weapon panel's top, plus a small gap: arena tips end above it.
const TIP_LOWEST: f32 = 187.;
const ARENA_TIP_WIDTH: f32 = 640.;
/// The arena tip panel and notices scale from the bottom centre, like the
/// panels beside them.
fn tip_canvas<'a>(base: &Canvas<'a>, k: f32) -> Canvas<'a> {
    base.anchored(k, 720., base.h)
}
/// Top of the arena tip panel, or of the opening reminder, if either shows.
/// Long notes rise so they end above the weapon and chalice panels.
fn arena_tip_top(c: &Canvas, g: &Game) -> Option<f32> {
    if let Some(active) = g.tip.filter(|t| !t.tip.in_shop()) {
        let height = tip_height(c, &active.tip.text(g), ARENA_TIP_WIDTH);
        Some((c.h - TIP_FROM_BOTTOM).min(c.h - TIP_LOWEST - height))
    } else {
        (g.run.time < 7.).then_some(c.h - TIP_FROM_BOTTOM)
    }
}
fn tip_layout(c: &Canvas, text: &str, width: f32) -> std::sync::Arc<egui::Galley> {
    c.p.layout_job({
        let mut job = egui::text::LayoutJob::simple(
            text.into(),
            TypeRole::Reading.font((18. * c.s).max(c.floor)),
            IVORY,
            (width - 48.) * c.s,
        );
        job.halign = egui::Align::Center;
        job
    })
}
fn tip_height(c: &Canvas, text: &str, width: f32) -> f32 {
    tip_layout(c, text, width).size().y / c.s + 46.
}
/// A field tip: a small heading over one to three centred lines.
fn field_tip(c: &Canvas, tip: &crate::tips::Tip, text: &str, y: f32, width: f32, alpha: f32) {
    let x = 720. - width / 2.;
    let height = tip_height(c, text, width);
    c.layout("tip", x, y, width, height);
    c.inset(x, y, width, height, HUD_SURFACE.gamma_multiply(alpha));
    c.border(x, y, width, height, GOLD.gamma_multiply(0.75 * alpha));
    c.diamond(720., y, 5., GOLD.gamma_multiply(alpha));
    c.center(720., y + 17., tip.heading(), 13., GOLD.gamma_multiply(alpha));
    // Centred paragraphs are anchored at their centre line.
    c.paragraph(
        720.,
        y + 32.,
        text,
        width - 48.,
        18.,
        IVORY.gamma_multiply(alpha),
        true,
    );
}
/// Where the pause screen's side panels sit, in design units: left and
/// right of the 512-wide folio, which spans 464 to 976.
const LEDGER_WIDTH: f32 = 384.;
const LEDGER_LEFT: f32 = 52.;
const LEDGER_RIGHT: f32 = 1004.;
/// The pause screen's side panels: the run so far, and the powers bound
/// with what each does at its rank.
fn pause_ledger(c: &Canvas, g: &Game) {
    let practice = g.practice_backup.is_some();
    let run = &g.run;
    let stats = &run.stats;
    let mut rows: Vec<(&str, String)> = vec![];
    if !practice {
        rows.extend([
            (
                "DESCENT",
                if run.survival.endless {
                    format!("ENDLESS / {:02}", run.wave)
                } else {
                    format!("{:02} / {}", run.wave, crate::survival::DESCENTS)
                },
            ),
            (
                "TIME",
                format!("{:02}:{:02}", run.time as u32 / 60, run.time as u32 % 60),
            ),
            (
                "VITALITY",
                format!(
                    "{} / {:.0}{}",
                    run.hp.max(0.).ceil() as u32,
                    g.max_hp(),
                    if run.armor > 0. {
                        format!("  +{:.0} ARMOR", run.armor)
                    } else {
                        String::new()
                    }
                ),
            ),
            ("SOULS", grouped(run.kills)),
            ("HEADSHOTS", grouped(stats.headshots)),
            ("DAMAGE DEALT", grouped(stats.damage_dealt.round() as u32)),
            ("DAMAGE TAKEN", grouped(stats.damage_taken.round() as u32)),
            ("GOLD", grouped(run.gold)),
            (
                "SOUL LEVEL",
                format!(
                    "{}  ({} / {})",
                    run.survival.level,
                    run.survival.xp,
                    run.survival.threshold()
                ),
            ),
        ]);
    }
    let card = &run.weapon;
    let paths = card.paths;
    let weapon_lines = [
        (rarity_line(card), GOLD),
        (
            format!(
                "{:.0} DAMAGE  /  EST. {:.0} DPS",
                card.damage(),
                card.estimated_dps()
            ),
            IVORY,
        ),
        (
            format!(
                "BINDING  {} / {} / {}{}",
                paths[0],
                paths[1],
                paths[2],
                if card.major { "  +  SOUL SIPHON" } else { "" }
            ),
            MUTED,
        ),
    ];
    let row = 34.;
    let height = 70. + rows.len() as f32 * row + if rows.is_empty() { 0. } else { 18. } + 160.;
    let x = LEDGER_LEFT;
    let y = 472. - height / 2.;
    c.hud_panel(x, y, LEDGER_WIDTH, height);
    let right = x + LEDGER_WIDTH - 22.;
    c.text(
        x + 22.,
        y + 32.,
        if practice { "PRACTICE GROUNDS" } else { "THE RUN SO FAR" },
        19.,
        GOLD,
        false,
        Align2::LEFT_CENTER,
    );
    c.line((x + 22., y + 56.), (right, y + 56.), GOLD.gamma_multiply(0.6), 1.);
    let mut ty = y + 56. + row / 2. + 6.;
    for (label, value) in &rows {
        c.text(x + 22., ty, *label, 15., MUTED, false, Align2::LEFT_CENTER);
        c.text_role(
            right,
            ty,
            value.clone(),
            18.,
            IVORY,
            TypeRole::Strong,
            Align2::RIGHT_CENTER,
        );
        ty += row;
    }
    if !rows.is_empty() {
        ty += 2.;
        c.line((x + 22., ty), (right, ty), GOLD.gamma_multiply(0.35), 0.8);
        ty += 16.;
    }
    c.text(x + 22., ty + 4., "WIELDING", 13., MUTED, false, Align2::LEFT_CENTER);
    let name = card.name();
    let fitted = c.fit_type(&name, TypeRole::Engraved, 21., LEDGER_WIDTH - 44.);
    c.text_role(
        x + 22.,
        ty + 36.,
        name,
        fitted,
        IVORY,
        TypeRole::Engraved,
        Align2::LEFT_CENTER,
    );
    for (i, (line, color)) in weapon_lines.into_iter().enumerate() {
        c.text(
            x + 22.,
            ty + 70. + i as f32 * 30.,
            line,
            15.,
            color,
            false,
            Align2::LEFT_CENTER,
        );
    }
    // The run's rows and the weapon's last line, for the layout test.
    c.layout("ledger_text", x + 22., y + 56., LEDGER_WIDTH - 44., ty + 145. - (y + 56.));
    if practice {
        return;
    }
    // Bound powers, each with what it does at its rank. The effect lines are
    // measured first, so the panel fits however they wrap.
    let held: Vec<(usize, u8)> = run
        .survival
        .ranks
        .iter()
        .enumerate()
        .filter(|(_, r)| **r > 0)
        .map(|(i, r)| (i, *r))
        .collect();
    let text_width = LEDGER_WIDTH - 44.;
    let effects: Vec<(String, f32)> = held
        .iter()
        .map(|&(p, rank)| {
            let text = crate::survival::power::effect(p, rank);
            let height = ledger_layout(c, &text, text_width).size().y / c.s;
            (text, height)
        })
        .collect();
    let empty = held.is_empty();
    let height = 70.
        + if empty {
            90.
        } else {
            effects.iter().map(|(_, h)| 30. + h + 10.).sum::<f32>()
        };
    let x = LEDGER_RIGHT;
    let y = 472. - height / 2.;
    let right = x + LEDGER_WIDTH - 22.;
    c.hud_panel(x, y, LEDGER_WIDTH, height);
    c.text(x + 22., y + 32., "BOUND POWERS", 19., GOLD, false, Align2::LEFT_CENTER);
    c.line((x + 22., y + 56.), (right, y + 56.), GOLD.gamma_multiply(0.6), 1.);
    if empty {
        c.paragraph(
            x + 22.,
            y + 72.,
            "None yet. Gather souls to reach the next soul level, then choose a power.",
            text_width,
            15.,
            MUTED,
            false,
        );
        return;
    }
    let mut ty = y + 66.;
    for (&(p, rank), (effect, h)) in held.iter().zip(effects) {
        c.text(
            x + 22.,
            ty + 13.,
            crate::survival::POWERS[p].0,
            17.,
            IVORY,
            false,
            Align2::LEFT_CENTER,
        );
        c.text_role(
            right,
            ty + 13.,
            format!("{rank}/5"),
            17.,
            GOLD,
            TypeRole::Strong,
            Align2::RIGHT_CENTER,
        );
        c.p.galley(
            c.pt(x + 22., ty + 30.),
            ledger_layout(c, &effect, text_width),
            MUTED,
        );
        c.layout("ledger_text", x + 22., ty, text_width, 30. + h);
        ty += 30. + h + 10.;
    }
}
fn ledger_layout(c: &Canvas, text: &str, width: f32) -> std::sync::Arc<egui::Galley> {
    c.p.layout_job(egui::text::LayoutJob::simple(
        text.into(),
        TypeRole::Reading.font((18. * c.s).max(c.floor)),
        MUTED,
        width * c.s,
    ))
}
fn pause(c: &Canvas, g: &mut Game) {
    c.fill(-100., 0., 1640., c.h, C::from_black_alpha(165));
    // The journal opens over the pause menu and is wider than it.
    if !g.settings {
        pause_ledger(c, g);
    }
    let x = 464.;
    let y = 215.;
    c.layout("pause_folio", x, y, 512., 515.);
    c.folio(x, y, 512., 515.);
    c.flourish(720., y + 140., 330., INK);
    c.text(
        720.,
        y + 67.,
        "A MOMENT IN THE DARK",
        28.,
        INK,
        true,
        Align2::CENTER_CENTER,
    );
    c.center(720., y + 113., "THE FOREST CAN WAIT.", 11., INK);
    if c.button(
        "resume",
        521.,
        y + 163.,
        398.,
        47.,
        "RETURN TO THE HUNT",
        true,
    ) {
        g.mode = Mode::Arena;
    }
    if c.button(
        "pause_settings",
        521.,
        y + 225.,
        398.,
        43.,
        "SETTINGS & CONTROLS",
        false,
    ) {
        g.settings = true;
    }
    if c.button(
        "save_title",
        521.,
        y + 284.,
        398.,
        43.,
        "SAVE & RETURN TO TITLE",
        false,
    ) {
        if g.save() {
            g.mode = Mode::Title;
        }
    }
    if c.button(
        "quit_pause",
        521.,
        y + 345.,
        398.,
        43.,
        if g.practice_backup.is_some() {
            "QUIT GAME"
        } else {
            "SAVE & QUIT GAME"
        },
        false,
    ) {
        g.quit_requested = true;
    }
    c.center(
        720.,
        y + 435.,
        if cfg!(target_os = "macos") {
            "ESC / RESUME   ·   CMD + Q / QUIT"
        } else {
            "ESC / RESUME"
        },
        16.,
        INK,
    );
}
fn level_up(c: &Canvas, g: &mut Game) {
    c.fill(-100., 0., 1640., c.h, C::from_black_alpha(210));
    c.seal(720., 145., 45., GOLD);
    c.center(720., 231., "THE SOUL AWAKENS", 42., IVORY);
    c.center(
        720.,
        279.,
        format!(
            "LEVEL {}  /  CHOOSE A PACT  /  COMBAT PAUSED",
            g.run.survival.level
        ),
        13.,
        GOLD,
    );
    let choices = g.run.survival.choices.clone();
    for (slot, p) in choices.into_iter().enumerate() {
        let x = 187. + slot as f32 * 366.;
        c.folio(x, 327., 334., 349.);
        c.seal(x + 167., 394., 31., GOLD);
        c.center(x + 167., 394., format!("{}", slot + 1), 23., INK);
        c.center(x + 167., 466., crate::survival::POWERS[p].0, 19., INK);
        c.paragraph(
            x + 167.,
            494.,
            crate::survival::POWERS[p].1,
            246.,
            17.,
            INK,
            true,
        );
        c.center(
            x + 167.,
            584.,
            format!("RANK {} / 5", g.run.survival.ranks[p] + 1),
            12.,
            INK,
        );
        if c.button(
            &format!("power{slot}"),
            x + 27.,
            616.,
            280.,
            42.,
            "BIND THIS POWER",
            true,
        ) {
            g.choose_power(slot);
            break;
        }
    }
    c.center(
        720.,
        718.,
        match g.device {
            Device::Keyboard => "PRESS 1, 2 OR 3 TO BIND A POWER",
            Device::Controller => "PRESS D-PAD LEFT, UP OR RIGHT TO BIND A POWER",
        },
        11.,
        MUTED,
    );
}
fn ending(c: &Canvas, g: &mut Game) {
    c.fill(-100., 0., 1640., c.h, C::from_black_alpha(195));
    let win = g.mode == Mode::Victory;
    c.seal(720., 160., 56., GOLD);
    c.text(
        720.,
        278.,
        if win {
            "THE DEBT IS PAID"
        } else {
            "THE TITHE TAKES ITS DUE"
        },
        52.,
        IVORY,
        true,
        Align2::CENTER_CENTER,
    );
    c.center(
        720.,
        334.,
        if win {
            "THE TITHEKEEPER FALLS. THE BELLS FALL SILENT."
        } else {
            "SAME BLOOD. DIFFERENT CARDS. ANOTHER CHANCE."
        },
        13.,
        GOLD,
    );
    // What ended the run, and what hurt most along the way.
    let stats = &g.run.stats;
    let cause = stats.last_hit.filter(|_| !win);
    if let Some(cause) = cause {
        c.center(
            720.,
            378.,
            format!("SLAIN BY {}", cause.describe()),
            17.,
            BLOOD,
        );
    }
    if let Some((kind, share)) = stats.top_source() {
        c.center(
            720.,
            if cause.is_some() { 406. } else { 388. },
            format!(
                "MOST DAMAGE TAKEN  /  {}  /  {:.0}%",
                crate::encounters::species(kind).name,
                share * 100.
            ),
            13.,
            MUTED,
        );
    }
    c.hud_panel(330., 428., 780., 140.);
    let cells = [
        ("DESCENT", format!("{:02}", g.run.wave)),
        ("SOULS", grouped(g.run.kills)),
        ("HEADSHOTS", grouped(stats.headshots)),
        (
            "TIME",
            format!(
                "{:02}:{:02}",
                g.run.time as u32 / 60,
                g.run.time as u32 % 60
            ),
        ),
        ("DAMAGE DEALT", grouped(stats.damage_dealt.round() as u32)),
        ("DAMAGE TAKEN", grouped(stats.damage_taken.round() as u32)),
        ("SOUL LEVEL", g.run.survival.level.to_string()),
        (
            "POWER RANKS",
            g.run.survival.ranks.iter().map(|&r| r as u32).sum::<u32>().to_string(),
        ),
    ];
    for (i, (label, value)) in cells.into_iter().enumerate() {
        let x = 427.5 + (i % 4) as f32 * 195.;
        let y = 450. + (i / 4) as f32 * 64.;
        c.center(x, y, label, 12., MUTED);
        c.text_role(
            x,
            y + 25.,
            value,
            24.,
            IVORY,
            TypeRole::Strong,
            Align2::CENTER_CENTER,
        );
    }
    c.center(
        720.,
        588.,
        format!("WIELDING  /  {}", g.run.weapon.name().to_uppercase()),
        13.,
        GOLD,
    );
    if !g.run_records.is_empty() {
        c.center(
            720.,
            614.,
            format!("NEW RECORD  /  {}", g.run_records.join("  /  ")),
            14.,
            GOLD,
        );
    }
    if c.button("again", 542., 640., 356., 54., "BUILD AGAIN", true) {
        g.new_run();
    }
    if c.button("end_title", 542., 706., 356., 43., "RETURN TO TITLE", false) {
        g.mode = Mode::Title;
    }
    if win
        && c.button(
            "endless",
            542.,
            761.,
            356.,
            43.,
            "THE TITHE NEVER ENDS",
            true,
        )
    {
        g.run.survival.endless = true;
        g.next_wave();
    }
}
/// 12345 -> "12,345".
fn grouped(n: u32) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, d) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(d);
    }
    out
}
/// The controller's virtual pointer, drawn above every menu and modal, with
/// a brass focus frame around the control it rests on.
pub fn pad_cursor(ctx: &egui::Context, pos: Pos2) {
    let p = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Tooltip,
        Id::new("pad_cursor"),
    ));
    let targets = pad_targets(ctx);
    if let Some(i) = crate::gamepad::focused(pos, &targets) {
        let r = targets[i].rect.expand(5.);
        p.rect_stroke(r, 5., Stroke::new(5., C::from_black_alpha(150)), egui::StrokeKind::Middle);
        p.rect_stroke(r, 5., Stroke::new(2., GOLD), egui::StrokeKind::Middle);
        // Corner marks keep the frame legible against gold-trimmed buttons.
        for (corner, dx, dy) in [
            (r.left_top(), 1., 1.),
            (r.right_top(), -1., 1.),
            (r.left_bottom(), 1., -1.),
            (r.right_bottom(), -1., -1.),
        ] {
            let stroke = Stroke::new(3., IVORY);
            p.line_segment([corner, corner + Vec2::new(dx * 12., 0.)], stroke);
            p.line_segment([corner, corner + Vec2::new(0., dy * 12.)], stroke);
        }
    }
    p.circle_stroke(pos, 11., Stroke::new(4.5, C::from_black_alpha(170)));
    p.circle_stroke(pos, 11., Stroke::new(2., GOLD));
    p.circle_filled(pos, 2.5, IVORY);
}
/// A screen, as far as transitions go: the mode, and whether the journal or
/// the new-run dialog is open over it.
type ScreenKey = (Mode, bool, bool);
/// How a new screen arrives: a fade from dark of this strength (0 to 1)
/// over this many seconds, or at once. Resuming the fight is never delayed.
fn screen_fade(from: ScreenKey, to: ScreenKey) -> Option<(f32, f32)> {
    if from == to {
        return None;
    }
    if from.0 == to.0 {
        // The journal or the dialog opening or closing over the same screen.
        return Some((0.35, 0.18));
    }
    match (from.0, to.0) {
        (Mode::Paused | Mode::LevelUp, Mode::Arena) => None,
        (Mode::Arena, Mode::Paused | Mode::LevelUp) => Some((0.4, 0.2)),
        (_, Mode::Dead | Mode::Victory) => Some((0.9, 0.7)),
        (_, Mode::Arena) => Some((0.85, 0.5)),
        _ => Some((0.6, 0.28)),
    }
}
/// The fade over the screen this frame, from the screen's last change.
fn fade_alpha(strength: f32, seconds: f32, since: f32) -> f32 {
    let t = (since / seconds).clamp(0., 1.);
    strength * (1. - t * t * (3. - 2. * t))
}
pub fn draw(ctx: &egui::Context, g: &mut Game, vp: Mat4) {
    // Screens fade in from dark when they change, timed by egui's clock,
    // which runs in every session and review.
    let now = ctx.input(|i| i.time) as f32;
    let key: ScreenKey = (g.mode, g.settings, g.confirm_new_run);
    let fade_id = Id::new("screen_fade");
    let fade = match ctx.data(|d| d.get_temp::<(ScreenKey, f32, f32, f32)>(fade_id)) {
        Some(state) if state.0 == key => state,
        previous => {
            let (strength, seconds) = previous
                .and_then(|(old, ..)| screen_fade(old, key))
                .unwrap_or((0., 1.));
            let state = (key, now, strength, seconds);
            ctx.data_mut(|d| d.insert_temp(fade_id, state));
            state
        }
    };
    let old = ctx.data(|d| d.get_temp::<(Mode, f32)>(Id::new("mode_transition")));
    if old.is_none_or(|(mode, _)| mode != g.mode) {
        ctx.data_mut(|d| d.insert_temp(Id::new("mode_transition"), (g.mode, g.elapsed)));
    }
    clear_pad_targets(ctx);
    if cfg!(test) {
        ctx.data_mut(|d| d.insert_temp(Id::new(HUD_LAYOUT), Vec::<(&'static str, Rect)>::new()));
    }
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show(ctx, |ui| {
            let c = Canvas::new(ui, g.elapsed);
            match g.mode {
                Mode::Title => title(&c, g),
                Mode::Arena => hud(&c, g, vp),
                Mode::LevelUp => level_up(&c, g),
                Mode::Shop => house(&c, g),
                Mode::Pack => pack(&c, g),
                Mode::Tree => tree(&c, g),
                Mode::Collection => collection(&c, g),
                Mode::Bestiary => bestiary(&c, g),
                // The pause ledger carries what the HUD shows, so the HUD
                // stays hidden behind it.
                Mode::Paused => pause(&c, g),
                Mode::Dead | Mode::Victory => ending(&c, g),
            }
            if g.notice_time > 0.
                && matches!(
                    g.mode,
                    Mode::Arena | Mode::Shop | Mode::Tree | Mode::Pack | Mode::Paused | Mode::Title
                )
            {
                // In the arena, notices sit above the tip panel and scale with it.
                let arena = tip_canvas(&c, g.prefs.hud_scale);
                let (c, y) = if g.mode != Mode::Arena {
                    (&c, 128.)
                } else if let Some(top) = arena_tip_top(&arena, g) {
                    (&arena, top - 40.)
                } else {
                    (&arena, c.h * 0.72)
                };
                let alpha = (g.notice_time.min(1.) * 245.) as u8;
                // Over the pause menu, keep between the ledger's panels.
                let (x, w) = if g.mode == Mode::Paused && !g.settings {
                    (LEDGER_LEFT + LEDGER_WIDTH + 4., LEDGER_RIGHT - LEDGER_LEFT - LEDGER_WIDTH - 8.)
                } else {
                    (310., 820.)
                };
                c.layout("notice", x, y - 17., w, 35.);
                c.fill(x, y - 17., w, 35., C::from_black_alpha(alpha.min(210)));
                c.center(
                    720.,
                    y,
                    &g.notice,
                    11.,
                    GOLD.gamma_multiply(g.notice_time.min(1.)),
                );
            }
            if g.mode == Mode::Arena {
                combat_feedback(&c, g);
            }
        });
    if g.confirm_new_run || g.settings {
        egui::Modal::new(Id::new("field_notes_modal"))
            .frame(egui::Frame::NONE)
            .area(
                egui::Modal::default_area(Id::new("field_notes_modal"))
                    .anchor(Align2::LEFT_TOP, Vec2::ZERO),
            )
            .backdrop_color(C::from_black_alpha(180))
            .show(ctx, |ui| {
                clear_pad_targets(ctx);
                let size = ctx.screen_rect().size();
                ui.set_min_size(size);
                ui.set_max_size(size);
                let c = Canvas::new(ui, g.elapsed);
                if g.confirm_new_run {
                    c.folio(390., 235., 660., 400.);
                    c.seal(720., 300., 26., INK);
                    c.text(
                        720.,
                        362.,
                        "Begin a new descent?",
                        30.,
                        INK,
                        true,
                        Align2::CENTER_CENTER,
                    );
                    c.center(720., 410., "Your saved run will be replaced.", 15., INK);
                    c.flourish(720., 452., 420., GOLD);
                    if c.button("keep_run", 440., 510., 260., 52., "Keep my pact", false) {
                        g.confirm_new_run = false;
                    }
                    if c.button(
                        "replace_run",
                        740.,
                        510.,
                        260.,
                        52.,
                        "Answer the bell",
                        true,
                    ) {
                        g.confirm_new_run = false;
                        g.new_run();
                    }
                } else {
                    journal(&c, g);
                }
            });
    }
    let alpha = fade_alpha(fade.2, fade.3, now - fade.1);
    if alpha > 0.003 {
        ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, fade_id))
            .rect_filled(ctx.screen_rect(), 0., C::from_black_alpha((alpha * 255.) as u8));
    }
}
/// The Hunter's Journal: a Preferences page and a Controls page.
fn journal(c: &Canvas, g: &mut Game) {
    c.folio(325., 88., 790., 724.);
    c.text(
        720.,
        150.,
        "The Hunter's Journal",
        38.,
        INK,
        true,
        Align2::CENTER_CENTER,
    );
    c.center(720., 190., "PREFERENCES  &  FIELD NOTES", 11., INK);
    c.flourish(720., 213., 530., INK);
    for (id, x, label, page) in JOURNAL_TABS {
        let open = g.journal_page == page;
        if c.button(id, x, 236., JOURNAL_TAB_WIDTH, 38., label, open) {
            g.journal_page = page;
            g.rebinding = None;
            g.pad_rebinding = None;
            g.controls_note.clear();
        }
        if open {
            let mid = x + JOURNAL_TAB_WIDTH * 0.5;
            c.line((mid - 62., 284.), (mid + 62., 284.), INK, 2.);
            c.diamond(mid, 284., 5., INK);
        }
    }
    match g.journal_page {
        JournalPage::Preferences => journal_preferences(c, g),
        JournalPage::Display => journal_display(c, g),
        JournalPage::Keyboard => journal_controls(c, g),
        JournalPage::Controller => journal_controller(c, g),
    }
    if c.button(
        "notes_back",
        558.,
        750.,
        324.,
        38.,
        "Close the journal",
        true,
    ) {
        g.rebinding = None;
        g.pad_rebinding = None;
        g.save_preferences();
        g.settings = false;
    }
}
fn journal_preferences(c: &Canvas, g: &mut Game) {
    let (min_fov, max_fov) = Preferences::FOV_RANGE;
    let (min_sens, max_sens) = Preferences::SENSITIVITY_RANGE;
    let prefs = &mut g.prefs;
    for (row, label, id, value, min, max, readout) in [
        (
            0,
            "Aim sensitivity",
            "sensitivity",
            &mut prefs.sensitivity,
            min_sens,
            max_sens,
            Some("%"),
        ),
        (
            1,
            "Sound volume",
            "volume",
            &mut prefs.volume,
            0.,
            1.,
            Some("%"),
        ),
        (
            2,
            "Music volume",
            "music_volume",
            &mut prefs.music_volume,
            0.,
            1.,
            Some("%"),
        ),
        (
            3,
            "Field of view",
            "fov",
            &mut prefs.fov,
            min_fov,
            max_fov,
            Some("°"),
        ),
    ] {
        let y = JOURNAL_SLIDER_Y + row as f32 * JOURNAL_SLIDER_STEP;
        c.text(392., y, label, 17., INK, true, Align2::LEFT_CENTER);
        c.slider(id, 615., y + 1., 360., value, min, max);
        if let Some(unit) = readout {
            // Sensitivity reads as a share of the default.
            let shown = match (id, unit) {
                ("sensitivity", _) => *value / Preferences::default().sensitivity * 100.,
                (_, "%") => *value * 100.,
                _ => *value,
            };
            c.center(1010., y + 1., format!("{}{unit}", shown.round()), 12., INK);
        }
    }
    for (id, x, y, label, on) in [
        ("invert_y", 392., JOURNAL_TOGGLES_Y, "INVERT LOOK", g.prefs.invert_y),
        (
            "reduce_flashes",
            734.,
            JOURNAL_TOGGLES_Y,
            "REDUCE FLASHES",
            g.prefs.reduce_flashes,
        ),
    ] {
        let text = format!("{label} {}", if on { "ON" } else { "OFF" });
        if c.button(id, x, y, 310., 35., &text, false) {
            match id {
                "invert_y" => g.prefs.invert_y = !on,
                _ => g.prefs.reduce_flashes = !on,
            }
        }
    }
    let tips = g.prefs.field_tips;
    if c.button(
        "field_tips",
        392.,
        JOURNAL_TOGGLES_Y + JOURNAL_TOGGLE_STEP,
        310.,
        35.,
        if tips {
            "FIELD TIPS ON"
        } else {
            "FIELD TIPS OFF"
        },
        false,
    ) {
        g.set_field_tips(!tips);
    }
    let toggle = g.prefs.toggle_sprint;
    if c.button(
        "toggle_sprint",
        734.,
        JOURNAL_TOGGLES_Y + JOURNAL_TOGGLE_STEP,
        310.,
        35.,
        if toggle {
            "SPRINT / TOGGLE"
        } else {
            "SPRINT / HOLD"
        },
        false,
    ) {
        g.prefs.toggle_sprint = !toggle;
    }
    let y = JOURNAL_TOGGLES_Y + 2. * JOURNAL_TOGGLE_STEP;
    let size = format!("RETICLE SIZE {:.0}%", g.prefs.reticle_size * 100.);
    if c.button("reticle_size", 392., y, 310., 35., &size, false) {
        g.prefs.reticle_size = g.prefs.next_reticle_size();
    }
    let color = format!("RETICLE / {}", g.prefs.reticle_color.name());
    if c.button("reticle_color", 734., y, 310., 35., &color, false) {
        g.prefs.reticle_color = g.prefs.reticle_color.next();
    }
}
/// The Display page: graphics fidelity beside the frame limit, the
/// Hollowlight treatment, and how the window and HUD are presented.
fn journal_display(c: &Canvas, g: &mut Game) {
    use crate::fidelity::Fidelity;
    let y = DISPLAY_FIDELITY_Y;
    let last = (Fidelity::ALL.len() - 1) as f32;
    let mut stop = g.prefs.fidelity.index() as f32;
    c.text(392., y, "Graphics fidelity", 17., INK, true, Align2::LEFT_CENTER);
    c.stepped_slider("fidelity", 615., y + 1., 360., &mut stop, 0., last, 1. / last);
    g.prefs.fidelity = Fidelity::ALL[stop.round() as usize];
    // Each stop is named under the track; the chosen one is inked and
    // underlined, so the slider needs no readout of its own.
    for (i, step) in Fidelity::ALL.into_iter().enumerate() {
        let x = 615. + 360. * i as f32 / last;
        if step == g.prefs.fidelity {
            c.center(x, y + 35., step.name(), 12., INK);
            c.line((x - 22., y + 46.), (x + 22., y + 46.), INK, 1.2);
        } else {
            c.center(x, y + 35., step.name(), 11., INK.gamma_multiply(0.72));
        }
    }
    c.center(720., y + 68., g.prefs.fidelity.summary(), 12., INK);
    let y = DISPLAY_HOLLOWLIGHT_Y;
    c.text(392., y, "Hollowlight / F6", 17., INK, true, Align2::LEFT_CENTER);
    c.slider("hollowlight_effects", 615., y + 1., 360., &mut g.shader_intensity, 0., 1.);
    c.center(
        1010.,
        y + 1.,
        format!("{}%", (g.shader_intensity * 100.).round()),
        12.,
        INK,
    );
    // The frame limit steps through its listed rates.
    let limits = crate::pacing::FRAME_LIMITS;
    let y = DISPLAY_FRAME_LIMIT_Y;
    let mut stop = limits
        .iter()
        .position(|&l| l == g.prefs.frame_limit)
        .unwrap_or(0) as f32;
    c.text(392., y, "Frame limit", 17., INK, true, Align2::LEFT_CENTER);
    let last = (limits.len() - 1) as f32;
    c.stepped_slider("frame_limit", 615., y + 1., 360., &mut stop, 0., last, 1. / last);
    g.prefs.frame_limit = limits[stop.round() as usize];
    c.center(
        1010.,
        y + 1.,
        match g.prefs.frame_limit {
            0 => "OFF".to_string(),
            fps => format!("{fps} FPS"),
        },
        12.,
        INK,
    );
    if c.button(
        "vsync",
        392.,
        DISPLAY_TOGGLES_Y,
        310.,
        35.,
        if g.vsync {
            "F7 / VSYNC ON"
        } else {
            "F7 / UNLOCKED FPS"
        },
        false,
    ) {
        g.vsync = !g.vsync;
        g.save_performance();
    }
    if c.button(
        "fpscounter",
        734.,
        DISPLAY_TOGGLES_Y,
        310.,
        35.,
        if g.show_fps {
            "F8 / FPS COUNTER ON"
        } else {
            "F8 / SHOW FPS"
        },
        false,
    ) {
        g.show_fps = !g.show_fps;
        g.save_performance();
    }
    let fullscreen = g.prefs.fullscreen;
    if c.button(
        "fullscreen",
        392.,
        DISPLAY_TOGGLES_Y + JOURNAL_TOGGLE_STEP,
        310.,
        35.,
        if fullscreen {
            "F11 / FULLSCREEN ON"
        } else {
            "F11 / FULLSCREEN OFF"
        },
        false,
    ) {
        g.prefs.fullscreen = !fullscreen;
        g.fullscreen_changed = true;
    }
    if c.button(
        "hud_scale",
        734.,
        DISPLAY_TOGGLES_Y + JOURNAL_TOGGLE_STEP,
        310.,
        35.,
        &format!("HUD SIZE {:.0}%", g.prefs.hud_scale * 100.),
        false,
    ) {
        g.prefs.hud_scale = g.prefs.next_hud_scale();
    }
}
fn journal_controls(c: &Canvas, g: &mut Game) {
    let mut clicked_key = false;
    for (i, action) in Action::ALL.into_iter().enumerate() {
        let (column, row) = if i < 5 { (0., i) } else { (1., i - 5) };
        let x = 372. + column * 372.;
        let y = 304. + row as f32 * 46.;
        c.text(x, y + 18., action.name(), 17., INK, true, Align2::LEFT_CENTER);
        let waiting = g.rebinding == Some(action);
        let label = if waiting {
            "PRESS A KEY".into()
        } else {
            g.prefs.bindings.label(action)
        };
        if waiting {
            let pulse = 0.55 + 0.45 * (c.time * 5.).sin().abs();
            c.border(x + 122., y - 6., 208., 48., GOLD.gamma_multiply(pulse));
        }
        if c.button(
            &format!("bind_{}", action.name()),
            x + 128.,
            y,
            196.,
            36.,
            &label,
            waiting,
        ) {
            clicked_key = true;
            if waiting {
                // A left click elsewhere is how the journal is used, so the
                // left button is bound by clicking the waiting button again.
                g.bind(
                    crate::controls::Trigger::Mouse(winit::event::MouseButton::Left),
                    None,
                );
            } else {
                g.rebinding = Some(action);
                g.controls_note.clear();
            }
        }
    }
    // Clicking anywhere else stops waiting for a key.
    if g.rebinding.is_some() && !clicked_key && c.ui.input(|i| i.pointer.any_click()) {
        g.rebinding = None;
    }
    for (i, line) in [
        "1 2 3  choose a power    Esc  pause    F11  fullscreen",
        "F6  Hollowlight    F7  VSync    F8  FPS counter",
    ]
    .into_iter()
    .enumerate()
    {
        let y = 552. + i as f32 * 28.;
        c.center(720., y, line, 13., INK);
        c.line((390., y + 14.), (1050., y + 14.), GOLD.gamma_multiply(0.35), 0.6);
    }
    let note = if let Some(action) = g.rebinding {
        format!(
            "Press a key or mouse button for {}, or click it again for the left button. \
             Escape cancels.",
            action.name()
        )
    } else if g.controls_note.is_empty() {
        "Choose an action's key to change it. A key already in use swaps.".into()
    } else {
        g.controls_note.clone()
    };
    c.center(720., 652., note, 13., INK);
    if c.button(
        "restore_keys",
        558.,
        686.,
        324.,
        36.,
        "Restore default keys",
        false,
    ) {
        g.prefs.bindings = Default::default();
        g.prefs.bindings.learn_layout(&g.layout);
        g.rebinding = None;
        g.controls_note = "Default keys restored.".into();
        g.save_preferences();
    }
}
/// The Controller page: a main and a second button for each action.
fn journal_controller(c: &Canvas, g: &mut Game) {
    let mut clicked_slot = false;
    c.center(630., 300., "MAIN", 11., INK);
    c.center(850., 300., "SECOND", 11., INK);
    for (row, action) in PadAction::ALL.into_iter().enumerate() {
        let y = 318. + row as f32 * 42.;
        c.text(392., y + 17., action.name(), 17., INK, true, Align2::LEFT_CENTER);
        for slot in 0..2 {
            let x = 532. + slot as f32 * 220.;
            let waiting = g.pad_rebinding == Some((action, slot));
            let label = if waiting {
                "PRESS A BUTTON"
            } else {
                g.prefs.pad_bindings.slot(action, slot).map_or("—", Button::label)
            };
            if waiting {
                let pulse = 0.55 + 0.45 * (c.time * 5.).sin().abs();
                c.border(x - 6., y - 5., 208., 44., GOLD.gamma_multiply(pulse));
            }
            if c.button(
                &format!("pad_{}_{slot}", action.name()),
                x,
                y,
                196.,
                34.,
                label,
                waiting,
            ) {
                clicked_slot = true;
                g.pad_rebinding = (!waiting).then_some((action, slot));
                g.controls_note.clear();
            }
        }
    }
    // Clicking anywhere else stops waiting for a button.
    if g.pad_rebinding.is_some() && !clicked_slot && c.ui.input(|i| i.pointer.any_click()) {
        g.pad_rebinding = None;
    }
    // The right stick's turn rate, separate from the mouse's sensitivity.
    let (min_speed, max_speed) = Preferences::STICK_SPEED_RANGE;
    c.text(392., 584., "Stick look speed", 17., INK, true, Align2::LEFT_CENTER);
    c.slider("stick_speed", 615., 585., 360., &mut g.prefs.stick_speed, min_speed, max_speed);
    c.center(
        1010.,
        585.,
        format!("{:.0}%", g.prefs.stick_speed * 100.),
        12.,
        INK,
    );
    for (i, line) in [
        "Left stick  move    Right stick  look    Start  pause    D-pad  choose a power",
        "In menus, the left stick moves the cursor, A selects and B goes back.",
    ]
    .into_iter()
    .enumerate()
    {
        let y = 616. + i as f32 * 25.;
        c.center(720., y, line, 13., INK);
        c.line((390., y + 12.5), (1050., y + 12.5), GOLD.gamma_multiply(0.35), 0.6);
    }
    let note = if let Some((action, _)) = g.pad_rebinding {
        format!(
            "Press a controller button for {}. Start or Escape cancels.",
            action.name()
        )
    } else if g.controls_note.is_empty() {
        "Choose a slot to change it. A button already in use swaps.".into()
    } else {
        g.controls_note.clone()
    };
    c.center(720., 670., note, 13., INK);
    let assist = g.prefs.aim_assist;
    if c.button(
        "aim_assist",
        392.,
        698.,
        310.,
        36.,
        if assist {
            "AIM ASSIST ON"
        } else {
            "AIM ASSIST OFF"
        },
        false,
    ) {
        g.prefs.aim_assist = !assist;
    }
    if c.button(
        "restore_buttons",
        734.,
        698.,
        310.,
        36.,
        "Restore default buttons",
        false,
    ) {
        g.prefs.pad_bindings = Default::default();
        g.pad_rebinding = None;
        g.controls_note = "Default buttons restored.".into();
        g.save_preferences();
    }
}
/// Top row of the Preferences page sliders, `JOURNAL_SLIDER_STEP` apart.
pub const JOURNAL_SLIDER_Y: f32 = 312.;
pub const JOURNAL_SLIDER_STEP: f32 = 40.;
/// Top of the Preferences page switches, `JOURNAL_TOGGLE_STEP` apart:
/// invert and flashes, field tips and sprint, then the reticle.
pub const JOURNAL_TOGGLES_Y: f32 = 492.;
pub const JOURNAL_TOGGLE_STEP: f32 = 41.;
/// Top of the Display page switches: VSync and FPS, then fullscreen and
/// HUD size.
pub const DISPLAY_TOGGLES_Y: f32 = 512.;
/// The Display page's sliders: Graphics fidelity (with its stop names and
/// summary below it), Hollowlight and the frame limit.
pub const DISPLAY_FIDELITY_Y: f32 = 312.;
pub const DISPLAY_HOLLOWLIGHT_Y: f32 = 412.;
pub const DISPLAY_FRAME_LIMIT_Y: f32 = 452.;
/// The journal's page tabs, left to right.
pub const JOURNAL_TAB_WIDTH: f32 = 168.;
pub const JOURNAL_TABS: [(&str, f32, &str, JournalPage); 4] = [
    ("journal_preferences", 372., "PREFERENCES", JournalPage::Preferences),
    ("journal_display", 548., "DISPLAY", JournalPage::Display),
    ("journal_controls", 724., "KEYBOARD", JournalPage::Keyboard),
    ("journal_controller", 900., "CONTROLLER", JournalPage::Controller),
];

#[cfg(test)]
mod tests {
    use super::*;
    /// Run `f` with a canvas filling a window of the given logical size.
    fn with_canvas(width: f32, height: f32, f: impl FnOnce(&Canvas)) {
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(width, height))),
            ..Default::default()
        };
        let mut f = Some(f);
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ctx, |ui| (f.take().unwrap())(&Canvas::new(ui, 0.)));
        });
    }
    #[test]
    fn screens_fade_in_but_the_fight_resumes_at_once() {
        let arena = (Mode::Arena, false, false);
        let paused = (Mode::Paused, false, false);
        assert_eq!(screen_fade(arena, arena), None);
        assert_eq!(screen_fade(paused, arena), None, "resume at once");
        assert_eq!(screen_fade((Mode::LevelUp, false, false), arena), None);
        assert!(screen_fade(arena, paused).is_some());
        assert!(screen_fade((Mode::Shop, false, false), arena).is_some(), "a descent begins");
        assert!(screen_fade((Mode::Title, false, false), (Mode::Title, true, false)).is_some());
        assert!(screen_fade((Mode::Shop, false, false), (Mode::Pack, false, false)).is_some());
        for (from, to) in [(paused, (Mode::Shop, false, false)), (arena, (Mode::Dead, false, false))] {
            let (strength, seconds) = screen_fade(from, to).unwrap();
            assert!(strength > 0. && strength <= 1. && seconds <= 0.75);
            assert_eq!(fade_alpha(strength, seconds, 0.), strength);
            assert!(fade_alpha(strength, seconds, seconds * 0.5) < strength);
            assert_eq!(fade_alpha(strength, seconds, seconds), 0.);
        }
    }
    #[test]
    fn buttons_tick_once_when_reached_and_clack_once_when_pressed() {
        let ctx = egui::Context::default();
        configure(&ctx);
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1440., 900.));
        let mut frame = |events: Vec<egui::Event>| {
            let input = egui::RawInput {
                screen_rect: Some(screen),
                events,
                ..Default::default()
            };
            let mut clicked = false;
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(ctx, |ui| {
                        let c = Canvas::new(ui, 0.);
                        clicked = c.button("cue_test", 600., 400., 240., 44., "TEST", false);
                    });
            });
            (take_cues(&ctx), clicked)
        };
        let on = Pos2::new(720., 422.);
        let off = Pos2::new(100., 100.);
        let button = |pressed| egui::Event::PointerButton {
            pos: on,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        assert_eq!(frame(vec![egui::Event::PointerMoved(off)]).0, Vec::<&str>::new());
        assert_eq!(frame(vec![egui::Event::PointerMoved(on)]).0, ["ui_hover"]);
        // Resting on it stays quiet.
        for _ in 0..3 {
            assert_eq!(frame(vec![]).0, Vec::<&str>::new());
        }
        assert_eq!(frame(vec![button(true)]).0, Vec::<&str>::new());
        assert_eq!(frame(vec![button(false)]), (vec!["ui_press"], true));
        assert_eq!(frame(vec![]).0, Vec::<&str>::new());
        // Leaving and coming back ticks again.
        assert_eq!(frame(vec![egui::Event::PointerMoved(off)]).0, Vec::<&str>::new());
        assert_eq!(frame(vec![egui::Event::PointerMoved(on)]).0, ["ui_hover"]);
    }
    /// Draw `f` on a 1440×900 canvas filled with `background` and rasterize
    /// egui's own triangles into RGB pixels.
    fn rasterize(background: C, f: impl FnOnce(&Canvas)) -> (usize, Vec<[f32; 3]>) {
        let (w, h) = (1440usize, 900usize);
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(w as f32, h as f32))),
            ..Default::default()
        };
        let mut f = Some(f);
        let out = ctx.run(input, |ctx| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ctx, |ui| {
                    let c = Canvas::new(ui, 0.);
                    c.fill(0., 0., 1440., 900., background);
                    (f.take().unwrap())(&c)
                });
        });
        let mut pixels = vec![[0f32; 3]; w * h];
        for primitive in ctx.tessellate(out.shapes, out.pixels_per_point) {
            let egui::epaint::Primitive::Mesh(mesh) = primitive.primitive else {
                continue;
            };
            for t in mesh.indices.chunks(3) {
                let v = [0, 1, 2].map(|i| mesh.vertices[t[i] as usize]);
                let area = |a: Pos2, b: Pos2, p: Pos2| (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x);
                let total = area(v[0].pos, v[1].pos, v[2].pos);
                if total.abs() < 1e-6 {
                    continue;
                }
                let xs = v.map(|v| v.pos.x);
                let ys = v.map(|v| v.pos.y);
                let x0 = xs.iter().cloned().fold(f32::MAX, f32::min).floor().max(0.) as usize;
                let x1 = (xs.iter().cloned().fold(f32::MIN, f32::max).ceil() as usize).min(w - 1);
                let y0 = ys.iter().cloned().fold(f32::MAX, f32::min).floor().max(0.) as usize;
                let y1 = (ys.iter().cloned().fold(f32::MIN, f32::max).ceil() as usize).min(h - 1);
                for y in y0..=y1 {
                    for x in x0..=x1 {
                        let p = Pos2::new(x as f32 + 0.5, y as f32 + 0.5);
                        let l = [
                            area(v[1].pos, v[2].pos, p) / total,
                            area(v[2].pos, v[0].pos, p) / total,
                            area(v[0].pos, v[1].pos, p) / total,
                        ];
                        if l.iter().any(|l| *l < 0.) {
                            continue;
                        }
                        // Premultiplied colour, blended over what's there.
                        let mix = |k: usize| -> f32 {
                            (0..3).map(|i| l[i] * v[i].color.to_array()[k] as f32 / 255.).sum()
                        };
                        let alpha = mix(3);
                        let px = &mut pixels[y * w + x];
                        for k in 0..3 {
                            px[k] = mix(k) + px[k] * (1. - alpha);
                        }
                    }
                }
            }
        }
        (w, pixels)
    }
    #[test]
    fn the_reticle_is_outlined_so_it_reads_on_a_pale_surface() {
        let luminance = |p: [f32; 3]| 0.2126 * p[0] + 0.7152 * p[1] + 0.0722 * p[2];
        // Across a stroke from outside: background, dark outline, the
        // stroke at full colour, dark outline, background.
        let crosses = |line: Vec<[f32; 3]>, target: [f32; 3]| -> Result<(), String> {
            let dark = |p: [f32; 3]| luminance(p) < 0.4;
            let mut i = 0;
            let mut skip = |want: &dyn Fn([f32; 3]) -> bool, what: &str| {
                let from = i;
                while i < line.len() && want(line[i]) {
                    i += 1;
                }
                if i == from {
                    return Err(format!("no {what} at {i}: {line:?}"));
                }
                Ok(from..i)
            };
            skip(&|p| !dark(p), "background")?;
            skip(&dark, "outline")?;
            let stroke = skip(&|p| !dark(p), "stroke")?;
            skip(&dark, "outline below")?;
            skip(&|p| !dark(p), "background below")?;
            let full = stroke
                .map(|k| line[k])
                .any(|p| (0..3).all(|k| (p[k] - target[k]).abs() < 0.12));
            if !full {
                return Err(format!("the stroke never reaches {target:?}: {line:?}"));
            }
            Ok(())
        };
        // Ivory on ivory (bone, fog, a lit wall) is the hardest case.
        for size in Preferences::RETICLE_SIZES {
            for color in crate::game::ReticleColor::ALL {
                let [r, g, b] = color.rgb();
                let target = [r, g, b].map(|v| v as f32 / 255.);
                let (w, pixels) = rasterize(IVORY, |c| {
                    reticle(c, 720., 450., size, 0., C::from_rgb(r, g, b))
                });
                let column = |x: usize| (436..464).map(|y| pixels[y * w + x]).collect();
                let row = |y: usize| (706..734).map(|x| pixels[y * w + x]).collect();
                // The dot, along a diagonal that misses the ticks.
                let diagonal = (-9..9).map(|t: i32| {
                    pixels[(450 + t) as usize * w + (720 + t) as usize]
                });
                // The right-hand tick, the lower tick and the dot.
                let along = (5. * size + 3. * size).round() as usize;
                for (what, line) in [
                    ("right tick", column(720 + along)),
                    ("lower tick", row(450 + along)),
                    ("dot", diagonal.collect()),
                ] {
                    if let Err(e) = crosses(line, target) {
                        panic!("{size} {color:?} {what}: {e}");
                    }
                }
            }
        }
    }
    #[test]
    fn every_cards_trait_line_and_header_fit_a_pack_card() {
        for (width, height) in [(1440., 900.), (960., 600.)] {
            let ctx = egui::Context::default();
            configure(&ctx);
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(width, height))),
                ..Default::default()
            };
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(ctx, |ui| {
                        let c = Canvas::new(ui, 0.);
                        let long = "LEGENDARY  /  120° SWEEP + 3.2m SPLASH + VENOM";
                        assert!(c.fit_type(long, TypeRole::Reading, 18., 240.) < 17.);
                        for kind in crate::weapons::WeaponKind::ALL {
                            let card = Card {
                                kind,
                                rarity: 3,
                                ..Card::starter()
                            };
                            // A 280-wide pack card, as drawn by `weapon_card`.
                            let traits = trait_line(&card);
                            let fitted = c.fit_type(&traits, TypeRole::Reading, 18., 240.);
                            assert!(
                                fitted >= 17.,
                                "{traits} shrinks to {fitted:.1} at {width}x{height}"
                            );
                            let header = format!(
                                "{}  /  {}",
                                rarity_name(&card),
                                crate::weapons::GROUPS[kind.spec().group].to_uppercase()
                            );
                            let wide = c.text_width(&header, 10.);
                            assert!(wide <= 236., "{header} is {wide:.0} wide at {width}x{height}");
                        }
                    });
            });
        }
    }
    /// The busiest HUD, as in the `hud-size-*-busy` review fixtures: every
    /// power at full rank, the boss under the last-threat bearing, a bound
    /// chalice, the longest arena note under a notice, a damage arc straight
    /// ahead and a dive winding up behind.
    fn busiest_hud(k: f32) -> Game {
        use crate::game::{DamageMark, Enemy};
        let mut g = Game::new(false);
        g.mode = Mode::Arena;
        g.prefs.hud_scale = k;
        g.show_fps = true;
        g.run.wave = 12;
        g.run.time = 30.;
        g.run.gold = 98765;
        g.run.survival.remaining = 0;
        g.run.survival.level = 99;
        g.run.survival.ranks = [5; 10];
        g.run.hp = 200.;
        g.run.armor = 999.;
        g.run.chalice = true;
        g.run.mana = 120.;
        g.dash_cd = 0.5;
        g.notice = "HEADSHOT / THE SKULL BREAKS FREE".into();
        g.notice_time = 2.;
        let player = g.run.pos;
        let mut diver = Enemy::spawn(4, player + glam::Vec3::new(-2., -player.y, 5.), 12, 0.);
        diver.ai.warning = crate::encounters::warning_time(4) * 0.3;
        diver.ai.target = glam::Vec3::new(player.x, 0., player.z);
        g.run.enemies = vec![
            Enemy::spawn(3, player + glam::Vec3::new(0., -player.y, -6.), 12, 0.),
            diver,
        ];
        g.tip = Some(crate::tips::ActiveTip {
            tip: crate::tips::Tip::Souls,
            life: 5.,
        });
        g.damage_marks = vec![DamageMark {
            bearing: g.run.yaw,
            life: DAMAGE_MARK_LIFE,
        }];
        g
    }
    #[test]
    fn busiest_hud_panels_never_overlap_at_any_size() {
        for (width, height) in [(1440., 900.), (960., 600.), (1920., 1080.)] {
            for k in Preferences::HUD_SCALES {
                let ctx = egui::Context::default();
                configure(&ctx);
                let mut g = busiest_hud(k);
                for _ in 0..2 {
                    let input = egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(
                            Pos2::ZERO,
                            Vec2::new(width, height),
                        )),
                        ..Default::default()
                    };
                    let _ = ctx.run(input, |ctx| draw(ctx, &mut g, Mat4::IDENTITY));
                }
                let layout = ctx
                    .data(|d| d.get_temp::<Vec<(&'static str, Rect)>>(Id::new(HUD_LAYOUT)))
                    .unwrap();
                let names: Vec<_> = layout.iter().map(|(n, _)| *n).collect();
                for name in ["reticle", "vitality", "tip", "notice"] {
                    assert!(names.contains(&name), "{name} missing: {names:?}");
                }
                // Status, descent, gold, compass, last threat, the boss (its
                // own panel up to 110%), chalice, armor (up to 110%), dodge
                // and weapon.
                let panels = names.iter().filter(|n| **n == "panel").count();
                let compact = k > Preferences::COMPACT_ABOVE;
                assert_eq!(panels, if compact { 8 } else { 10 }, "{k}");
                // Combat feedback draws over field notes and notices, which
                // may reach the arcs' circle at larger sizes; nothing else may.
                let feedback_over = |a: &str, b: &str| {
                    (a == "reticle" && matches!(b, "tip" | "notice"))
                        || (b == "reticle" && matches!(a, "tip" | "notice"))
                };
                for (i, (a, ra)) in layout.iter().enumerate() {
                    for (b, rb) in &layout[i + 1..] {
                        assert!(
                            !ra.intersects(*rb) || feedback_over(a, b),
                            "{a} {ra:?} overlaps {b} {rb:?} at {k} in {width}x{height}"
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn the_pause_ledger_fits_beside_the_menu() {
        for (width, height) in [(1440., 900.), (960., 600.), (1920., 1080.)] {
            for full in [false, true] {
                let ctx = egui::Context::default();
                configure(&ctx);
                let mut g = Game::new(false);
                g.mode = Mode::Paused;
                if full {
                    g.run.survival.ranks = [5; 10];
                    g.run.armor = 30.;
                    g.run.gold = 98765;
                    g.run.kills = 12345;
                    g.run.time = 3894.;
                    g.run.weapon.paths = [5, 5, 3];
                    g.run.weapon.major = true;
                } else {
                    g.run.survival.ranks[6] = 1;
                }
                // The notice a lost controller leaves over the pause menu.
                g.notify("CONTROLLER DISCONNECTED / PAUSED");
                let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(width, height));
                for _ in 0..2 {
                    let input = egui::RawInput {
                        screen_rect: Some(screen),
                        ..Default::default()
                    };
                    let _ = ctx.run(input, |ctx| draw(ctx, &mut g, Mat4::IDENTITY));
                }
                let layout = ctx
                    .data(|d| d.get_temp::<Vec<(&'static str, Rect)>>(Id::new(HUD_LAYOUT)))
                    .unwrap();
                let of = |name: &str| -> Vec<Rect> {
                    layout.iter().filter(|(n, _)| *n == name).map(|(_, r)| *r).collect()
                };
                let (folio, panels, text) = (of("pause_folio"), of("panel"), of("ledger_text"));
                assert_eq!((folio.len(), panels.len()), (1, 2), "{width}x{height}");
                // The run's rows plus one block per power held.
                assert_eq!(text.len(), 1 + if full { 10 } else { 1 });
                let notice = of("notice");
                assert_eq!(notice.len(), 1);
                for panel in &panels {
                    assert!(!panel.intersects(folio[0]), "{panel:?} at {width}x{height}");
                    assert!(!panel.intersects(notice[0]), "{panel:?} at {width}x{height}");
                    assert!(screen.contains_rect(*panel), "{panel:?} at {width}x{height}");
                }
                for block in &text {
                    assert!(
                        panels.iter().any(|p| p.contains_rect(*block)),
                        "{block:?} leaves its panel at {width}x{height}"
                    );
                }
            }
        }
    }
    #[test]
    fn the_chronicle_fits_beside_the_title_menu() {
        use crate::game::{Attack, Cause, WeaponKind};
        // The longest blow and weapon names there are.
        let cause = (0..12)
            .flat_map(|kind| {
                [Attack::Strike, Attack::Bolt, Attack::Slam, Attack::Burst]
                    .map(|attack| Cause { kind, attack })
            })
            .max_by_key(|c| c.describe().len())
            .unwrap();
        let weapon = WeaponKind::ALL
            .iter()
            .flat_map(|&kind| (0..4).map(move |rarity| Card { kind, rarity, ..Card::starter() }))
            .map(|card| card.name())
            .max_by_key(|name| name.len())
            .unwrap();
        let past = |ending, descent, endless| PastRun {
            number: 9999,
            descent,
            ending,
            endless,
            souls: 123_456,
            time: 35_999.,
            level: 199,
            weapon: weapon.clone(),
        };
        let history = vec![
            past(Ending::Slain(Some(cause)), 9999, true),
            past(Ending::Victory, 12, false),
            past(Ending::Slain(Some(cause)), 11, false),
            past(Ending::Abandoned, 3, false),
            past(Ending::Slain(None), 1, false),
            past(Ending::Abandoned, 2, false),
        ];
        for (width, height) in [(1440., 900.), (960., 600.), (1920., 1080.)] {
            let ctx = egui::Context::default();
            configure(&ctx);
            let mut g = Game::new(false);
            g.mode = Mode::Title;
            g.has_save = true;
            g.records.runs = 9999;
            g.records.deepest = 9999;
            g.records.most_souls = 123_456;
            g.records.fastest_victory = Some(35_999.);
            g.records.history = history.clone();
            let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(width, height));
            for _ in 0..2 {
                let input = egui::RawInput {
                    screen_rect: Some(screen),
                    ..Default::default()
                };
                let _ = ctx.run(input, |ctx| draw(ctx, &mut g, Mat4::IDENTITY));
            }
            let layout = ctx
                .data(|d| d.get_temp::<Vec<(&'static str, Rect)>>(Id::new(HUD_LAYOUT)))
                .unwrap();
            let of = |name: &str| -> Vec<Rect> {
                layout.iter().filter(|(n, _)| *n == name).map(|(_, r)| *r).collect()
            };
            let (panels, lines, descents) =
                (of("panel"), of("chronicle_line"), of("chronicle_descent"));
            assert_eq!(panels.len(), 1, "{width}x{height}");
            assert_eq!((lines.len(), descents.len()), (2 * CHRONICLE_SHOWN, CHRONICLE_SHOWN));
            let panel = panels[0];
            assert!(screen.contains_rect(panel), "{panel:?} at {width}x{height}");
            // Clear of every title button, the logo and the records line.
            for target in pad_targets(&ctx) {
                assert!(!panel.intersects(target.rect), "{:?} at {width}x{height}", target.rect);
            }
            let mut left = Rect::NOTHING;
            with_canvas(width, height, |c| left = c.rect(0., 100., 640., 800.));
            assert!(!panel.intersects(left), "{panel:?} at {width}x{height}");
            for (i, line) in lines.iter().enumerate() {
                assert!(panel.contains_rect(line.shrink(0.5)), "{line:?} at {width}x{height}");
                // A run's ending stays clear of the descent on its right.
                if i % 2 == 0 {
                    let descent = descents[i / 2];
                    assert!(line.max.x + 8. < descent.min.x, "{line:?} {descent:?} at {width}x{height}");
                }
            }
            for descent in &descents {
                assert!(panel.contains_rect(descent.shrink(0.5)));
            }
        }
        // No panel before the first run ends.
        let ctx = egui::Context::default();
        configure(&ctx);
        let mut g = Game::new(false);
        g.mode = Mode::Title;
        let _ = ctx.run(egui::RawInput::default(), |ctx| draw(ctx, &mut g, Mat4::IDENTITY));
        let layout = ctx
            .data(|d| d.get_temp::<Vec<(&'static str, Rect)>>(Id::new(HUD_LAYOUT)))
            .unwrap();
        assert!(layout.iter().all(|(n, _)| *n != "panel"));
    }
    #[test]
    fn hud_groups_scale_around_their_anchors() {
        for (width, height) in [(1440., 900.), (960., 600.), (1920., 1080.)] {
            with_canvas(width, height, |base| {
                let h = base.h;
                for k in Preferences::HUD_SCALES {
                    for (ax, ay) in [(0., 0.), (720., 0.), (1440., h), (720., h)] {
                        let c = base.anchored(k, ax, ay);
                        // The anchor stays put and distances from it scale by k.
                        assert!((c.pt(ax, ay) - base.pt(ax, ay)).length() < 1e-3);
                        let moved = c.pt(ax - 100., ay - 50.) - c.pt(ax, ay);
                        let unscaled = base.pt(ax - 100., ay - 50.) - base.pt(ax, ay);
                        assert!((moved - unscaled * k).length() < 1e-3, "{k} at ({ax}, {ay})");
                        // Text never grows below its normal floor, and shrinks with the HUD.
                        assert_eq!(c.floor, 13.5 * k.min(1.));
                    }
                }
                let shifted = base.shifted(10., -4.);
                let gap = shifted.pt(0., 0.) - base.pt(0., 0.);
                assert!((gap - Vec2::new(10., -4.) * base.s).length() < 1e-3);
            });
        }
    }
}
