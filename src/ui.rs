use crate::controls::{Action, Device};
use crate::game::{Card, DAMAGE_MARK_LIFE, Game, HIT_MARKER_LIFE, HitKind, Mode, Preferences};
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
struct Canvas<'a> {
    ui: &'a egui::Ui,
    p: egui::Painter,
    s: f32,
    offset: Vec2,
    h: f32,
    time: f32,
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
        let font = role.font((size * self.s).max(13.5));
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
    fn fit_type(&self, text: &str, role: TypeRole, size: f32, width: f32) -> f32 {
        let galley =
            self.p
                .layout_no_wrap(text.into(), role.font((size * self.s).max(13.5)), IVORY);
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
            role.font((size.max(18.) * self.s).max(13.5)),
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
        let hover = self
            .ui
            .ctx()
            .animate_bool(Id::new((id, "hover")), r.hovered() || r.has_focus());
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
        self.line((x, y), (x + w, y), C::from_rgb(91, 66, 40), 5.);
        self.line((x, y - 1.), (x + w * t, y - 1.), GOLD, 2.);
        for i in 0..=10 {
            let xx = x + i as f32 * w / 10.;
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
        crate::weapons::GROUPS[card.kind.spec().group].to_uppercase(),
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
    c.center(
        x + w * 0.5,
        y + h * 0.663,
        ["COMMON", "UNCOMMON", "RARE", "LEGENDARY"][card.rarity],
        16.,
        ink,
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
// Estimated damage per second, inked green or red against the equipped card.
fn dps_comparison(card: &Card, equipped: &Card) -> (String, C) {
    let dps = card.sustained_dps();
    let delta = (dps - equipped.sustained_dps()).round();
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
    let r = g.records;
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
        field_tip(c, &active.tip.text(g), 14., 600., active.alpha());
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
    if c.ui
        .interact(
            c.rect(280., y + 16., 70., 103.),
            Id::new("equipped"),
            Sense::click(),
        )
        .clicked()
    {
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
        y + 27.,
        292.,
        59.,
        &format!("ENTER DESCENT {:02}  >", g.run.wave + 1),
        true,
    ) {
        g.next_wave();
    }
    c.center(1226., y + 108., "SAME FOREST. HIGHER STAKES.", 9., MUTED);
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
fn world_compass(c: &Canvas, g: &Game) {
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
    c.hud_panel(493., 120., 454., 70.);
    c.line((516., 152.), (924., 152.), C::from_rgb(78, 70, 51), 1.);
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
    direction_pointer(c, 516., 174., relative, GOLD);
    c.text_role(
        541.,
        174.,
        next.name,
        16.,
        IVORY,
        TypeRole::Strong,
        Align2::LEFT_CENTER,
    );
    c.text(
        928.,
        174.,
        format!("{}  /  {:.0} m", cardinal(bearing), distance),
        16.,
        GOLD,
        false,
        Align2::RIGHT_CENTER,
    );
}
fn last_threat(c: &Canvas, g: &Game) -> bool {
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
fn hud(c: &Canvas, g: &mut Game, vp: Mat4) {
    let h = c.h;
    let xp = &g.run.survival;
    let power_count = xp.ranks.iter().filter(|r| **r > 0).count();
    c.hud_panel(22., 18., 310., 154. + power_count as f32 * 24.);
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
        let y = 178. + row as f32 * 24.;
        c.text(
            40.,
            y,
            crate::survival::POWERS[i].0,
            16.,
            IVORY,
            false,
            Align2::LEFT_CENTER,
        );
        c.text_role(
            314.,
            y,
            format!("{rank}/5"),
            16.,
            GOLD,
            TypeRole::Strong,
            Align2::RIGHT_CENTER,
        );
    }
    c.hud_panel(493., 18., 454., 94.);
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
    c.center(
        720.,
        84.,
        format!(
            "{} HUNTING  /  {} APPROACHING",
            g.run.enemies.len(),
            xp.remaining
        ),
        18.,
        MUTED,
    );
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
    world_compass(c, g);
    let threat_shown = last_threat(c, g);
    let status_y = if threat_shown { 243. } else { 200. };
    let boss = g.run.enemies.iter().find(|e| e.kind == 3 && e.hp > 0.);
    if let Some(e) = boss {
        c.hud_panel(480., status_y, 480., 65.);
        c.center(720., status_y + 21., "THE TITHEKEEPER", 18., GOLD);
        c.fill(494., status_y + 45., 452., 7., C::from_rgb(54, 30, 28));
        c.fill(
            494.,
            status_y + 45.,
            452. * (e.hp / e.max_hp).clamp(0., 1.),
            7.,
            C::from_rgb(209, 65, 52),
        );
    }
    let d = 5. + g.flash * 38.;
    let cx = 720.;
    let cy = h * 0.5;
    for side in [-1., 1.] {
        c.line((cx + side * d, cy), (cx + side * (d + 6.), cy), IVORY, 1.);
        c.line((cx, cy + side * d), (cx, cy + side * (d + 6.)), IVORY, 1.);
    }
    c.p.circle_filled(c.pt(cx, cy), c.s, IVORY);
    hit_feedback(c, g, cx, cy);
    threat_pointers(c, g, cx, cy);
    // Keep the sculpted end caps away from the labels and digits.
    c.texture("button_plate", 16., h - 134., 395., 108., C::WHITE);
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
    if g.run.chalice {
        c.hud_panel(24., h - 166., 526., 34.);
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
            530.,
            h - 149.,
            format!("{:.0} MANA", g.run.mana),
            18.,
            IVORY,
            TypeRole::Strong,
            Align2::RIGHT_CENTER,
        );
    }
    c.hud_panel(601., h - 91., 238., 66.);
    c.fill(613., h - 73., 214., 6., C::from_rgb(53, 63, 58));
    c.fill(
        613.,
        h - 73.,
        214. * (1. - g.dash_cd / 1.5).clamp(0., 1.),
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
            c.text_role(
                (p.x * 0.5 + 0.5) * 1440.,
                (0.5 - p.y * 0.5) * h,
                &f.text,
                23.,
                IVORY,
                TypeRole::Strong,
                Align2::CENTER_CENTER,
            );
        }
    }
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
    if let Some(active) = g.tip.filter(|t| !t.tip.in_shop()) {
        field_tip(c, &active.tip.text(g), c.h - TIP_FROM_BOTTOM, 640., active.alpha());
    } else if g.run.time < 7. {
        let y = c.h - TIP_FROM_BOTTOM;
        let (fire, pause) = match g.device {
            Device::Keyboard => ("Mouse", "Esc"),
            Device::Controller => ("RT", "Start"),
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
/// Distance from the bottom of the screen to the top of the arena tip panel.
const TIP_FROM_BOTTOM: f32 = 272.;
/// A field tip: a small heading over one to three centred lines.
fn field_tip(c: &Canvas, text: &str, y: f32, width: f32, alpha: f32) {
    let x = 720. - width / 2.;
    let galley = c.p.layout_job({
        let mut job = egui::text::LayoutJob::simple(
            text.into(),
            TypeRole::Reading.font((18. * c.s).max(13.5)),
            IVORY,
            (width - 48.) * c.s,
        );
        job.halign = egui::Align::Center;
        job
    });
    let height = galley.size().y / c.s + 46.;
    c.inset(x, y, width, height, HUD_SURFACE.gamma_multiply(alpha));
    c.border(x, y, width, height, GOLD.gamma_multiply(0.75 * alpha));
    c.diamond(720., y, 5., GOLD.gamma_multiply(alpha));
    c.center(720., y + 17., "FIELD NOTE", 13., GOLD.gamma_multiply(alpha));
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
fn pause(c: &Canvas, g: &mut Game) {
    c.fill(-100., 0., 1640., c.h, C::from_black_alpha(165));
    let x = 464.;
    let y = 215.;
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
/// The controller's virtual pointer, drawn above every menu and modal.
pub fn pad_cursor(ctx: &egui::Context, pos: Pos2) {
    let p = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Tooltip,
        Id::new("pad_cursor"),
    ));
    p.circle_stroke(pos, 11., Stroke::new(4.5, C::from_black_alpha(170)));
    p.circle_stroke(pos, 11., Stroke::new(2., GOLD));
    p.circle_filled(pos, 2.5, IVORY);
}
pub fn draw(ctx: &egui::Context, g: &mut Game, vp: Mat4) {
    let old = ctx.data(|d| d.get_temp::<(Mode, f32)>(Id::new("mode_transition")));
    if old.is_none_or(|(mode, _)| mode != g.mode) {
        ctx.data_mut(|d| d.insert_temp(Id::new("mode_transition"), (g.mode, g.elapsed)));
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
                Mode::Paused => {
                    hud(&c, g, vp);
                    pause(&c, g);
                }
                Mode::Dead | Mode::Victory => ending(&c, g),
            }
            if g.notice_time > 0.
                && matches!(
                    g.mode,
                    Mode::Arena | Mode::Shop | Mode::Tree | Mode::Pack | Mode::Paused | Mode::Title
                )
            {
                let tip_panel =
                    g.tip.is_some_and(|t| !t.tip.in_shop()) || g.run.time < 7.;
                let y = if g.mode != Mode::Arena {
                    128.
                } else if tip_panel {
                    c.h - TIP_FROM_BOTTOM - 40.
                } else {
                    c.h * 0.72
                };
                let alpha = (g.notice_time.min(1.) * 245.) as u8;
                c.fill(
                    310.,
                    y - 17.,
                    820.,
                    35.,
                    C::from_black_alpha(alpha.min(210)),
                );
                c.center(
                    720.,
                    y,
                    &g.notice,
                    11.,
                    GOLD.gamma_multiply(g.notice_time.min(1.)),
                );
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
    for (id, x, label, controls) in [
        ("journal_preferences", 418., "PREFERENCES", false),
        ("journal_controls", 742., "CONTROLS", true),
    ] {
        let open = g.journal_controls == controls;
        if c.button(id, x, 236., 280., 38., label, open) {
            g.journal_controls = controls;
            g.rebinding = None;
            g.controls_note.clear();
        }
        if open {
            c.line((x + 40., 284.), (x + 240., 284.), INK, 2.);
            c.diamond(x + 140., 284., 5., INK);
        }
    }
    if g.journal_controls {
        journal_controls(c, g);
    } else {
        journal_preferences(c, g);
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
            None,
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
        (
            4,
            "Hollowlight / F6",
            "hollowlight_effects",
            &mut g.shader_intensity,
            0.,
            1.,
            Some("%"),
        ),
    ] {
        let y = JOURNAL_SLIDER_Y + row as f32 * 44.;
        c.text(392., y, label, 17., INK, true, Align2::LEFT_CENTER);
        c.slider(id, 615., y + 1., 360., value, min, max);
        if let Some(unit) = readout {
            let shown = if unit == "%" { *value * 100. } else { *value };
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
        JOURNAL_TOGGLES_Y + 88.,
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
    if c.button(
        "vsync",
        392.,
        JOURNAL_TOGGLES_Y + 44.,
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
        JOURNAL_TOGGLES_Y + 44.,
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
            g.rebinding = (!waiting).then_some(action);
            g.controls_note.clear();
        }
    }
    // Clicking anywhere else stops waiting for a key.
    if g.rebinding.is_some() && !clicked_key && c.ui.input(|i| i.pointer.any_click()) {
        g.rebinding = None;
    }
    for (i, line) in [
        "Left mouse  fire    1 2 3  choose a power    Esc  pause    F11  fullscreen",
        "F6  Hollowlight    F7  VSync    F8  FPS counter",
        "Controller   RT fire  /  LT sprint  /  A dodge  /  X reload  /  B melee  /  Y bolt",
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
            "Press a key or mouse button for {}. Escape cancels.",
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
        g.rebinding = None;
        g.controls_note = "Default keys restored.".into();
        g.save_preferences();
    }
}
/// Top row of the Preferences page sliders; the Hollowlight slider is row 4.
pub const JOURNAL_SLIDER_Y: f32 = 312.;
/// Top of the Preferences page switches: invert and flashes, then the
/// presentation row (VSync, FPS), then field tips, 44 apart.
pub const JOURNAL_TOGGLES_Y: f32 = 560.;
