// 재생 영역의 꾸밈: 위에서 번지는 빛이 있는 바탕과 가는 slider

use crate::theme::{self, Palette};
use eframe::egui::{
    Color32, CornerRadius, EventFilter, Key, Mesh, Pos2, Rect, Response, Role, Sense, Shape,
    Stroke, StrokeKind, Ui, Vec2, WidgetInfo, pos2, vec2,
};

// 모서리를 둥글게 하는 반지름
const RADIUS: f32 = 20.0;

// 카드의 바탕색. 위쪽 가운데는 강조색이 비치고 멀어질수록 평범한 바탕색
#[derive(Clone, Copy)]
pub struct Glow {
    rect: Rect,
    near: Color32,
    far: Color32,
}

impl Glow {
    pub fn new(rect: Rect, colors: Palette) -> Self {
        Self {
            rect,
            near: colors.tint(0.26),
            far: colors.shade(0.07),
        }
    }

    // point 위치의 바탕색. 가운데 위(카드 높이의 20% 위)에서 타원 모양으로 번짐
    pub fn color_at(&self, point: Pos2) -> Color32 {
        let size = self.rect.size();
        let center = pos2(self.rect.center().x, self.rect.top() - size.y * 0.2);
        let x = (point.x - center.x) / (size.x * 1.2);
        let y = (point.y - center.y) / (size.y * 1.4);
        let distance = (x * x + y * y).sqrt();
        let amount = (distance / 0.62).clamp(0.0, 1.0);
        self.near.lerp_to_gamma(self.far, amount)
    }

    // 둥근 카드 모양의 바탕과 테두리
    pub fn shape(&self, border: Color32) -> Shape {
        const ROWS: usize = 40;
        const COLUMNS: usize = 24;
        let rect = self.rect;
        let radius = RADIUS.min(rect.height() / 2.0);

        // 가로 줄마다 점을 놓아 삼각형으로 잇기
        let mut mesh = Mesh::default();
        for row in 0..=ROWS {
            // 둥근 모서리가 있는 위아래 끝에는 줄을 촘촘하게
            let t = row as f32 / ROWS as f32;
            let eased = (1.0 - (t * std::f32::consts::PI).cos()) / 2.0;
            let y = rect.top() + rect.height() * eased;
            let from_edge = (y - rect.top()).min(rect.bottom() - y);
            let inset = if from_edge < radius {
                let height = radius - from_edge;
                radius - (radius * radius - height * height).max(0.0).sqrt()
            } else {
                0.0
            };
            let width = rect.width() - inset * 2.0;
            for column in 0..=COLUMNS {
                let x = rect.left() + inset + width * column as f32 / COLUMNS as f32;
                mesh.colored_vertex(pos2(x, y), self.color_at(pos2(x, y)));
            }
        }
        let stride = (COLUMNS + 1) as u32;
        for row in 0..ROWS as u32 {
            for column in 0..COLUMNS as u32 {
                let first = row * stride + column;
                mesh.add_triangle(first, first + 1, first + stride);
                mesh.add_triangle(first + 1, first + stride + 1, first + stride);
            }
        }

        // 테두리가 바탕의 가장자리를 매끈하게 덮음
        let outline = Shape::rect_stroke(
            rect,
            CornerRadius::same(radius as u8),
            Stroke::new(1.0, border),
            StrokeKind::Inside,
        );
        Shape::Vec(vec![Shape::mesh(mesh), outline])
    }
}

// 가는 막대와 둥근 손잡이의 slider. 누르거나 끌어서, 또는 방향키로 값을 바꿈.
// vertical이면 아래가 0
pub fn slider(
    ui: &mut Ui,
    value: &mut f32,
    max: f32,
    length: f32,
    vertical: bool,
    label: &str,
) -> Response {
    const THICKNESS: f32 = 14.0;
    const KNOB: f32 = 5.5;
    let size = if vertical {
        vec2(THICKNESS, length)
    } else {
        vec2(length, THICKNESS)
    };
    let (rect, mut response) = ui.allocate_exact_size(size, Sense::click_and_drag());

    // 손잡이가 끝에서 잘리지 않는 범위
    let (start, end) = if vertical {
        (
            pos2(rect.center().x, rect.bottom() - KNOB),
            pos2(rect.center().x, rect.top() + KNOB),
        )
    } else {
        (
            pos2(rect.left() + KNOB, rect.center().y),
            pos2(rect.right() - KNOB, rect.center().y),
        )
    };

    let mut new = *value;
    if (response.dragged() || response.clicked() || response.drag_stopped())
        && let Some(pointer) = response.interact_pointer_pos()
    {
        let along = end - start;
        let ratio = (pointer - start).dot(along) / along.length_sq().max(1.0);
        new = ratio.clamp(0.0, 1.0) * max;
    }
    if response.has_focus() {
        // 방향키를 다른 곳으로 이동하는 데 쓰지 않고 값 바꾸기에 사용
        ui.memory_mut(|memory| {
            let filter = EventFilter {
                horizontal_arrows: !vertical,
                vertical_arrows: vertical,
                ..Default::default()
            };
            memory.set_focus_lock_filter(response.id, filter);
        });
        let (less, more) = if vertical {
            (Key::ArrowDown, Key::ArrowUp)
        } else {
            (Key::ArrowLeft, Key::ArrowRight)
        };
        let steps = ui.input(|i| i.num_presses(more) as f32 - i.num_presses(less) as f32);
        new = (new + steps * max * 0.05).clamp(0.0, max);
    }
    if new != *value {
        *value = new;
        response.mark_changed();
    }
    response.widget_info(|| WidgetInfo::labeled(Role::Slider, ui.is_enabled(), label));

    if ui.is_rect_visible(rect) {
        let colors = theme::palette(ui);
        let ratio = if max > 0.0 { *value / max } else { 0.0 };
        let knob = start + (end - start) * ratio.clamp(0.0, 1.0);
        let rail = |from: Pos2, to: Pos2, color: Color32| {
            let thick = Vec2::splat(2.0);
            let area = Rect::from_two_pos(from, to).expand2(thick);
            ui.painter().rect_filled(area, CornerRadius::same(2), color);
        };
        rail(start, end, colors.veil(0.14));
        if ratio > 0.0 {
            rail(start, knob, colors.point);
        }
        ui.painter().circle_filled(knob, KNOB, colors.ink);
    }
    response
}
