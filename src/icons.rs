// 직접 그리는 아이콘.
// 색은 theme.rs의 3색만 사용. 저장과 재생 중인 버튼만 강조색.
// 이미지 파일 없이 도형으로 그려서 어떤 크기에서도 선명함

use crate::theme::palette;
use eframe::egui::{
    Color32, CornerRadius, CursorIcon, Painter, Pos2, Rect, Response, Role, Sense, Shape, Stroke,
    Ui, Vec2, WidgetInfo, pos2, vec2,
};

#[derive(Clone, Copy)]
pub enum Icon {
    Play,
    Pause,
    // 이전 곡, 다음 곡
    Previous,
    Next,
    // 반복 (전체, 한 곡)과 섞기
    Repeat,
    RepeatOne,
    Shuffle,
    Folder,
    Download,
    Speaker,
    // 음소거
    Muted,
    // 프로그램 정보
    Info,
    // 언어 바꾸기
    Globe,
    // 화면 색상
    Palette,
    // 목록 편집
    Edit,
    // 음표
    Music,
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
        paint(ui.painter(), rect.shrink(7.0), icon, glyph);
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

// 재생 영역의 큰 원형 버튼. rect 위치에 그림. 재생 중이면 강조색
pub fn round_button(ui: &mut Ui, rect: Rect, icon: Icon, label: String) -> Response {
    let response = place(ui, rect, &label);
    let colors = palette(ui);
    let hovered = response.hovered();
    let (fill, glyph) = if matches!(icon, Icon::Pause) {
        let amount = if hovered { 0.15 } else { 0.0 };
        (
            colors.point.lerp_to_gamma(colors.ink, amount),
            colors.on_point,
        )
    } else {
        let amount = if hovered { 0.22 } else { 0.13 };
        (colors.veil(amount), colors.ink)
    };
    ui.painter()
        .circle_filled(rect.center(), rect.width() / 2.0, fill);
    paint(ui.painter(), rect.shrink(rect.width() * 0.31), icon, glyph);
    response.on_hover_text(label)
}

// 배경 없는 버튼. 마우스를 올리면 옅은 원이 보임. 켜진 기능은 강조색
pub fn flat_button(ui: &mut Ui, rect: Rect, icon: Icon, on: bool, label: String) -> Response {
    let response = place(ui, rect, &label);
    let colors = palette(ui);
    if response.hovered() {
        ui.painter()
            .circle_filled(rect.center(), rect.width() / 2.0, colors.veil(0.12));
    }
    let color = match icon {
        // 이전, 다음 곡은 밝게. 반복과 섞기는 꺼져 있으면 흐리게
        Icon::Previous | Icon::Next => colors.ink,
        _ if on => colors.point,
        _ => colors.veil(0.45),
    };
    paint(ui.painter(), rect.shrink(rect.width() * 0.22), icon, color);
    response.on_hover_text(label)
}

// 정해진 위치의 버튼 만들기. 비활성화된 ui에서는 눌리지 않고 흐리게 그려짐
fn place(ui: &mut Ui, rect: Rect, label: &str) -> Response {
    let id = ui.scope_id().with(("player", rect.center().x as i32));
    let response = ui.interact(rect, id, Sense::click());
    describe(&response, ui, label);
    response
}

// 편집 모드에서 줄 앞에 보여주는 선택 표시 (누르는 것은 줄 전체)
pub fn check_mark(ui: &mut Ui, checked: bool) {
    let (rect, _) = ui.allocate_exact_size(vec2(22.0, 28.0), Sense::hover());
    let colors = palette(ui);
    let center = rect.center();
    if checked {
        ui.painter().circle_filled(center, 10.0, colors.point);
        let check = [vec2(-4.5, 0.0), vec2(-1.5, 3.5), vec2(5.0, -3.5)];
        let points = check.iter().map(|&offset| center + offset).collect();
        let line = Stroke::new(2.0, colors.on_point);
        ui.painter().add(Shape::line(points, line));
    } else {
        let line = Stroke::new(1.5, colors.shade(0.45));
        ui.painter().circle_stroke(center, 9.0, line);
    }
}

// 편집 모드에서 줄 끝에 보여주는 손잡이. 줄을 끌어서 옮길 수 있다는 표시
pub fn drag_handle(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
    let color = palette(ui).shade(0.55);
    for offset in [-5.0, 0.0, 5.0] {
        let y = rect.center().y + offset;
        let line = [
            pos2(rect.center().x - 8.0, y),
            pos2(rect.center().x + 8.0, y),
        ];
        ui.painter().line_segment(line, Stroke::new(2.0, color));
    }
}

// 배경 없는 작은 버튼. 마우스를 올리면 옅은 원이 보임
pub fn ghost_button(ui: &mut Ui, icon: Icon, label: String) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::click());
    describe(&response, ui, &label);

    if ui.is_rect_visible(rect) {
        let colors = palette(ui);
        let color = if response.hovered() {
            ui.painter()
                .circle_filled(rect.center(), rect.width() / 2.0, colors.veil(0.12));
            colors.ink
        } else {
            colors.shade(0.55)
        };
        paint(ui.painter(), rect.shrink(6.0), icon, color);
    }

    response
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text(label)
}

// 다른 버튼 안에 넣는 그림. 설명이나 반응 없이 글자색으로 그림
pub fn symbol(ui: &mut Ui, icon: Icon) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::hover());
    let color = palette(ui).ink;
    paint(ui.painter(), rect.shrink(1.0), icon, color);
}

// 설명용 그림 (누를 수 없음). 배경 없이 그려서 버튼과 구별
pub fn picture(ui: &mut Ui, icon: Icon, label: String) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
    if ui.is_rect_visible(rect) {
        let colors = palette(ui);
        paint(ui.painter(), rect.shrink(6.0), icon, colors.shade(0.55));
    }
    response.on_hover_text(label)
}

// 아이콘과 글자가 함께 있는 강조 버튼
pub fn primary_button(
    ui: &mut Ui,
    icon: Icon,
    label: String,
    enabled: bool,
    width: f32,
) -> Response {
    let enabled = enabled && ui.is_enabled();
    let colors = palette(ui);
    let font = eframe::egui::TextStyle::Button.resolve(ui.style());
    let text = ui
        .painter()
        .layout_no_wrap(label.clone(), font, colors.on_point);

    let size = vec2(width, 38.0);
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
    paint(&painter, icon_rect, icon, colors.on_point);
    let text_pos = pos2(left + 22.0, rect.center().y - text.size().y / 2.0);
    painter.galley(text_pos, text, colors.on_point);

    response
}

// 화면 읽기 프로그램이 버튼의 이름을 알 수 있게 하기
fn describe(response: &Response, ui: &Ui, label: &str) {
    response.widget_info(|| WidgetInfo::labeled(Role::Button, ui.is_enabled(), label));
}

// rect 안에 채운 모양의 아이콘 그리기. 좌표는 0.0 ~ 1.0 비율로 지정.
fn paint(painter: &Painter, rect: Rect, icon: Icon, color: Color32) {
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
        Icon::Previous => {
            for x in [0.0, 0.50] {
                fill(
                    vec![at(x, 0.50), at(x + 0.50, 0.14), at(x + 0.50, 0.86)],
                    color,
                );
            }
        }
        Icon::Next => {
            for x in [0.0, 0.50] {
                fill(vec![at(x, 0.14), at(x + 0.50, 0.50), at(x, 0.86)], color);
            }
        }
        Icon::Repeat | Icon::RepeatOne => {
            // 위와 아래의 꺾인 선과 화살촉이 순환하는 모양
            let line = Stroke::new(rect.width() * 0.11, color);
            let top = vec![at(0.08, 0.52), at(0.08, 0.24), at(0.78, 0.24)];
            let bottom = vec![at(0.92, 0.48), at(0.92, 0.76), at(0.22, 0.76)];
            painter.add(Shape::line(top, line));
            painter.add(Shape::line(bottom, line));
            fill(vec![at(0.74, 0.05), at(1.00, 0.24), at(0.74, 0.43)], color);
            fill(vec![at(0.26, 0.57), at(0.00, 0.76), at(0.26, 0.95)], color);
            if matches!(icon, Icon::RepeatOne) {
                // 가운데의 숫자 1
                let one = vec![at(0.43, 0.42), at(0.53, 0.37), at(0.53, 0.65)];
                painter.add(Shape::line(one, line));
            }
        }
        Icon::Shuffle => {
            // 서로 엇갈리는 두 선과 오른쪽 화살촉
            let line = Stroke::new(rect.width() * 0.11, color);
            for (from, to) in [(0.78, 0.22), (0.22, 0.78)] {
                let path = vec![at(0.02, from), at(0.24, from), at(0.58, to), at(0.78, to)];
                painter.add(Shape::line(path, line));
                fill(
                    vec![at(0.74, to - 0.19), at(1.00, to), at(0.74, to + 0.19)],
                    color,
                );
            }
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
        Icon::Muted => {
            // 스피커와 × 표시
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
            let line = Stroke::new(rect.width() * 0.12, color);
            painter.line_segment([at(0.70, 0.35), at(1.00, 0.65)], line);
            painter.line_segment([at(1.00, 0.35), at(0.70, 0.65)], line);
        }
        Icon::Info => {
            // 원 안의 i
            let width = rect.width() * 0.13;
            painter.circle_stroke(
                rect.center(),
                rect.width() * 0.50,
                Stroke::new(width, color),
            );
            painter.circle_filled(at(0.50, 0.27), rect.width() * 0.10, color);
            block(0.42, 0.43, 0.58, 0.78, 1);
        }
        Icon::Globe => {
            // 바깥 원, 가운데의 세로 타원(경선), 가로선(위선)
            let line = Stroke::new(rect.width() * 0.10, color);
            painter.circle_stroke(rect.center(), rect.width() * 0.50, line);
            let meridian = (0..=32).map(|i| {
                let angle = (i as f32 / 32.0) * std::f32::consts::TAU;
                rect.center() + rect.width() * vec2(0.22 * angle.cos(), 0.50 * angle.sin())
            });
            painter.add(Shape::line(meridian.collect(), line));
            painter.line_segment([at(0.02, 0.50), at(0.98, 0.50)], line);
        }
        Icon::Edit => {
            // 비스듬한 연필
            fill(
                vec![
                    at(0.68, 0.02),
                    at(0.98, 0.32),
                    at(0.40, 0.90),
                    at(0.10, 0.60),
                ],
                color,
            );
            fill(vec![at(0.07, 0.68), at(0.32, 0.93), at(0.00, 1.00)], color);
        }
        Icon::Palette => {
            // 팔레트: 큰 원과 물감 세 곳
            let line = Stroke::new(rect.width() * 0.10, color);
            painter.circle_stroke(rect.center(), rect.width() * 0.50, line);
            for (x, y) in [(0.30, 0.38), (0.55, 0.24), (0.76, 0.42)] {
                painter.circle_filled(at(x, y), rect.width() * 0.09, color);
            }
            painter.circle_filled(at(0.58, 0.72), rect.width() * 0.15, color);
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
    }
}
