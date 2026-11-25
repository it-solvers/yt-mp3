// 화면 색상. 바탕, 글자, 강조색 3개만 사용하고
// 연한 색이 필요하면 이 셋을 섞어서 만듦.
// 색은 미리 만든 조합에서 고르거나, 3개를 직접 정할 수 있음

use eframe::egui::{self, Color32, Id, Stroke, Theme};

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

// 바탕, 글자, 강조색
pub type Colors = [Color32; 3];

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

// 미리 만든 색 조합: 어두운 화면의 색과 밝은 화면의 색
pub const PRESETS: [[Colors; 2]; 3] = [
    // 하늘
    [
        [rgb(0x000000), rgb(0xececee), rgb(0x5aa9ff)],
        [rgb(0xffffff), rgb(0x1c2540), rgb(0x2f6fe4)],
    ],
    // 산호
    [
        [rgb(0x000000), rgb(0xeeeeee), rgb(0xff6b5e)],
        [rgb(0xfffdfb), rgb(0x262321), rgb(0xd93f33)],
    ],
    // 연보라
    [
        [rgb(0x000000), rgb(0xececf2), rgb(0xa991ff)],
        [rgb(0xffffff), rgb(0x221c40), rgb(0x6f4fe0)],
    ],
];

// 사용자가 고른 색
#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct Choice {
    // PRESETS의 번호
    pub preset: usize,
    // 직접 고른 색. 어두운 화면, 밝은 화면 순서이며 고르지 않았으면 None
    pub custom: [Option<Colors>; 2],
}

impl Choice {
    // mode(0: 어두운 화면, 1: 밝은 화면)에서 쓸 색
    pub fn colors(self, mode: usize) -> Colors {
        self.custom[mode].unwrap_or(PRESETS[self.preset.min(PRESETS.len() - 1)][mode])
    }
}

// 설정에 저장할 때의 모양: "000000,ececee,5aa9ff"
pub fn format_colors(colors: Colors) -> String {
    colors
        .map(|c| format!("{:02x}{:02x}{:02x}", c.r(), c.g(), c.b()))
        .join(",")
}

pub fn parse_colors(text: &str) -> Option<Colors> {
    let colors: Vec<Color32> = text
        .split(',')
        .filter(|part| part.len() == 6)
        .filter_map(|part| u32::from_str_radix(part, 16).ok())
        .map(rgb)
        .collect();
    colors.try_into().ok()
}

impl Palette {
    fn new([base, ink, point]: Colors) -> Self {
        // 강조색이 밝으면 그 위의 글자는 어둡게, 어두우면 흰색
        let brightness =
            (point.r() as u32 * 299 + point.g() as u32 * 587 + point.b() as u32 * 114) / 1000;
        let on_point = if brightness > 140 {
            rgb(0x07101c)
        } else {
            Color32::WHITE
        };
        Self {
            base,
            ink,
            point,
            on_point,
            edge: base.lerp_to_gamma(ink, 0.085),
        }
    }

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

// 지금 적용한 색(어두운 화면, 밝은 화면)을 egui에 보관할 때의 이름
fn palettes_id() -> Id {
    Id::unique("palettes")
}

// 지금 화면 모드의 색
pub fn palette(ui: &egui::Ui) -> Palette {
    let mode = usize::from(!ui.visuals().dark_mode);
    let saved: Option<[Palette; 2]> = ui.ctx().data(|data| data.get_temp(palettes_id()));
    saved.map_or_else(
        || Palette::new(Choice::default().colors(mode)),
        |palettes| palettes[mode],
    )
}

// 어두운 화면과 밝은 화면 모두에 색 적용
pub fn apply(ctx: &egui::Context, choice: Choice) {
    let palettes = [0, 1].map(|mode| Palette::new(choice.colors(mode)));
    ctx.data_mut(|data| data.insert_temp(palettes_id(), palettes));

    for (theme, colors) in [Theme::Dark, Theme::Light].into_iter().zip(palettes) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_and_reads_colors() {
        let colors = PRESETS[1][0];
        assert_eq!(format_colors(colors), "000000,eeeeee,ff6b5e");
        assert_eq!(parse_colors(&format_colors(colors)), Some(colors));
        assert_eq!(parse_colors("000000,ffffff"), None);
        assert_eq!(parse_colors("zzzzzz,ffffff,000000"), None);
    }

    #[test]
    fn custom_colors_replace_preset_for_one_mode() {
        let custom = PRESETS[2][0];
        let choice = Choice {
            preset: 1,
            custom: [Some(custom), None],
        };
        assert_eq!(choice.colors(0), custom);
        assert_eq!(choice.colors(1), PRESETS[1][1]);
    }
}
