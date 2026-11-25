// 화면 색상. 바탕, 글자, 강조색 3개만 사용하고
// 연한 색이 필요하면 이 셋을 섞어서 만듦

use eframe::egui::{self, Color32, Stroke, Theme};

#[derive(Clone, Copy)]
pub struct Palette {
    // 바탕
    pub base: Color32,
    // 글자
    pub ink: Color32,
    // 강조
    pub point: Color32,
    // 강조색 위에 올리는 글자
    pub on_point: Color32,
    // 헤더와 재생 영역의 바탕. 바탕색보다 한 단계 구별되는 색
    pub edge: Color32,
}

// 어두운 화면: 남색 바탕과 하늘 파랑
const DARK: Palette = Palette {
    base: Color32::from_rgb(21, 27, 43),
    ink: Color32::from_rgb(232, 236, 245),
    point: Color32::from_rgb(90, 169, 255),
    on_point: Color32::from_rgb(6, 24, 46),
    edge: Color32::from_rgb(35, 44, 69),
};

// 밝은 화면: 흰 바탕에 남색 글자
const LIGHT: Palette = Palette {
    base: Color32::WHITE,
    ink: Color32::from_rgb(28, 37, 64),
    point: Color32::from_rgb(47, 111, 228),
    on_point: Color32::WHITE,
    edge: Color32::from_rgb(233, 237, 246),
};

impl Palette {
    // 바탕에 글자색을 amount(0.0 ~ 1.0)만큼 섞은 색
    pub fn shade(self, amount: f32) -> Color32 {
        self.base.lerp_to_gamma(self.ink, amount)
    }

    // 어떤 바탕 위에서도 보이도록 반투명하게 만든 글자색
    pub fn veil(self, amount: f32) -> Color32 {
        self.ink.gamma_multiply(amount)
    }

    // 바탕에 강조색을 섞은 색
    pub fn tint(self, amount: f32) -> Color32 {
        self.base.lerp_to_gamma(self.point, amount)
    }
}

// 지금 화면 모드의 색
pub fn palette(ui: &egui::Ui) -> Palette {
    if ui.visuals().dark_mode { DARK } else { LIGHT }
}

// 어두운 화면과 밝은 화면 모두에 색 적용
pub fn apply(ctx: &egui::Context) {
    for (theme, colors) in [(Theme::Dark, DARK), (Theme::Light, LIGHT)] {
        ctx.style_mut_of(theme, |style| {
            // 줄의 기본 높이를 아이콘 버튼에 맞춰서 글자와 버튼의 세로 위치를 가지런히 하기
            style.spacing.interact_size.y = 28.0;

            let visuals = &mut style.visuals;
            visuals.panel_fill = colors.base;
            visuals.window_fill = colors.base;
            // 입력창의 바탕
            visuals.extreme_bg_color = colors.shade(0.07);
            visuals.faint_bg_color = colors.shade(0.05);
            visuals.weak_text_color = Some(colors.shade(0.55));
            visuals.hyperlink_color = colors.point;
            // 선택한 부분은 강조색을 옅게 깔고 강조색 글자
            visuals.selection.bg_fill = colors.tint(0.28);
            visuals.selection.stroke = Stroke::new(1.0, colors.point);

            let widgets = &mut visuals.widgets;
            // 글자와 분리선
            widgets.noninteractive.bg_fill = colors.base;
            widgets.noninteractive.bg_stroke = Stroke::new(1.0, colors.shade(0.14));
            widgets.noninteractive.fg_stroke.color = colors.shade(0.85);
            // 평소, 마우스를 올렸을 때, 누를 때의 버튼
            for (widget, amount) in [
                (&mut widgets.inactive, 0.12),
                (&mut widgets.hovered, 0.20),
                (&mut widgets.active, 0.27),
                (&mut widgets.open, 0.16),
            ] {
                widget.bg_fill = colors.shade(amount);
                widget.weak_bg_fill = colors.shade(amount);
                widget.fg_stroke.color = colors.ink;
            }
            widgets.hovered.bg_stroke = Stroke::new(1.0, colors.point);
        });
    }
}
