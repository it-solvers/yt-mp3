// 직접 그리는 아이콘.
// 색은 theme.rs의 3색만 사용. 저장과 재생 중인 버튼만 강조색, 파일 종류는 모양으로 구별.
// 이미지 파일 없이 도형으로 그려서 어떤 크기에서도 선명함

use crate::theme::palette;
use eframe::egui::{
    Color32, CornerRadius, CursorIcon, Painter, Pos2, Rect, Response, Role, Sense, Shape, Stroke,
    StrokeKind, Ui, Vec2, WidgetInfo, pos2, vec2,
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

// 아이콘 하나가 들어가는 버튼. label은 마우스를 올렸을 때 보여줄 설명
pub fn button(ui: &mut Ui, icon: Icon, label: String) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::click());
    describe(&response, ui, &label);

    if ui.is_rect_visible(rect) {
        let colors = palette(ui);
        let pressed = response.is_pointer_button_down_on();
        let hovered = response.hovered();
        let (fill, glyph) = match icon {
            // 재생 중임을 알리는 일시정지만 강조색
            Icon::Pause => {
                let fill = if pressed {
                    colors.point.lerp_to_gamma(colors.base, 0.2)
                } else if hovered {
                    colors.point.lerp_to_gamma(colors.ink, 0.15)
                } else {
                    colors.point
                };
                (fill, colors.on_point)
            }
            // 나머지는 반투명해서 헤더와 재생 bar 위에서도 구별됨
            _ => {
                let amount = if pressed {
                    0.28
                } else if hovered {
                    0.20
                } else {
                    0.12
                };
                (colors.veil(amount), colors.ink)
            }
        };
        ui.painter().rect_filled(rect, CornerRadius::same(8), fill);
        paint(ui.painter(), rect.shrink(7.0), icon, glyph, fill);
    }

    response.on_hover_text(label)
}

// 입력창 안에 올려놓는 지우기(×) 버튼. rect 위치에 그림
pub fn clear_button(ui: &mut Ui, rect: Rect, label: String) -> Response {
    let response = ui.interact(rect, ui.scope_id().with("clear"), Sense::click());
    describe(&response, ui, &label);

    // 평소에는 흐리게, 마우스를 올리면 둥근 배경과 함께 진하게
    let color = if response.hovered() {
        let background = palette(ui).shade(0.12);
        ui.painter()
            .circle_filled(rect.center(), rect.width() / 2.0, background);
        ui.visuals().strong_text_color()
    } else {
        ui.visuals().weak_text_color()
    };
    let cross = rect.shrink(rect.width() * 0.32);
    let stroke = Stroke::new(1.8, color);
    ui.painter()
        .line_segment([cross.left_top(), cross.right_bottom()], stroke);
    ui.painter()
        .line_segment([cross.right_top(), cross.left_bottom()], stroke);

    response
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text(label)
}

// 목록 앞의 파일 아이콘 겸 재생 버튼.
// 평소에는 파일 종류(kind: Music 또는 Video)를 보여주고, 마우스를 올리면 재생 모양.
// paused는 이 파일이 재생 중일 때의 일시정지 여부, 재생 중이 아니면 None
pub fn file_button(ui: &mut Ui, kind: Icon, paused: Option<bool>, label: String) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::click());
    describe(&response, ui, &label);

    if ui.is_rect_visible(rect) {
        let colors = palette(ui);
        let radius = CornerRadius::same(8);
        let hovered = response.hovered();

        if paused.is_some() || hovered {
            // 재생 중이거나 마우스를 올린 상태: 강조색 버튼
            let fill = if hovered {
                colors.point.lerp_to_gamma(colors.ink, 0.15)
            } else {
                colors.point
            };
            let glyph = match paused {
                Some(false) => Icon::Pause,
                _ => Icon::Play,
            };
            ui.painter().rect_filled(rect, radius, fill);
            paint(ui.painter(), rect.shrink(7.0), glyph, colors.on_point, fill);
        } else {
            // 평소: 테두리만 있는 칸에 파일 종류 (음표 또는 필름)
            let line = Stroke::new(1.5, colors.shade(0.35));
            ui.painter()
                .rect_stroke(rect, radius, line, StrokeKind::Inside);
            paint(
                ui.painter(),
                rect.shrink(6.0),
                kind,
                colors.ink,
                colors.base,
            );
        }
    }

    response
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text(label)
}

// 설명용 그림 (누를 수 없음). 배경 없이 그려서 버튼과 구별
pub fn picture(ui: &mut Ui, icon: Icon, label: String) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
    if ui.is_rect_visible(rect) {
        let colors = palette(ui);
        paint(
            ui.painter(),
            rect.shrink(6.0),
            icon,
            colors.shade(0.55),
            colors.base,
        );
    }
    response.on_hover_text(label)
}

// 아이콘과 글자가 함께 있는 강조 버튼
pub fn primary_button(ui: &mut Ui, icon: Icon, label: String, enabled: bool) -> Response {
    let enabled = enabled && ui.is_enabled();
    let colors = palette(ui);
    let font = eframe::egui::TextStyle::Button.resolve(ui.style());
    let text = ui
        .painter()
        .layout_no_wrap(label.clone(), font, colors.on_point);

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
        colors.point.lerp_to_gamma(colors.ink, 0.15)
    } else {
        colors.point
    };
    painter.rect_filled(rect, CornerRadius::same(10), fill);

    let content = 16.0 + 6.0 + text.size().x;
    let left = rect.center().x - content / 2.0;
    let icon_rect = Rect::from_center_size(pos2(left + 8.0, rect.center().y), Vec2::splat(16.0));
    paint(&painter, icon_rect, icon, colors.on_point, fill);
    let text_pos = pos2(left + 22.0, rect.center().y - text.size().y / 2.0);
    painter.galley(text_pos, text, colors.on_point);

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
            // 필름: 몸체에 양옆의 구멍과 가운데 칸 구분선을 배경색으로 파내기
            block(0.02, 0.04, 0.98, 0.96, 2);
            let cut_out = |x1: f32, y1: f32, x2: f32, y2: f32| {
                painter.rect_filled(
                    Rect::from_min_max(at(x1, y1), at(x2, y2)),
                    CornerRadius::ZERO,
                    background,
                );
            };
            for y in [0.13, 0.42, 0.71] {
                cut_out(0.09, y, 0.21, y + 0.16);
                cut_out(0.79, y, 0.91, y + 0.16);
            }
            cut_out(0.30, 0.45, 0.70, 0.55);
        }
    }
}
