// 직접 그리는 아이콘.
// 색은 청록 하나만 강조에 쓰고 나머지는 회색. 삭제는 마우스를 올렸을 때만 빨간색.
// 이미지 파일 없이 도형으로 그려서 어떤 크기에서도 선명함

use eframe::egui::{
    Color32, CornerRadius, Painter, Pos2, Rect, Response, Role, Sense, Shape, Stroke, Ui, Vec2,
    WidgetInfo, pos2, vec2,
};

#[derive(Clone, Copy)]
pub enum Icon {
    Play,
    Pause,
    Stop,
    Refresh,
    Trash,
    Folder,
    Download,
    Speaker,
    // 소리 파일 표시
    Music,
    // 동영상 파일 표시
    Video,
}

// 강조색(청록색). 저장, 재생, 일시정지에 사용
pub const ACCENT: Color32 = Color32::from_rgb(22, 128, 118);
// 삭제 버튼에 마우스를 올렸을 때의 색
const DANGER: Color32 = Color32::from_rgb(200, 70, 66);

// 아이콘 하나가 들어가는 버튼. label은 마우스를 올렸을 때 보여줄 설명
pub fn button(ui: &mut Ui, icon: Icon, label: String) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::click());
    describe(&response, ui, &label);

    if ui.is_rect_visible(rect) {
        let (mut fill, mut glyph) = match icon {
            // 주요 동작은 강조색
            Icon::Play | Icon::Pause => (ACCENT, Color32::WHITE),
            _ => neutral(ui),
        };
        if response.hovered() {
            (fill, glyph) = match icon {
                Icon::Trash => (DANGER, Color32::WHITE),
                _ => (brighten(fill), glyph),
            };
        }
        if response.is_pointer_button_down_on() {
            fill = fill.gamma_multiply(0.8).to_opaque();
        }
        ui.painter().rect_filled(rect, CornerRadius::same(8), fill);
        paint(ui.painter(), rect.shrink(7.0), icon, glyph, fill);
    }

    response.on_hover_text(label)
}

// 설명용 그림 (누를 수 없음)
pub fn picture(ui: &mut Ui, icon: Icon, label: String) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
    if ui.is_rect_visible(rect) {
        let weak = ui.visuals().weak_text_color();
        match icon {
            // 파일 종류는 옅은 청록 배경으로 표시
            Icon::Music | Icon::Video => {
                let accent = ui.visuals().hyperlink_color;
                let tint = ui.visuals().panel_fill.lerp_to_gamma(accent, 0.18);
                ui.painter().rect_filled(rect, CornerRadius::same(8), tint);
                paint(ui.painter(), rect.shrink(7.0), icon, accent, tint);
            }
            // 나머지는 배경 없이 그려서 버튼과 구별
            _ => {
                let background = ui.visuals().panel_fill;
                paint(ui.painter(), rect.shrink(6.0), icon, weak, background);
            }
        }
    }
    response.on_hover_text(label)
}

// 보조 버튼의 배경색과 그림색
fn neutral(ui: &Ui) -> (Color32, Color32) {
    if ui.visuals().dark_mode {
        (Color32::from_gray(54), Color32::from_gray(215))
    } else {
        (Color32::from_gray(228), Color32::from_gray(70))
    }
}

fn brighten(color: Color32) -> Color32 {
    color.lerp_to_gamma(Color32::WHITE, 0.15)
}

// 아이콘과 글자가 함께 있는 강조 버튼
pub fn primary_button(ui: &mut Ui, icon: Icon, label: String, enabled: bool) -> Response {
    let enabled = enabled && ui.is_enabled();
    let font = eframe::egui::TextStyle::Button.resolve(ui.style());
    let text = ui
        .painter()
        .layout_no_wrap(label.clone(), font, Color32::WHITE);

    // 여백 12 + 아이콘 16 + 간격 6 + 글자 + 여백 14
    let size = vec2((text.size().x + 48.0).max(92.0), 32.0);
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(size, sense);
    describe(&response, ui, &label);

    // 누를 수 없을 때는 흐리게, 마우스를 올리면 밝게
    let mut painter = ui.painter().clone();
    if !enabled {
        painter.multiply_opacity(0.4);
    }
    let fill = if response.hovered() && enabled {
        brighten(ACCENT)
    } else {
        ACCENT
    };
    painter.rect_filled(rect, CornerRadius::same(10), fill);

    let content = 16.0 + 6.0 + text.size().x;
    let left = rect.center().x - content / 2.0;
    let icon_rect = Rect::from_center_size(pos2(left + 8.0, rect.center().y), Vec2::splat(16.0));
    paint(&painter, icon_rect, icon, Color32::WHITE, fill);
    let text_pos = pos2(left + 22.0, rect.center().y - text.size().y / 2.0);
    painter.galley(text_pos, text, Color32::WHITE);

    response
}

// 화면 읽기 프로그램이 버튼의 이름을 알 수 있게 하기
fn describe(response: &Response, ui: &Ui, label: &str) {
    response.widget_info(|| WidgetInfo::labeled(Role::Button, ui.is_enabled(), label));
}

// rect 안에 채운 모양의 아이콘 그리기. 좌표는 0.0 ~ 1.0 비율로 지정.
// background는 그림 안을 파낼 때 쓰는 배경색
fn paint(painter: &Painter, rect: Rect, icon: Icon, color: Color32, background: Color32) {
    let at = |x: f32, y: f32| rect.min + vec2(x * rect.width(), y * rect.height());
    // 볼록 다각형만 채울 수 있으므로 복잡한 모양은 나눠서 그림
    let fill = |points: Vec<Pos2>, color: Color32| {
        painter.add(Shape::convex_polygon(points, color, Stroke::NONE));
    };
    let block = |x1: f32, y1: f32, x2: f32, y2: f32, radius: u8| {
        painter.rect_filled(
            Rect::from_min_max(at(x1, y1), at(x2, y2)),
            CornerRadius::same(radius),
            color,
        );
    };

    match icon {
        Icon::Play => fill(vec![at(0.18, 0.02), at(0.98, 0.5), at(0.18, 0.98)], color),
        Icon::Pause => {
            block(0.12, 0.04, 0.40, 0.96, 2);
            block(0.60, 0.04, 0.88, 0.96, 2);
        }
        Icon::Stop => block(0.08, 0.08, 0.92, 0.92, 3),
        Icon::Refresh => {
            // 오른쪽 위가 열린 굵은 원과 화살촉
            let center = rect.center();
            let radius = rect.width() * 0.39;
            let point = |degrees: f32| {
                let angle = degrees.to_radians();
                center + radius * vec2(angle.cos(), angle.sin())
            };
            let arc = (0..=24).map(|i| point(i as f32 * 285.0 / 24.0)).collect();
            painter.add(Shape::line(arc, Stroke::new(rect.width() * 0.19, color)));
            let tip = point(285.0);
            let size = rect.width() * 0.34;
            // 원이 진행하는 방향과 그에 수직인 방향
            let angle = 285.0_f32.to_radians();
            let forward = vec2(-angle.sin(), angle.cos());
            let side = vec2(angle.cos(), angle.sin());
            fill(
                vec![
                    tip + forward * size,
                    tip + side * size * 0.85,
                    tip - side * size * 0.85,
                ],
                color,
            );
        }
        Icon::Trash => {
            block(0.34, 0.00, 0.66, 0.18, 1);
            block(0.04, 0.12, 0.96, 0.27, 1);
            fill(
                vec![at(0.14, 0.35), at(0.86, 0.35), at(0.78, 1.0), at(0.22, 1.0)],
                color,
            );
        }
        Icon::Folder => {
            block(0.00, 0.08, 0.46, 0.40, 2);
            block(0.00, 0.24, 1.00, 0.92, 2);
        }
        Icon::Download => {
            block(0.37, 0.00, 0.63, 0.40, 0);
            fill(vec![at(0.13, 0.36), at(0.87, 0.36), at(0.50, 0.72)], color);
            block(0.04, 0.82, 0.96, 1.00, 1);
        }
        Icon::Speaker => {
            block(0.00, 0.30, 0.26, 0.70, 1);
            fill(
                vec![
                    at(0.22, 0.30),
                    at(0.58, 0.02),
                    at(0.58, 0.98),
                    at(0.22, 0.70),
                ],
                color,
            );
            let wave = [
                (0.74, 0.28),
                (0.86, 0.38),
                (0.90, 0.50),
                (0.86, 0.62),
                (0.74, 0.72),
            ];
            painter.add(Shape::line(
                wave.iter().map(|&(x, y)| at(x, y)).collect(),
                Stroke::new(rect.width() * 0.13, color),
            ));
        }
        Icon::Music => {
            // 두 음표를 잇는 막대, 기둥, 머리
            fill(
                vec![
                    at(0.28, 0.14),
                    at(0.96, 0.00),
                    at(0.96, 0.24),
                    at(0.28, 0.38),
                ],
                color,
            );
            block(0.28, 0.20, 0.40, 0.82, 0);
            block(0.84, 0.10, 0.96, 0.72, 0);
            painter.circle_filled(at(0.22, 0.82), rect.width() * 0.18, color);
            painter.circle_filled(at(0.78, 0.72), rect.width() * 0.18, color);
        }
        Icon::Video => {
            block(0.00, 0.10, 1.00, 0.90, 4);
            // 가운데의 재생 삼각형은 배경색으로 파내기
            fill(
                vec![at(0.36, 0.28), at(0.72, 0.50), at(0.36, 0.72)],
                background,
            );
        }
    }
}
