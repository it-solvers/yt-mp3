// 직접 그리는 아이콘 버튼.
// 이미지 파일 없이 선과 도형으로 그려서, 어떤 크기에서도 선명하고 화면 색에 맞춰짐

use eframe::egui::{
    Color32, CornerRadius, Painter, Pos2, Rect, Response, Role, Sense, Shape, Stroke, StrokeKind,
    Ui, Vec2, WidgetInfo, pos2, vec2,
};

#[derive(Clone, Copy)]
pub enum Icon {
    Play,
    Pause,
    Stop,
    Refresh,
    Trash,
    Folder,
    Help,
    Download,
    Speaker,
    // 바깥 앱에서 열기 (동영상)
    Video,
}

// 아이콘 하나가 들어가는 버튼. label은 마우스를 올렸을 때 보여줄 설명
pub fn button(ui: &mut Ui, icon: Icon, label: String) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::click());
    describe(&response, ui, &label);

    if ui.is_rect_visible(rect) {
        let visuals = *ui.style().interact(&response);
        ui.painter().rect(
            rect.expand(visuals.expansion),
            visuals.corner_radius,
            visuals.weak_bg_fill,
            visuals.bg_stroke,
            StrokeKind::Inside,
        );
        paint(
            ui.painter(),
            rect.shrink(7.0),
            icon,
            visuals.fg_stroke.color,
        );
    }

    response.on_hover_text(label)
}

// 설명용 그림 (누를 수 없음)
pub fn picture(ui: &mut Ui, icon: Icon, label: String) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::hover());
    let color = ui.visuals().weak_text_color();
    paint(ui.painter(), rect.shrink(1.0), icon, color);
    response.on_hover_text(label)
}

// 아이콘과 글자가 함께 있는 강조 버튼
pub fn primary_button(
    ui: &mut Ui,
    icon: Icon,
    label: String,
    fill: Color32,
    enabled: bool,
) -> Response {
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
        fill.gamma_multiply(1.2).to_opaque()
    } else {
        fill
    };
    painter.rect_filled(rect, CornerRadius::same(6), fill);

    let content = 16.0 + 6.0 + text.size().x;
    let left = rect.center().x - content / 2.0;
    let icon_rect = Rect::from_center_size(pos2(left + 8.0, rect.center().y), Vec2::splat(16.0));
    paint(&painter, icon_rect, icon, Color32::WHITE);
    let text_pos = pos2(left + 22.0, rect.center().y - text.size().y / 2.0);
    painter.galley(text_pos, text, Color32::WHITE);

    response
}

// 화면 읽기 프로그램이 버튼의 이름을 알 수 있게 하기
fn describe(response: &Response, ui: &Ui, label: &str) {
    response.widget_info(|| WidgetInfo::labeled(Role::Button, ui.is_enabled(), label));
}

// rect 안에 아이콘 그리기. 좌표는 0.0 ~ 1.0 비율로 지정
fn paint(painter: &Painter, rect: Rect, icon: Icon, color: Color32) {
    let at = |x: f32, y: f32| rect.min + vec2(x * rect.width(), y * rect.height());
    let stroke = Stroke::new((rect.width() * 0.11).max(1.3), color);
    let fill = |points: Vec<Pos2>| {
        painter.add(Shape::convex_polygon(points, color, Stroke::NONE));
    };
    let line = |points: Vec<Pos2>| {
        painter.add(Shape::line(points, stroke));
    };
    let block = |x1: f32, y1: f32, x2: f32, y2: f32| {
        painter.rect_filled(
            Rect::from_min_max(at(x1, y1), at(x2, y2)),
            CornerRadius::same(1),
            color,
        );
    };
    let outline = |x1: f32, y1: f32, x2: f32, y2: f32| {
        painter.rect_stroke(
            Rect::from_min_max(at(x1, y1), at(x2, y2)),
            CornerRadius::same(2),
            stroke,
            StrokeKind::Middle,
        );
    };

    match icon {
        Icon::Play => fill(vec![at(0.2, 0.05), at(0.95, 0.5), at(0.2, 0.95)]),
        Icon::Pause => {
            block(0.17, 0.08, 0.40, 0.92);
            block(0.60, 0.08, 0.83, 0.92);
        }
        Icon::Stop => block(0.14, 0.14, 0.86, 0.86),
        Icon::Refresh => {
            // 오른쪽 위가 열린 원과 화살촉
            let center = rect.center();
            let radius = rect.width() * 0.40;
            let point = |degrees: f32| {
                let angle = degrees.to_radians();
                center + radius * vec2(angle.cos(), angle.sin())
            };
            let arc = (0..=24).map(|i| point(i as f32 * 290.0 / 24.0)).collect();
            line(arc);
            let tip = point(290.0);
            let size = rect.width() * 0.24;
            // 원이 진행하는 방향과 그에 수직인 방향
            let angle = 290.0_f32.to_radians();
            let forward = vec2(-angle.sin(), angle.cos());
            let side = vec2(angle.cos(), angle.sin());
            fill(vec![
                tip + forward * size,
                tip + side * size * 0.9,
                tip - side * size * 0.9,
            ]);
        }
        Icon::Trash => {
            line(vec![at(0.05, 0.22), at(0.95, 0.22)]);
            line(vec![
                at(0.35, 0.22),
                at(0.37, 0.05),
                at(0.63, 0.05),
                at(0.65, 0.22),
            ]);
            line(vec![
                at(0.18, 0.25),
                at(0.24, 0.95),
                at(0.76, 0.95),
                at(0.82, 0.25),
            ]);
            line(vec![at(0.40, 0.42), at(0.41, 0.77)]);
            line(vec![at(0.60, 0.42), at(0.59, 0.77)]);
        }
        Icon::Folder => {
            painter.add(Shape::closed_line(
                vec![
                    at(0.04, 0.14),
                    at(0.38, 0.14),
                    at(0.50, 0.28),
                    at(0.96, 0.28),
                    at(0.96, 0.86),
                    at(0.04, 0.86),
                ],
                stroke,
            ));
        }
        Icon::Help => {
            painter.circle_stroke(rect.center(), rect.width() * 0.50, stroke);
            line(vec![
                at(0.35, 0.38),
                at(0.38, 0.28),
                at(0.50, 0.23),
                at(0.62, 0.28),
                at(0.65, 0.38),
                at(0.60, 0.46),
                at(0.50, 0.53),
                at(0.50, 0.62),
            ]);
            painter.circle_filled(at(0.50, 0.76), stroke.width * 0.8, color);
        }
        Icon::Download => {
            line(vec![at(0.50, 0.02), at(0.50, 0.58)]);
            line(vec![at(0.26, 0.38), at(0.50, 0.62), at(0.74, 0.38)]);
            line(vec![
                at(0.08, 0.68),
                at(0.08, 0.94),
                at(0.92, 0.94),
                at(0.92, 0.68),
            ]);
        }
        Icon::Speaker => {
            block(0.02, 0.32, 0.24, 0.68);
            fill(vec![
                at(0.22, 0.32),
                at(0.52, 0.08),
                at(0.52, 0.92),
                at(0.22, 0.68),
            ]);
            line(vec![at(0.68, 0.34), at(0.76, 0.50), at(0.68, 0.66)]);
            line(vec![at(0.82, 0.18), at(0.96, 0.50), at(0.82, 0.82)]);
        }
        Icon::Video => {
            outline(0.02, 0.14, 0.98, 0.86);
            fill(vec![at(0.38, 0.32), at(0.68, 0.50), at(0.38, 0.68)]);
        }
    }
}
