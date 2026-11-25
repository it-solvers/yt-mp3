// Windows 배포용 실행 파일은 뒤에 콘솔 창이 뜨지 않도록 설정
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

// tr! 매크로를 아래 코드에서 쓰려면 가장 먼저 선언해야 함
#[macro_use]
mod lang;

mod icons;
mod midi;
mod theme;

use eframe::egui;
use icons::Icon;
use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player, Source};
use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::Duration;

fn main() -> eframe::Result {
    // 처음에는 시스템 언어를 따름. 사용자가 고른 언어가 있으면 App::new에서 바꿈
    lang::set_korean(lang::system_is_korean());

    // 창과 Dock에 보일 아이콘. 지정하지 않으면 eframe 기본 아이콘이 쓰임
    let icon = app_icon();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            // 설정이 저장되는 폴더 이름으로도 쓰임
            .with_app_id("yt-mp3")
            .with_inner_size([560.0, 560.0])
            // 화면의 요소들이 겹치지 않는 최소 크기
            .with_min_inner_size([360.0, 480.0])
            .with_icon(icon),
        ..Default::default()
    };

    eframe::run_native(
        &app_name(),
        options,
        Box::new(|cc| {
            let app = App::new(cc.storage);
            theme::apply(&cc.egui_ctx, app.colors);
            // 저장해 둔 언어에 맞게 창 제목 다시 설정
            cc.egui_ctx
                .send_viewport_cmd(egui::ViewportCommand::Title(app_name()));
            Ok(Box::new(app))
        }),
    )
}

// 프로그램에 포함한 앱 아이콘
fn app_icon() -> egui::IconData {
    eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png"))
        .expect("프로그램에 포함한 아이콘은 올바른 PNG")
}

fn app_name() -> String {
    tr!("걍담아 플레이어", "JustGrab Player")
}

// 정보창에 보여줄 저작권 정보
fn copyright() -> String {
    tr!(
        "© 2026 아이티해결단(이혁). All rights reserved.",
        "© 2026 ITSolvers(Hyeok Lee). All rights reserved.",
    )
}

// 지원 사이트 안내창의 내용. 다운로드를 확인한 사이트만 적기
const SITES: &str = "YouTube, Instagram, TikTok, X, Facebook, SoundCloud";

// 지원 사이트 목록. notes가 true면 주의할 점도 함께 보여줌
fn supported_sites(ui: &mut egui::Ui, notes: bool) {
    let strong = ui.visuals().strong_text_color();

    ui.label(
        egui::RichText::new(tr!("지원 사이트", "Supported sites"))
            .size(15.0)
            .strong()
            .color(ui.visuals().hyperlink_color),
    );
    ui.label(egui::RichText::new(SITES).color(strong));
    if notes {
        ui.label(
            egui::RichText::new(tr!(
                "• SoundCloud는 소리만 받을 수 있어요\n\
                 • 게시물에 따라 받지 못할 수 있어요\n\
                 • 로그인, 비공개, DRM 콘텐츠는 안 돼요",
                "• SoundCloud provides audio only\n\
                 • Some posts may fail to download\n\
                 • No login-only, private, or DRM content",
            ))
            .color(strong),
        );
    }
}

// 잠시 보였다가 사라지는 알림
struct Toast {
    title: Option<String>,
    text: String,
    // 저장 위치처럼 작게 덧붙이는 글
    place: Option<String>,
    // 세기 시작한 시각. 아직 그리지 않았으면 None
    since: Option<f64>,
}

impl Toast {
    // 흐려지며 사라지는 데 걸리는 시간(초)
    const FADE: f32 = 0.5;

    fn new(text: String) -> Self {
        Self {
            title: None,
            text,
            place: None,
            since: None,
        }
    }

    // 저장 완료: 받은 파일의 이름과 위치
    fn saved(files: &[PathBuf], folder: &Path) -> Self {
        let names: Vec<String> = files.iter().map(|path| file_name(path)).collect();
        // 파일 이름을 알 수 없으면 다운로드 폴더
        let place = files
            .first()
            .map_or_else(|| folder.display().to_string(), |path| location(path));
        Self {
            title: Some(tr!("저장 완료!", "Saved!")),
            text: names.join("\n"),
            place: Some(tr!("위치: {place}", "Location: {place}")),
            since: None,
        }
    }

    // 흐려지기 전까지 보여주는 시간(초). 글이 길수록 오래 (2~5초)
    fn seconds(&self) -> f32 {
        (self.text.chars().count() as f32 / 14.0).clamp(2.0, 5.0)
    }
}

// 목록에서 끌고 있는 줄
struct Drag {
    path: PathBuf,
    // 줄의 위쪽에서 마우스로 잡은 곳까지의 거리
    grab: f32,
    // 줄의 높이
    height: f32,
}

// 다운로드에 실패했을 때 팝업에 보여줄 내용
struct Failure {
    reason: String,
    // 오류 내용을 복사했는지
    copied: bool,
}

// 재생 영역의 내용 높이 (제목 20 + 곡 정보 28 + 재생 버튼 52 + 탐색 bar 18 + 간격)
const PLAYER_HEIGHT: f32 = 127.0;

// 화면이 기억해야 하는 값들
struct App {
    url: String,
    folder: PathBuf,
    status: String,
    // 결과나 오류를 알리는 알림이 떠 있으면 Some
    toast: Option<Toast>,
    // 다운로드 실패 팝업이 열려 있으면 Some
    failure: Option<Failure>,
    // 고른 화면 색
    colors: theme::Choice,
    // 색상 선택 팝업이 열려 있는지
    choosing_colors: bool,
    // 프로그램 정보 팝업이 열려 있는지
    showing_about: bool,
    // 다운로드 중이면 Some, 아니면 None
    job: Option<Job>,
    // 재생목록. 파일은 원래 위치에 두고 경로만 기억
    files: Vec<PathBuf>,
    // 재생목록 중에서 지금은 찾을 수 없는 파일
    missing: HashSet<PathBuf>,
    // 재생 중이면 Some, 아니면 None
    playing: Option<Playing>,
    // 편집 중이면 Some(고른 파일), 아니면 None
    selected: Option<HashSet<PathBuf>>,
    // 직전에 창이 선택되어 있었는지. 다시 선택된 순간을 알아내는 데 사용
    focused: bool,
    // 편집 중에 끌고 있는 줄
    dragging: Option<Drag>,
    // 음량 (0.0 ~ 1.0)
    volume: f32,
    // 음소거를 풀 때 돌아갈 음량
    unmuted: f32,
    repeat: Repeat,
    // 다음 곡을 무작위로 고를지
    shuffle: bool,
    // 섞기 중에 지나온 곡. 이전 곡으로 돌아가고 같은 곡을 다시 고르지 않는 데 사용
    history: Vec<PathBuf>,
    // 섞기 중에 이전 곡으로 돌아온 뒤, 다음 곡으로 다시 갈 곡
    upcoming: Vec<PathBuf>,
    // 다운로드할 때 저장할 형식
    format: Format,
    // 정보창에 보여줄 앱 아이콘. 처음 열 때 만듦
    icon_texture: Option<egui::TextureHandle>,
    // 사용자가 언어를 직접 골랐는지. 고르지 않았으면 시스템 언어를 계속 따름
    language_chosen: bool,
}

// 반복 방식
#[derive(Clone, Copy, PartialEq, Debug)]
enum Repeat {
    Off,
    // 목록 전체
    All,
    // 한 곡
    One,
}

// 저장할 형식
#[derive(Clone, Copy, PartialEq, Debug)]
enum Format {
    // 소리만
    Mp3,
    // 동영상
    Mp4,
}

// 진행 중인 다운로드
struct Job {
    receiver: Receiver<Update>,
    format: Format,
    // 받은 비율 (0.0 ~ 1.0). 아직 모르면 None
    progress: Option<f32>,
}

// 다운로드 thread가 화면에 보내는 소식
enum Update {
    Progress(f32),
    // 다 받은 뒤 ffmpeg로 처리하는 중
    Processing,
    // 성공하면 받은 파일들의 경로
    Done(Result<Vec<PathBuf>, String>),
}

// 재생에 필요한 값들
struct Playing {
    path: PathBuf,
    player: Player,
    // 전체 길이. 알 수 없는 파일이면 None
    total: Option<Duration>,
    // 탐색 bar를 끄는 중이면 그 위치(초)
    seeking: Option<f32>,
    // 소리 출력 장치. 이 값이 사라지면 재생도 멈추므로 함께 보관
    _device: MixerDeviceSink,
}

impl App {
    fn new(storage: Option<&dyn eframe::Storage>) -> Self {
        // 지난번에 저장한 설정 읽기
        let saved = |key: &str| storage.and_then(|s| s.get_string(key));

        // 저장해 둔 폴더가 지금도 있으면 사용하고,
        // 아니면 기본값인 홈 폴더의 Music (홈 폴더를 모르면 현재 폴더)
        let folder = saved("folder")
            .map(PathBuf::from)
            .filter(|folder| folder.is_dir())
            .unwrap_or_else(|| match std::env::home_dir() {
                Some(home) => home.join("Music"),
                None => PathBuf::from("."),
            });
        let volume = saved("volume")
            .and_then(|v| v.parse::<f32>().ok())
            .map_or(1.0, |v| v.clamp(0.0, 1.0));
        let unmuted = saved("unmuted")
            .and_then(|v| v.parse::<f32>().ok())
            .map_or(1.0, |v| v.clamp(0.0, 1.0));
        let repeat = match saved("repeat").as_deref() {
            Some("all") => Repeat::All,
            Some("one") => Repeat::One,
            _ => Repeat::Off,
        };
        let format = match saved("format").as_deref() {
            Some("mp4") => Format::Mp4,
            _ => Format::Mp3,
        };

        let language_chosen = match saved("language").as_deref() {
            Some(language @ ("ko" | "en")) => {
                lang::set_korean(language == "ko");
                true
            }
            _ => false,
        };

        let mut app = Self {
            url: String::new(),
            folder,
            // 보여줄 안내가 생기기 전까지는 비워 두기
            status: String::new(),
            toast: None,
            failure: None,
            colors: theme::Choice {
                preset: saved("color_preset")
                    .and_then(|v| v.parse().ok())
                    .filter(|&i| i < theme::PRESETS.len())
                    .unwrap_or(0),
                custom: ["color_dark", "color_light"]
                    .map(|key| saved(key).and_then(|v| theme::parse_colors(&v))),
            },
            choosing_colors: false,
            showing_about: false,
            job: None,
            files: Vec::new(),
            missing: HashSet::new(),
            playing: None,
            selected: None,
            focused: true,
            dragging: None,
            volume,
            unmuted,
            repeat,
            // 섞기는 저장하지 않고, 켤 때마다 목록 순서대로 시작
            shuffle: false,
            history: Vec::new(),
            upcoming: Vec::new(),
            format,
            language_chosen,
            icon_texture: None,
        };
        app.files = match saved("playlist") {
            Some(text) => parse_playlist(&text),
            // 재생목록이 처음이면 지금 폴더의 파일을 이전에 정한 순서대로 가져오기
            None => {
                let files = list_media(&app.folder).unwrap_or_default();
                let orders = saved("order").map(|v| parse_orders(&v)).unwrap_or_default();
                match orders.get(&app.folder.display().to_string()) {
                    Some(order) => apply_order(files, order),
                    None => files,
                }
            }
        };
        app.check_missing();
        app
    }

    // 화면 상단: 왼쪽에 앱 이름과 정보 버튼, 오른쪽에 색상과 언어 버튼
    fn title_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(app_name()).heading().strong());

            if icons::button(ui, Icon::Info, tr!("프로그램 정보", "About")).clicked() {
                self.showing_about = true;
            }

            let right_to_left = egui::Layout::right_to_left(egui::Align::Center);
            ui.with_layout(right_to_left, |ui| {
                // 누를 때마다 한국어와 영어 바꾸기. 설명은 바뀔 언어로 적기
                let korean = lang::is_korean();
                let hint = if korean {
                    "Switch to English"
                } else {
                    "한국어로 바꾸기"
                };
                if icons::button(ui, Icon::Globe, hint.to_string()).clicked() {
                    self.set_language(ui.ctx(), !korean);
                }
                let colors = tr!("화면 색상", "Colors");
                if icons::button(ui, Icon::Palette, colors).clicked() {
                    self.choosing_colors = true;
                }
            });
        });
    }

    // 프로그램 정보 팝업
    fn about_dialog(&mut self, ui: &mut egui::Ui) {
        if !self.showing_about {
            return;
        }
        let title = tr!("프로그램 정보", "About");
        let closed = dialog(ui, "about", &title, |ui| self.about(ui), |_| {});
        if closed {
            self.showing_about = false;
        }
    }

    // 정보 팝업의 내용: 아이콘, 이름, version, 지원 사이트, 저작권, 오픈소스 license
    fn about(&mut self, ui: &mut egui::Ui) {
        let texture = self.icon_texture.get_or_insert_with(|| {
            let icon = app_icon();
            let size = [icon.width as usize, icon.height as usize];
            let image = egui::ColorImage::from_rgba_unmultiplied(size, &icon.rgba);
            // 크게 만든 그림을 작게 보여줘도 거칠지 않게 mipmap 사용
            let options = egui::TextureOptions {
                mipmap_mode: Some(egui::TextureFilter::Linear),
                ..egui::TextureOptions::LINEAR
            };
            ui.ctx().load_texture("app-icon", image, options)
        });
        let icon_id = texture.id();

        ui.vertical_centered(|ui| {
            ui.add(egui::Image::new((icon_id, egui::Vec2::splat(64.0))));
            let strong = ui.visuals().strong_text_color();
            ui.label(
                egui::RichText::new(app_name())
                    .heading()
                    .strong()
                    .color(strong),
            );
            ui.label(egui::RichText::new(concat!("v", env!("CARGO_PKG_VERSION"))).weak());
            ui.separator();
            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                supported_sites(ui, true);
            });
            ui.separator();
            ui.label(egui::RichText::new(copyright()).size(11.0).color(strong));
            ui.separator();
            let licenses = tr!("오픈소스 라이선스", "Open source licenses");
            if ui.link(licenses).clicked() {
                self.open_licenses();
            }
        });
    }

    // 창 아래쪽 가운데의 알림. 잠시 보인 뒤 흐려지며 사라짐.
    // 마우스를 올리면 기다리고, 누르면 바로 닫힘. 뒤의 화면은 계속 사용할 수 있음
    fn show_toast(&mut self, ui: &egui::Ui) {
        let Some(toast) = &mut self.toast else {
            return;
        };
        let ctx = ui.ctx();
        let now = ctx.input(|i| i.time);
        let elapsed = (now - *toast.since.get_or_insert(now)) as f32;
        let remaining = toast.seconds() - elapsed;
        // 시간이 지나면 FADE 동안 1에서 0으로
        let opacity = (1.0 + remaining / Toast::FADE).min(1.0);
        if opacity <= 0.0 {
            self.toast = None;
            return;
        }

        let width = (ctx.content_rect().width() - 64.0).min(332.0);
        let area = egui::Area::new(egui::Id::unique("toast"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -16.0))
            .show(ctx, |ui| {
                ui.multiply_opacity(opacity);
                // 글자를 선택하는 기능이 클릭을 가로채지 않게 끄기
                ui.style_mut().interaction.selectable_labels = false;
                // 뒤의 목록과 구별되도록 강조색을 옅게 깐 바탕
                let colors = theme::palette(ui);
                let frame = popup_frame(ui)
                    .fill(colors.tint(0.28))
                    .stroke(egui::Stroke::new(1.0, colors.tint(0.6)));
                frame.show(ui, |ui| {
                    ui.set_width(width);
                    let strong = ui.visuals().strong_text_color();
                    if let Some(title) = &toast.title {
                        let title = egui::RichText::new(title).strong().color(strong);
                        ui.label(title);
                    }
                    let text = egui::RichText::new(&toast.text).color(strong);
                    ui.add(egui::Label::new(text).wrap());
                    if let Some(place) = &toast.place {
                        // 색이 있는 바탕에서도 읽히도록 글자색을 조금만 흐리게
                        let place = egui::RichText::new(place)
                            .size(11.0)
                            .color(colors.veil(0.65));
                        ui.add(egui::Label::new(place).wrap());
                    }
                });
            });

        let response = area.response.interact(egui::Sense::click());
        if response.clicked() {
            self.toast = None;
        } else if response.contains_pointer() {
            // 마우스를 올려 둔 동안은 처음부터 다시 세기
            toast.since = Some(now);
        } else if remaining > 0.0 {
            ctx.request_repaint_after(Duration::from_secs_f32(remaining));
        } else {
            // 흐려지는 동안은 계속 그리기
            ctx.request_repaint();
        }
    }

    // 다운로드 실패 팝업: 오류 내용과 지원 사이트
    fn failure_dialog(&mut self, ui: &mut egui::Ui) {
        let Some(failure) = &mut self.failure else {
            return;
        };
        let title = tr!("다운로드에 실패했어요", "Download failed");
        // 본문과 아래 버튼이 함께 쓰는 값
        let reason = failure.reason.clone();
        let closed = dialog(
            ui,
            "failure",
            &title,
            |ui| {
                // 긴 오류는 작은 영역 안에서 스크롤
                egui::Frame::new()
                    .fill(theme::palette(ui).shade(0.14))
                    .corner_radius(8)
                    .inner_margin(8)
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        egui::ScrollArea::vertical()
                            .id_salt("reason")
                            .max_height(80.0)
                            .show(ui, |ui| {
                                let text = egui::RichText::new(&reason).monospace().size(11.0);
                                ui.add(egui::Label::new(text).wrap());
                            });
                    });

                ui.separator();
                supported_sites(ui, false);
            },
            |ui| {
                let copy = if failure.copied {
                    tr!("복사했어요", "Copied")
                } else {
                    tr!("오류 내용 복사", "Copy error")
                };
                if ui.button(copy).clicked() {
                    ui.ctx().copy_text(failure.reason.clone());
                    failure.copied = true;
                }
            },
        );
        if closed {
            self.failure = None;
        }
    }

    // 색상 선택 팝업: 미리 만든 조합 고르기와 직접 고르기
    fn color_dialog(&mut self, ui: &mut egui::Ui) {
        if !self.choosing_colors {
            return;
        }
        // 직접 고른 색은 지금 보고 있는 화면 모드에 적용
        let mode = usize::from(!ui.visuals().dark_mode);
        let before = self.colors;
        let choice = std::cell::RefCell::new(self.colors);

        let title = tr!("화면 색상", "Colors");
        let body = |ui: &mut egui::Ui| {
            let mut choice = choice.borrow_mut();
            let names = [
                tr!("하늘", "Sky"),
                tr!("산호", "Coral"),
                tr!("연보라", "Lavender"),
            ];
            ui.columns(names.len(), |columns| {
                for (index, (ui, name)) in columns.iter_mut().zip(names).enumerate() {
                    ui.vertical_centered(|ui| {
                        let selected = choice.custom[mode].is_none() && choice.preset == index;
                        if preset_button(ui, theme::PRESETS[index][mode], &name, selected) {
                            *choice = theme::Choice {
                                preset: index,
                                custom: [None, None],
                            };
                        }
                    });
                }
            });

            ui.separator();
            ui.label(egui::RichText::new(tr!("직접 고르기", "Custom")).weak());
            let mut colors = choice.colors(mode);
            let labels = [
                tr!("바탕색", "Background"),
                tr!("글자색", "Text"),
                tr!("강조색", "Accent"),
            ];
            let mut changed = false;
            for (color, label) in colors.iter_mut().zip(labels) {
                ui.horizontal(|ui| {
                    ui.label(label);
                    let right_to_left = egui::Layout::right_to_left(egui::Align::Center);
                    ui.with_layout(right_to_left, |ui| {
                        let mut rgb = [color.r(), color.g(), color.b()];
                        ui.spacing_mut().interact_size.x = 52.0;
                        if ui.color_edit_button_srgb(&mut rgb).changed() {
                            *color = egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
                            changed = true;
                        }
                    });
                });
            }
            if changed {
                choice.custom[mode] = Some(colors);
            }
        };
        let footer = |ui: &mut egui::Ui| {
            if ui.button(tr!("처음 색으로", "Reset")).clicked() {
                *choice.borrow_mut() = theme::Choice::default();
            }
        };
        let closed = dialog(ui, "colors", &title, body, footer);

        self.colors = choice.into_inner();
        if closed {
            self.choosing_colors = false;
        }
        if self.colors != before {
            theme::apply(ui.ctx(), self.colors);
        }
    }

    fn set_language(&mut self, ctx: &egui::Context, korean: bool) {
        lang::set_korean(korean);
        self.language_chosen = true;
        // 이미 만들어 둔 안내 문구는 이전 언어이므로 지우기
        // (다운로드 중의 상태는 다음 소식을 받을 때 새 언어로 바뀜)
        if self.job.is_none() {
            self.status.clear();
        }
        self.toast = None;
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(app_name()));
    }

    // 재생목록에서 지금은 찾을 수 없는 파일 확인하기
    fn check_missing(&mut self) {
        self.missing = self
            .files
            .iter()
            .filter(|path| !path.is_file())
            .cloned()
            .collect();
    }

    // 재생목록 맨 위에 파일 추가하기. 새로 추가한 개수를 돌려줌
    fn add_files(&mut self, paths: Vec<PathBuf>) -> usize {
        let added = add_to_front(&mut self.files, paths);
        self.check_missing();
        added
    }

    // 창에 끌어다 놓은 파일과 폴더를 재생목록에 추가하기
    fn add_dropped(&mut self, dropped: &[PathBuf]) {
        let found = collect_media(dropped);
        let total = found.len();
        let added = self.add_files(found);
        self.toast = Some(Toast::new(if added > 0 {
            tr!(
                "{added}개를 재생목록에 추가했어요",
                "Added {added} file(s) to the playlist",
            )
        } else if total > 0 {
            tr!("이미 재생목록에 있어요", "Already in the playlist")
        } else {
            tr!(
                "추가할 음악, 영상 파일이 없어요",
                "No music or video files to add",
            )
        }));
    }

    // 파일을 재생목록에서만 빼기. 원본 파일은 그대로 둠
    fn remove_from_list(&mut self, paths: &[PathBuf]) {
        if self
            .playing
            .as_ref()
            .is_some_and(|p| paths.contains(&p.path))
        {
            self.playing = None;
        }
        self.files.retain(|path| !paths.contains(path));
        self.check_missing();
        // 편집 중이면 편집은 계속하고 선택만 비우기
        if let Some(selected) = &mut self.selected {
            selected.clear();
        }
    }

    // 선택한 파일 재생하기 (이전에 재생하던 곡은 자동으로 멈춤)
    fn play(&mut self, path: PathBuf) {
        self.playing = None;
        match start_playing(path) {
            Ok(playing) => {
                playing.player.set_volume(self.volume);
                self.playing = Some(playing);
            }
            Err(message) => {
                self.toast = Some(Toast::new(tr!("오류: {message}", "Error: {message}")))
            }
        }
    }

    // 동영상은 PC의 기본 player로 열기
    fn open_video(&mut self, path: &Path) {
        // 소리가 겹치지 않도록 재생 중인 곡은 일시정지
        if let Some(playing) = &self.playing {
            playing.player.pause();
        }
        if let Err(e) = open::that_detached(path) {
            self.toast = Some(Toast::new(tr!(
                "오류: 동영상을 열 수 없어요: {e}",
                "Error: cannot open the video: {e}"
            )));
        }
    }

    // 재생 중인 곡에 지금 음량 적용
    fn apply_volume(&self) {
        if let Some(playing) = &self.playing {
            playing.player.set_volume(self.volume);
        }
    }

    // 음소거하거나, 음소거 전의 음량으로 되돌리기
    fn toggle_mute(&mut self) {
        if self.volume > 0.0 {
            self.unmuted = self.volume;
            self.volume = 0.0;
        } else if self.unmuted > 0.0 {
            self.volume = self.unmuted;
        } else {
            self.volume = 1.0;
        }
        self.apply_volume();
    }

    // 목록에서 직접 고른 곡 재생하기
    fn choose(&mut self, path: PathBuf) {
        // 섞기 중에는 방금 듣던 곡으로 돌아갈 수 있게 기억
        if self.shuffle
            && let Some(playing) = &self.playing
            && playing.path != path
        {
            self.history.push(playing.path.clone());
            self.upcoming.clear();
        }
        self.play(path);
    }

    // 재생할 수 있는 소리 파일 (동영상과 찾을 수 없는 파일 제외)
    fn songs(&self) -> Vec<PathBuf> {
        self.files
            .iter()
            .filter(|path| !is_video(path) && !self.missing.contains(*path))
            .cloned()
            .collect()
    }

    // current 다음에 재생할 곡. wrap이면 끝까지 간 뒤 처음부터 다시
    fn next_song(&mut self, current: &Path, wrap: bool) -> Option<PathBuf> {
        let songs = self.songs();
        if !self.shuffle {
            return step_song(&songs, current, true, wrap);
        }

        self.history.push(current.to_path_buf());
        // 이전 곡으로 돌아왔었다면 지나갔던 순서대로 다시 앞으로
        if let Some(path) = pop_existing(&mut self.upcoming, &songs) {
            return Some(path);
        }
        let mut candidates = unplayed(&songs, &self.history);
        if candidates.is_empty() && wrap {
            // 모두 들었으면 방금 곡을 빼고 다시 섞기 (방금 곡으로는 돌아갈 수 있음)
            self.history = vec![current.to_path_buf()];
            candidates = unplayed(&songs, &[current.to_path_buf()]);
            if candidates.is_empty() {
                candidates = songs;
            }
        }
        fastrand::choice(candidates)
    }

    // 이전 곡 버튼: 조금이라도 들었으면 처음부터, 곡의 시작 부분이면 이전 곡
    fn previous(&mut self) {
        let Some(playing) = &self.playing else {
            return;
        };
        if playing.player.get_pos() > Duration::from_secs(3) {
            let path = playing.path.clone();
            self.play(path);
            return;
        }

        let current = playing.path.clone();
        let previous = if self.shuffle {
            // 지나온 곡 중에서 지금도 있는 가장 최근 곡
            let songs = self.songs();
            let previous = pop_existing(&mut self.history, &songs);
            if previous.is_some() {
                self.upcoming.push(current.clone());
            }
            previous
        } else {
            step_song(&self.songs(), &current, false, true)
        };
        self.play(previous.unwrap_or(current));
    }

    // 다음 곡 버튼
    fn skip(&mut self) {
        let Some(current) = self.playing.as_ref().map(|p| p.path.clone()) else {
            return;
        };
        if let Some(path) = self.next_song(&current, true) {
            self.play(path);
        }
    }

    // 곡이 끝까지 재생됐으면 정리하고, 반복 설정에 따라 다음 곡 재생하기
    fn check_playing(&mut self) {
        let Some(playing) = &self.playing else {
            return;
        };
        if !playing.player.empty() {
            return;
        }

        let finished = playing.path.clone();
        self.playing = None;

        let next = match self.repeat {
            Repeat::One => Some(finished),
            // 전체 반복은 마지막 곡 다음에 처음부터, 반복을 끄면 마지막 곡에서 멈춤
            Repeat::All => self.next_song(&finished, true),
            Repeat::Off => self.next_song(&finished, false),
        };
        if let Some(path) = next {
            self.play(path);
        }
    }

    // 재생 영역의 첫 줄: 전체 폭을 쓰는 곡 제목
    fn track_title(&mut self, ui: &mut egui::Ui) {
        let size = egui::vec2(ui.available_width(), 20.0);
        let layout = egui::Layout::top_down(egui::Align::Center);
        ui.allocate_ui_with_layout(size, layout, |ui| {
            ui.set_min_size(size);
            let Some(playing) = &self.playing else {
                let hint = tr!("재생할 곡을 선택하세요", "Select a track to play");
                ui.add(egui::Label::new(egui::RichText::new(hint).weak()).truncate());
                return;
            };
            // 재생 위치에 맞춰 움직이므로 일시정지하면 함께 멈춤
            marquee(ui, &file_name(&playing.path), playing.player.get_pos());
            if !playing.player.is_paused() {
                // 부드럽게 흐르도록 자주 다시 그리기
                ui.ctx().request_repaint_after(Duration::from_millis(33));
            }
        });
    }

    // 재생 영역의 둘째 줄: 음표, 곡 순서, 음량
    fn track_info(&mut self, ui: &mut egui::Ui) {
        let row = egui::vec2(ui.available_width(), 28.0);
        let right_to_left = egui::Layout::right_to_left(egui::Align::Center);
        ui.allocate_ui_with_layout(row, right_to_left, |ui| {
            ui.scope(|ui| {
                ui.spacing_mut().slider_width = 70.0;
                // 손잡이가 너무 커지지 않도록 slider는 낮게
                ui.spacing_mut().interact_size.y = 18.0;
                let percent = self.volume * 100.0;
                let before = self.volume;
                let slider = egui::Slider::new(&mut self.volume, 0.0..=1.0)
                    .show_value(false)
                    .trailing_fill(true);
                // 음량은 곡을 고르기 전에도 조절할 수 있음
                let response = ui.add(slider).on_hover_text(format!("{percent:.0}%"));
                // slider로 0까지 내려도 스피커를 눌러 되돌릴 수 있게 이전 음량 기억
                let silenced = response.clicked() && self.volume == 0.0;
                if (response.drag_started() || silenced) && before > 0.0 {
                    self.unmuted = before;
                }
                if response.changed() {
                    self.apply_volume();
                }
            });
            // 스피커를 누르면 음소거, 다시 누르면 이전 음량
            let (icon, hint) = if self.volume > 0.0 {
                (Icon::Speaker, tr!("음소거", "Mute"))
            } else {
                (Icon::Muted, tr!("음소거 해제", "Unmute"))
            };
            if icons::ghost_button(ui, icon, hint).clicked() {
                self.toggle_mute();
            }

            let left_to_right = egui::Layout::left_to_right(egui::Align::Center);
            ui.with_layout(left_to_right, |ui| {
                icons::picture(ui, Icon::Music, tr!("재생 중인 곡", "Now playing"));
                // 소리 파일 중 몇 번째인지
                if let Some(playing) = &self.playing {
                    let songs = self.songs();
                    if let Some(index) = songs.iter().position(|p| p == &playing.path) {
                        let number = format!("{} / {}", index + 1, songs.len());
                        ui.label(egui::RichText::new(number).weak());
                    }
                }
            });
        });
    }

    // 재생 영역의 가운데 줄: 반복, 이전 곡, 재생/일시정지, 다음 곡, 섞기
    fn transport(&mut self, ui: &mut egui::Ui) {
        let size = egui::vec2(ui.available_width(), 52.0);
        let (row, _) = ui.allocate_exact_size(size, egui::Sense::hover());
        // 가운데에서 offset만큼 떨어진 곳의 정사각형
        let at = |offset: f32, side: f32| {
            let center = row.center() + egui::vec2(offset, 0.0);
            egui::Rect::from_center_size(center, egui::Vec2::splat(side))
        };
        let edge = row.width() / 2.0 - 18.0;

        let (repeat_icon, repeat_hint) = match self.repeat {
            Repeat::Off => (Icon::Repeat, tr!("반복 꺼짐", "Repeat off")),
            Repeat::All => (Icon::Repeat, tr!("전체 반복", "Repeat all")),
            Repeat::One => (Icon::RepeatOne, tr!("한 곡 반복", "Repeat one")),
        };
        let repeating = self.repeat != Repeat::Off;
        if icons::flat_button(ui, at(-edge, 32.0), repeat_icon, repeating, repeat_hint).clicked() {
            self.repeat = match self.repeat {
                Repeat::Off => Repeat::All,
                Repeat::All => Repeat::One,
                Repeat::One => Repeat::Off,
            };
        }
        let shuffle_hint = if self.shuffle {
            tr!("섞기 켜짐", "Shuffle on")
        } else {
            tr!("섞기 꺼짐", "Shuffle off")
        };
        if icons::flat_button(
            ui,
            at(edge, 32.0),
            Icon::Shuffle,
            self.shuffle,
            shuffle_hint,
        )
        .clicked()
        {
            self.shuffle = !self.shuffle;
            self.history.clear();
            self.upcoming.clear();
        }

        let active = self.playing.is_some();
        let playing_now = self.playing.as_ref().is_some_and(|p| !p.player.is_paused());
        let mut toggle = false;
        let mut previous = false;
        let mut next = false;
        ui.add_enabled_ui(active, |ui| {
            let hint = tr!("이전 곡", "Previous");
            previous =
                icons::flat_button(ui, at(-76.0, 40.0), Icon::Previous, true, hint).clicked();

            let (icon, hint) = if playing_now {
                (Icon::Pause, tr!("일시정지", "Pause"))
            } else if active {
                (Icon::Play, tr!("계속 재생", "Resume"))
            } else {
                (Icon::Play, tr!("재생", "Play"))
            };
            toggle = icons::round_button(ui, at(0.0, 52.0), icon, hint).clicked();

            let hint = tr!("다음 곡", "Next");
            next = icons::flat_button(ui, at(76.0, 40.0), Icon::Next, true, hint).clicked();
        });

        if toggle && let Some(playing) = &self.playing {
            if playing.player.is_paused() {
                playing.player.play();
            } else {
                playing.player.pause();
            }
        }
        if previous {
            self.previous();
        }
        if next {
            self.skip();
        }
    }

    // 목록 위의 재생 영역: 위에서부터 곡 정보, 재생 버튼, 탐색 bar.
    // 재생 중인 곡이 없어도 같은 모양을 보여주고, 재생 조작만 비활성화
    fn player_bar(&mut self, ui: &mut egui::Ui) {
        // slider의 채워진 부분은 진한 강조색, 빈 부분은 바탕과 구별되는 색
        let colors = theme::palette(ui);
        ui.visuals_mut().selection.bg_fill = colors.point;
        // 손잡이에도 쓰이는 색이므로 불투명하게
        ui.visuals_mut().widgets.inactive.bg_fill = colors.edge.lerp_to_gamma(colors.ink, 0.2);

        self.track_title(ui);
        self.track_info(ui);
        self.transport(ui);

        // 현재 위치, 전체 길이, 끄는 중인 위치
        let (position, total, seeking) = match &self.playing {
            Some(p) => (p.player.get_pos(), p.total, p.seeking),
            None => (Duration::ZERO, None, None),
        };
        // 전체 길이를 알아야 탐색할 수 있음
        let total_secs = total.map_or(0.0, |t| t.as_secs_f32());
        // 끄는 중에는 실제 재생 위치 대신 끌고 있는 위치를 보여주기
        let mut seconds = seeking.unwrap_or(position.as_secs_f32());
        if total.is_some() {
            seconds = seconds.min(total_secs);
        }

        ui.spacing_mut().interact_size.y = 18.0;
        ui.horizontal(|ui| {
            ui.label(format_time(Duration::from_secs_f32(seconds)));

            let right_to_left = egui::Layout::right_to_left(egui::Align::Center);
            ui.with_layout(right_to_left, |ui| {
                ui.label(format_time(total.unwrap_or_default()));

                // 두 시간 표시 사이를 탐색 bar로 채우기
                ui.spacing_mut().slider_width = ui.available_width();
                // 길이가 0인 slider는 만들 수 없으므로 최소 1초로
                let mut bar = seconds;
                let slider = egui::Slider::new(&mut bar, 0.0..=total_secs.max(1.0))
                    .show_value(false)
                    .trailing_fill(true);
                let slider = ui.add_enabled(total.is_some(), slider);

                let Some(playing) = &mut self.playing else {
                    return;
                };
                if total.is_none() {
                    return;
                }
                if slider.dragged() {
                    playing.seeking = Some(bar);
                } else if slider.drag_stopped() || slider.changed() {
                    // 마우스를 놓았을 때 한 번만 이동하기
                    playing.seeking = None;
                    if let Err(e) = playing.player.try_seek(Duration::from_secs_f32(bar)) {
                        self.toast = Some(Toast::new(tr!(
                            "오류: 이동할 수 없어요: {e}",
                            "Error: cannot seek: {e}"
                        )));
                    }
                }
            });
        });
    }

    // 함께 설치한 오픈소스 license 문서 열기
    fn open_licenses(&mut self) {
        // 개발 중(cargo run)에는 설치된 문서가 없으므로 그 자리에서 만들기
        #[cfg(debug_assertions)]
        let found = notices_path().map_or_else(make_dev_notices, Ok);
        #[cfg(not(debug_assertions))]
        let found = notices_path().ok_or_else(|| {
            tr!(
                "라이선스 문서를 찾을 수 없어요",
                "the license document was not found",
            )
        });

        let path = match found {
            Ok(path) => path,
            Err(e) => {
                self.toast = Some(Toast::new(tr!(
                    "오류: 라이선스 문서를 열 수 없어요: {e}",
                    "Error: cannot open the license document: {e}",
                )));
                return;
            }
        };
        if let Err(e) = open::that_detached(path) {
            self.toast = Some(Toast::new(tr!(
                "오류: 라이선스 문서를 열 수 없어요: {e}",
                "Error: cannot open the license document: {e}",
            )));
        }
    }

    // 화면 위쪽: URL 입력과 형식, 저장 위치, 저장 버튼
    fn download_form(&mut self, ui: &mut egui::Ui) {
        let has_url = !self.url.trim().is_empty();
        let mut clicked = false;
        let mut entered = false;

        // 첫 줄: 형식 선택을 오른쪽에 두고, 입력창이 남은 폭을 채우기
        let row = egui::vec2(ui.available_width(), 32.0);
        let right_to_left = egui::Layout::right_to_left(egui::Align::Center);
        ui.allocate_ui_with_layout(row, right_to_left, |ui| {
            let name = |format| match format {
                Format::Mp3 => tr!("소리 (MP3)", "Audio (MP3)"),
                Format::Mp4 => tr!("영상 (MP4)", "Video (MP4)"),
            };
            ui.scope(|ui| {
                ui.spacing_mut().interact_size.y = 32.0;
                egui::ComboBox::from_id_salt("format")
                    .selected_text(name(self.format))
                    .width(96.0)
                    .show_ui(ui, |ui| {
                        for format in [Format::Mp3, Format::Mp4] {
                            ui.selectable_value(&mut self.format, format, name(format));
                        }
                    });
            });

            // 지울 내용이 있으면 지우기 버튼이 글자를 가리지 않게 오른쪽 여백 넓히기
            let filled = !self.url.is_empty();
            let margin = egui::Margin {
                left: 8,
                right: if filled { 32 } else { 8 },
                top: 2,
                bottom: 2,
            };
            let input = ui.add(
                egui::TextEdit::singleline(&mut self.url)
                    .hint_text(tr!("영상 URL 붙여넣기", "Paste a video URL"))
                    .desired_width(f32::INFINITY)
                    .min_size(egui::vec2(0.0, 32.0))
                    .margin(margin)
                    .align(egui::Align2::LEFT_CENTER),
            );
            if filled {
                let center = egui::pos2(input.rect.right() - 16.0, input.rect.center().y);
                let rect = egui::Rect::from_center_size(center, egui::Vec2::splat(22.0));
                if icons::clear_button(ui, rect, tr!("내용 지우기", "Clear")).clicked() {
                    self.url.clear();
                    // 바로 새 주소를 붙여넣을 수 있게 입력창 선택
                    input.request_focus();
                }
            }
            // 입력창에서 Enter를 눌렀는지
            entered = input.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        });

        // 둘째 줄: 다운로드 폴더
        ui.add_space(4.0);
        if folder_button(ui, &self.folder) {
            // 취소하면 None이므로 기존 폴더를 그대로 유지
            if let Some(folder) = rfd::FileDialog::new()
                .set_directory(&self.folder)
                .pick_folder()
            {
                // 받는 위치만 바꾸고 재생목록은 그대로
                self.folder = folder;
            }
        }

        // 셋째 줄: 주소, 형식, 폴더를 확인한 뒤 누르는 저장 버튼
        ui.add_space(4.0);
        let label = tr!("저장하기", "Save");
        let width = ui.available_width();
        clicked |= icons::primary_button(ui, Icon::Download, label, has_url, width).clicked();

        if clicked || (entered && has_url) {
            self.start_download(ui.ctx().clone());
        }
    }

    // 확인을 받은 뒤 파일들을 휴지통으로 옮기고 재생목록에서 빼기
    fn delete(&mut self, paths: Vec<PathBuf>) {
        if paths.is_empty() || !confirm_delete(&paths) {
            return;
        }

        let mut removed = Vec::new();
        let mut moved = Vec::new();
        let mut errors = Vec::new();
        for path in paths {
            // 지우려는 곡이 재생 중이면 먼저 정지 (Windows는 열린 파일을 옮길 수 없음)
            if self.playing.as_ref().is_some_and(|p| p.path == path) {
                self.playing = None;
            }
            // 이미 없는 파일은 목록에서만 빼기
            if !path.exists() {
                removed.push(path);
                continue;
            }
            match move_to_trash(&path) {
                Ok(()) => {
                    moved.push(file_name(&path));
                    removed.push(path);
                }
                Err(e) => errors.push(e.to_string()),
            }
        }

        let message = if let Some(e) = errors.first() {
            let count = errors.len();
            tr!(
                "오류: {count}개를 삭제할 수 없어요: {e}",
                "Error: cannot delete {count} file(s): {e}",
            )
        } else if let [name] = moved.as_slice() {
            tr!("휴지통으로 옮겼어요: {name}", "Moved to trash: {name}")
        } else {
            let count = moved.len();
            tr!(
                "{count}개 파일을 휴지통으로 옮겼어요",
                "Moved {count} files to trash",
            )
        };
        self.toast = Some(Toast::new(message));
        self.remove_from_list(&removed);
    }

    // 목록의 from 번째 파일을 to 번째 줄 앞으로 옮기기
    fn move_file(&mut self, from: usize, to: usize) {
        move_item(&mut self.files, from, to);
    }

    // 편집 모드 시작. 목록을 고치는 동안에는 재생을 일시정지
    fn start_editing(&mut self) {
        if let Some(playing) = &self.playing {
            playing.player.pause();
        }
        self.selected = Some(HashSet::new());
    }

    // 목록의 제목 줄. 편집 중일 때는 전체 선택, 빼기, 삭제, 완료 버튼을 보여줌
    fn list_header(&mut self, ui: &mut egui::Ui) {
        let mut delete = None;
        let mut remove = None;
        ui.horizontal(|ui| {
            let count = self.files.len();
            let title = match &self.selected {
                Some(selected) => {
                    let chosen = selected.len();
                    tr!("{chosen}개 선택", "{chosen} selected")
                }
                None => tr!("재생목록 ({count})", "Playlist ({count})"),
            };
            ui.label(egui::RichText::new(title).strong());

            let right_to_left = egui::Layout::right_to_left(egui::Align::Center);
            ui.with_layout(right_to_left, |ui| {
                let Some(selected) = &mut self.selected else {
                    let hint = tr!(
                        "편집: 빼기, 삭제, 순서 바꾸기",
                        "Edit: remove, delete, or reorder",
                    );
                    ui.add_enabled_ui(!self.files.is_empty(), |ui| {
                        if icons::button(ui, Icon::Edit, hint).clicked() {
                            self.start_editing();
                        }
                    });
                    return;
                };

                let done = ui.button(tr!("완료", "Done")).clicked();
                ui.add_enabled_ui(!selected.is_empty(), |ui| {
                    // 목록에 보이는 순서대로
                    let chosen = || {
                        let files = self.files.iter().filter(|p| selected.contains(*p));
                        files.cloned().collect::<Vec<_>>()
                    };
                    // 오른쪽부터 배치
                    if ui
                        .button(tr!("파일 삭제", "Delete"))
                        .on_hover_text(tr!(
                            "원본 파일을 휴지통으로 옮기기",
                            "Move the files to the trash"
                        ))
                        .clicked()
                    {
                        delete = Some(chosen());
                    }
                    if ui
                        .button(tr!("목록에서 빼기", "Remove"))
                        .on_hover_text(tr!(
                            "원본 파일은 그대로 두기",
                            "The files themselves are kept"
                        ))
                        .clicked()
                    {
                        remove = Some(chosen());
                    }
                });
                // 모두 골랐으면 같은 버튼으로 모두 해제
                let all = selected.len() == self.files.len();
                let label = if all {
                    tr!("선택 해제", "Clear all")
                } else {
                    tr!("전체 선택", "Select all")
                };
                if ui.button(label).clicked() {
                    if all {
                        selected.clear();
                    } else {
                        selected.extend(self.files.iter().cloned());
                    }
                }
                if done {
                    self.selected = None;
                    self.dragging = None;
                }
            });
        });

        if let Some(paths) = delete {
            self.delete(paths);
        }
        if let Some(paths) = remove {
            let count = paths.len();
            self.remove_from_list(&paths);
            self.toast = Some(Toast::new(tr!(
                "{count}개를 목록에서 뺐어요",
                "Removed {count} file(s) from the playlist",
            )));
        }
    }

    // 저장된 mp3 목록 보여주기
    fn file_list(&mut self, ui: &mut egui::Ui) {
        self.list_header(ui);

        if self.files.is_empty() {
            empty_playlist(ui);
            return;
        }

        // 목록을 그리는 동안에는 클릭한 파일만 기억하고, 재생과 삭제는 그린 뒤에 하기
        let mut clicked = None;
        let mut toggle_pause = false;
        let mut toggle_select = None;
        let mut recheck = false;
        // 기본 스크롤 bar는 평소에 숨어 있으므로, 목록이 넘치면 항상 보이는 모양 사용
        ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            // 기본 최소 높이(64)를 없애서, 공간이 좁아도 하단 영역 뒤로 넘치지 않게 하기
            .min_scrolled_height(0.0)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = ROW_GAP;

                // 끄는 중이면: (원래 번호, 지금 놓으면 갈 번호, 떠 있는 줄의 위치)
                let top = ui.cursor().top();
                let moving = self.dragging.as_ref().and_then(|drag| {
                    let from = self.files.iter().position(|path| path == &drag.path)?;
                    let pointer = ui.ctx().pointer_latest_pos()?;
                    let step = drag.height + ROW_GAP;
                    let floating = pointer.y - drag.grab;
                    let to = nearest_slot(floating - top, step, self.files.len());
                    Some((from, to, floating))
                });
                let released = !ui.input(|i| i.pointer.primary_down());
                // 떠 있는 줄을 목록 위에 겹쳐 그릴 위치
                let mut floating_row = None;

                for (index, path) in self.files.iter().enumerate() {
                    let video = is_video(path);
                    let missing = self.missing.contains(path);
                    // 이 파일이 재생 중이면 Some(일시정지 여부)
                    let current = self
                        .playing
                        .as_ref()
                        .filter(|p| &p.path == path)
                        .map(|p| p.player.is_paused());

                    // 선택 중이면 Some(이 파일을 골랐는지)
                    let checked = self.selected.as_ref().map(|s| s.contains(path));

                    // 줄 전체가 재생 버튼 (편집 중에는 선택 버튼).
                    let builder = egui::UiBuilder::new()
                        .id_salt(path)
                        // 편집 중에는 끌어서 순서를 바꿀 수도 있음
                        .sense(if checked.is_some() {
                            egui::Sense::click_and_drag()
                        } else {
                            egui::Sense::click()
                        });
                    let dragged = self.dragging.as_ref().is_some_and(|d| &d.path == path);
                    // 끄는 줄이 지나가면 다른 줄은 한 칸 비켜 주기
                    let shift = match (&self.dragging, moving) {
                        (Some(drag), Some((from, to, _))) if !dragged => {
                            shift_slots(index, from, to) as f32 * (drag.height + ROW_GAP)
                        }
                        _ => 0.0,
                    };
                    // 비키는 움직임은 부드럽게. 놓은 순간에는 새 자리에서 바로 시작
                    let dropping = released && moving.is_some();
                    let duration = if dropping { 0.0 } else { SLIDE_SECONDS };
                    let offset =
                        ui.ctx()
                            .animate_value_with_time(row_shift_id(path), shift, duration);
                    let row = ui.scope_builder(builder, |ui| {
                        // 글자를 선택하는 기능이 줄의 클릭을 가로채지 않게 끄기
                        ui.style_mut().interaction.selectable_labels = false;
                        let hovered = ui.response().contains_pointer();

                        let highlighted = checked.unwrap_or(current.is_some());
                        // 끄는 줄의 원래 자리는 비워 두기 (마우스를 따라 따로 그림)
                        if dragged {
                            ui.multiply_opacity(0.0);
                        } else if missing && checked.is_none() {
                            // 찾을 수 없는 파일은 흐리게
                            ui.multiply_opacity(0.45);
                        }
                        let look = RowLook {
                            checked,
                            current: current.is_some(),
                            missing,
                        };
                        let hovered = hovered && !missing && moving.is_none();
                        let transform =
                            egui::emath::TSTransform::from_translation(egui::vec2(0.0, offset));
                        ui.with_visual_transform(transform, |ui| {
                            file_card(ui, highlighted, hovered)
                                .show(ui, |ui| row_content(ui, path, look));
                        });
                        if dragged {
                            floating_row = Some((index, look));
                        }
                    });

                    // 마우스를 올리면 누를 수 있다는 것과 할 일을 알려주기
                    let hint = match current {
                        _ if checked.is_some() => {
                            tr!(
                                "눌러서 선택, 끌어서 순서 바꾸기",
                                "Click to select, drag to reorder",
                            )
                        }
                        _ if missing => tr!("파일을 찾을 수 없어요", "The file cannot be found"),
                        Some(false) => tr!("눌러서 일시정지", "Click to pause"),
                        Some(true) => tr!("눌러서 계속 재생", "Click to resume"),
                        None if video => {
                            tr!(
                                "눌러서 기본 player로 열기",
                                "Click to open in default player"
                            )
                        }
                        None => tr!("눌러서 재생", "Click to play"),
                    };
                    let response = row.response;
                    if checked.is_some()
                        && response.drag_started()
                        && let Some(pointer) = response.interact_pointer_pos()
                    {
                        // 누른 곳을 기준으로 해야 줄이 튀지 않음
                        let origin = ui.input(|i| i.pointer.press_origin());
                        self.dragging = Some(Drag {
                            path: path.clone(),
                            grab: origin.unwrap_or(pointer).y - response.rect.top(),
                            height: response.rect.height(),
                        });
                    }
                    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
                    // 끄는 중에는 설명이 놓을 위치를 가리지 않게 숨기기
                    let response = if self.dragging.is_some() {
                        response
                    } else {
                        response.on_hover_text(hint)
                    };
                    if response.clicked() {
                        if checked.is_some() {
                            toggle_select = Some(path.clone());
                        } else if missing {
                            // 그 사이에 다시 생겼을 수 있으므로 한 번 더 확인
                            recheck = true;
                        } else if current.is_some() {
                            toggle_pause = true;
                        } else {
                            clicked = Some(path.clone());
                        }
                    }
                }

                // 끄는 줄은 목록 위에 떠서 마우스를 따라감
                if let Some((from, to, floating)) = moving
                    && let Some(drag) = &self.dragging
                {
                    let step = drag.height + ROW_GAP;
                    if let Some((index, look)) = floating_row {
                        let rect = egui::Rect::from_min_size(
                            egui::pos2(ui.cursor().left(), floating),
                            egui::vec2(ui.available_width(), drag.height),
                        );
                        floating_card(ui, rect, &self.files[index], look);
                    }
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);

                    if released {
                        let path = drag.path.clone();
                        // 놓은 곳에서 새 자리까지 미끄러져 들어가게 하기
                        let from_slot = floating - (top + to as f32 * step);
                        // 다른 줄은 이미 새 자리에 보이므로 비킨 거리를 바로 없애기
                        for other in &self.files {
                            ui.ctx()
                                .animate_value_with_time(row_shift_id(other), 0.0, 0.0);
                        }
                        ui.ctx()
                            .animate_value_with_time(row_shift_id(&path), from_slot, 0.0);
                        self.move_file(from, to_insert_position(from, to));
                        ui.ctx().request_repaint();
                    } else if let Some(pointer) = ui.ctx().pointer_latest_pos() {
                        scroll_near_edge(ui, pointer.y);
                    }
                }
                // 버튼을 뗐으면 끌기 끝
                if released {
                    self.dragging = None;
                }
            });

        if recheck {
            self.check_missing();
        }
        // 이미 골랐으면 해제, 아니면 선택
        if let Some(path) = toggle_select
            && let Some(selected) = &mut self.selected
            && !selected.remove(&path)
        {
            selected.insert(path);
        }
        if toggle_pause && let Some(playing) = &self.playing {
            if playing.player.is_paused() {
                playing.player.play();
            } else {
                playing.player.pause();
            }
        }
        if let Some(path) = clicked {
            if is_video(&path) {
                self.open_video(&path);
            } else {
                self.choose(path);
            }
        }
    }

    // 별도 thread에서 다운로드 시작하기 (화면이 멈추지 않도록)
    fn start_download(&mut self, ctx: egui::Context) {
        let (sender, receiver) = mpsc::channel();
        let url = self.url.trim().to_string();
        let folder = self.folder.clone();
        let format = self.format;

        thread::spawn(move || {
            // 소식을 보내고 화면을 다시 그려 달라고 요청
            let send = |update| {
                // 창이 이미 닫혔다면 보낼 곳이 없으므로 결과는 무시
                let _ = sender.send(update);
                ctx.request_repaint();
            };

            let result = download(&url, &folder, format, send);
            send(Update::Done(result));
        });

        self.job = Some(Job {
            receiver,
            format,
            progress: None,
        });
        self.status = tr!("다운로드 준비 중...", "Preparing download...");
    }

    // 다운로드 thread가 보낸 소식 확인하기 (기다리지 않음)
    fn check_download(&mut self) {
        let Some(job) = &mut self.job else {
            return;
        };

        let mut downloaded = Vec::new();
        // 그동안 쌓인 소식을 모두 처리
        self.status = loop {
            match job.receiver.try_recv() {
                Ok(Update::Progress(progress)) => {
                    job.progress = Some(progress);
                    self.status = tr!("다운로드 중...", "Downloading...");
                }
                Ok(Update::Processing) => {
                    job.progress = None;
                    self.status = match job.format {
                        Format::Mp3 => tr!("mp3로 변환 중...", "Converting to mp3..."),
                        Format::Mp4 => tr!(
                            "영상과 소리를 mp4로 정리하는 중...",
                            "Packaging video and audio as mp4..."
                        ),
                    };
                }
                // 완료는 알림, 실패는 팝업으로 알려주기
                Ok(Update::Done(Ok(paths))) => {
                    downloaded = paths.clone();
                    self.toast = Some(Toast::saved(&paths, &self.folder));
                    break String::new();
                }
                Ok(Update::Done(Err(reason))) => {
                    self.failure = Some(Failure {
                        reason,
                        copied: false,
                    });
                    break String::new();
                }
                Err(TryRecvError::Empty) => return, // 아직 진행 중
                Err(TryRecvError::Disconnected) => {
                    break tr!(
                        "오류: 작업이 중단됐어요",
                        "Error: the task stopped unexpectedly"
                    );
                }
            }
        };
        self.job = None;
        // 받은 파일은 재생목록 맨 위에 추가
        self.add_files(downloaded);
    }
}

impl eframe::App for App {
    // 화면 그리기 전에 처리할 일. 창이 가려져 있어도 호출됨
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.check_download();
        self.check_playing();

        // 다른 곳에서 파일이 옮겨졌을 수 있으므로, 창으로 돌아오면 다시 확인
        let focused = ctx.input(|i| i.focused);
        if focused && !self.focused {
            self.check_missing();
        }
        self.focused = focused;

        let dropped: Vec<PathBuf> = ctx.input(|i| {
            let files = i.raw.dropped_files.iter();
            files.map(|file| file.path().to_path_buf()).collect()
        });
        if !dropped.is_empty() {
            self.add_dropped(&dropped);
        }

        // 재생 시간 표시와 다음 곡 재생을 위해 잠시 뒤 다시 확인하기
        if self.playing.is_some() {
            ctx.request_repaint_after(Duration::from_millis(500));
        }
    }

    // 종료할 때와 실행 중 주기적으로 설정 저장하기
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        if let Some(folder) = self.folder.to_str() {
            storage.set_string("folder", folder.to_string());
        }
        storage.set_string("volume", self.volume.to_string());
        storage.set_string("unmuted", self.unmuted.to_string());
        storage.set_string("color_preset", self.colors.preset.to_string());
        for (key, custom) in ["color_dark", "color_light"]
            .into_iter()
            .zip(self.colors.custom)
        {
            // 직접 고른 색이 없으면 빈 값으로 저장해서 이전 값을 지우기
            let value = custom.map(theme::format_colors).unwrap_or_default();
            storage.set_string(key, value);
        }
        let repeat = match self.repeat {
            Repeat::Off => "off",
            Repeat::All => "all",
            Repeat::One => "one",
        };
        storage.set_string("repeat", repeat.to_string());
        storage.set_string("playlist", format_playlist(&self.files));
        let format = match self.format {
            Format::Mp3 => "mp3",
            Format::Mp4 => "mp4",
        };
        storage.set_string("format", format.to_string());
        if self.language_chosen {
            let language = if lang::is_korean() { "ko" } else { "en" };
            storage.set_string("language", language.to_string());
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let idle = self.job.is_none();

        self.about_dialog(ui);
        self.color_dialog(ui);
        self.failure_dialog(ui);
        // 다른 팝업에서 생긴 오류도 보이도록 마지막에
        self.show_toast(ui);
        // 파일을 창 위로 끌어 오면 안내 보여주기
        if ui.input(|i| !i.raw.hovered_files.is_empty()) {
            drop_overlay(ui);
        }

        egui::Panel::top("title")
            .frame(edge_frame(ui))
            .show(ui, |ui| self.title_bar(ui));

        // 아래쪽 영역은 나중에 배치한 것이 위에 놓임.
        // 남은 공간을 쓰는 CentralPanel보다 먼저 배치해야 함

        egui::CentralPanel::default().show(ui, |ui| {
            // 다운로드 중에는 입력과 버튼을 비활성화
            ui.add_enabled_ui(idle, |ui| self.download_form(ui));

            // 알려줄 내용이 있을 때만 상태 줄 보여주기
            if !self.status.is_empty() {
                ui.separator();
                ui.horizontal(|ui| {
                    if !idle {
                        ui.spinner();
                    }
                    ui.label(&self.status);
                });
                if let Some(progress) = self.job.as_ref().and_then(|job| job.progress) {
                    ui.scope(|ui| {
                        // 진행률 글자가 채워진 부분 위에서도 보이게 글자색 사용
                        let colors = theme::palette(ui);
                        ui.visuals_mut().selection.bg_fill = colors.tint(0.55);
                        ui.visuals_mut().selection.stroke.color = colors.ink;
                        ui.add(egui::ProgressBar::new(progress).show_percentage());
                    });
                }
            }

            ui.separator();
            // 재생 영역은 항상 보여줘서, 재생을 시작해도 목록이 움직이지 않게 하기
            player_frame(ui).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.set_height(PLAYER_HEIGHT);
                self.player_bar(ui);
            });
            ui.add_space(4.0);
            self.file_list(ui);
        });
    }
}

// 저장 형식에 따라 달라지는 yt-dlp 옵션
fn format_args(format: Format) -> &'static [&'static str] {
    match format {
        //   -x                  : 오디오만 추출
        //   --audio-format mp3  : mp3로 변환 (ffmpeg 필요)
        Format::Mp3 => &["-x", "--audio-format", "mp3"],
        //   -f                      : 가장 좋은 영상과 소리를 받기
        //   -S                      : 1080p 이하, 재생이 잘 되는 H.264와 AAC를 우선
        //   --merge-output-format   : 영상과 소리를 mp4 하나로 합치기
        //   --remux-video           : 하나로 받은 파일도 mp4에 담기
        //   -o                      : 파일 이름. mp3로 받을 때 생기는 임시 파일과
        //                             겹치지 않도록 video를 붙임
        Format::Mp4 => &[
            "-f",
            "bv*+ba/b",
            "-S",
            "res:1080,vcodec:h264,acodec:m4a",
            "--merge-output-format",
            "mp4",
            "--remux-video",
            "mp4",
            "-o",
            "%(title)s [%(id)s] video.%(ext)s",
        ],
    }
}

// yt-dlp를 실행해서 저장하기. 진행 상황은 on_update로 알려줌
fn download(
    url: &str,
    folder: &Path,
    format: Format,
    on_update: impl Fn(Update),
) -> Result<Vec<PathBuf>, String> {
    //   --print             : 다 만든 파일의 경로를 알려주기
    //   --no-quiet          : --print를 써도 진행 상황 출력은 유지
    //   --no-playlist       : 재생목록 링크여도 영상 하나만 받기
    //   --no-mtime          : 파일 날짜를 받은 시각으로 (최신순 정렬용)
    //   --newline           : 진행률을 한 줄씩 출력
    //   --color never       : 출력에 색상 코드를 넣지 않기
    //   --encoding utf-8    : 한글이 깨지지 않도록 UTF-8로 출력
    //   --progress-template : 진행률 줄의 모양 지정
    //   -P 폴더             : 저장할 폴더 지정
    //   --                  : 이 뒤의 값은 옵션이 아니라고 알려주기
    let mut command = Command::new("yt-dlp");
    command
        .args(format_args(format))
        .args(["--no-playlist", "--no-mtime", "--no-quiet", "--progress"])
        .arg("--print")
        .arg(format!("after_move:{FILE_PREFIX}%(filepath)s"))
        .args(["--newline", "--color", "never", "--encoding", "utf-8"])
        .arg("--progress-template")
        .arg(format!(
            "download:{PROGRESS_PREFIX}%(progress._percent_str)s"
        ))
        .arg("-P")
        .arg(folder)
        .arg("--")
        .arg(url)
        // yt-dlp와, yt-dlp가 부르는 ffmpeg 등을 찾을 위치
        .env("PATH", tool_path())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    hide_console(&mut command);

    let mut child = command
        .spawn()
        .map_err(|e| tr!("yt-dlp를 실행할 수 없어요: {e}", "cannot run yt-dlp: {e}"))?;

    // 오류 출력은 별도 thread에서 읽기.
    // 읽지 않고 두면 통로가 가득 찼을 때 yt-dlp가 멈출 수 있음
    let mut stderr = child.stderr.take().expect("stderr를 piped로 설정함");
    let errors = thread::spawn(move || {
        let mut bytes = Vec::new();
        // 읽기에 실패해도 읽은 데까지만 사용
        let _ = stderr.read_to_end(&mut bytes);
        String::from_utf8_lossy(&bytes).into_owned()
    });

    // 일반 출력은 한 줄씩 읽으면서 진행 상황 알리기
    let stdout = child.stdout.take().expect("stdout을 piped로 설정함");
    let mut files = Vec::new();
    for line in BufReader::new(stdout).split(b'\n') {
        let Ok(line) = line else {
            break;
        };
        let line = String::from_utf8_lossy(&line);

        if let Some(path) = line.strip_prefix(FILE_PREFIX) {
            files.push(PathBuf::from(path.trim_end_matches('\r')));
        } else if let Some(progress) = parse_progress(&line) {
            on_update(Update::Progress(progress));
        } else if ["[ExtractAudio]", "[Merger]", "[VideoRemuxer]"]
            .iter()
            .any(|step| line.starts_with(step))
        {
            on_update(Update::Processing);
        }
    }

    let status = child.wait().map_err(|e| {
        tr!(
            "yt-dlp의 종료를 확인할 수 없어요: {e}",
            "cannot wait for yt-dlp: {e}"
        )
    })?;

    if !status.success() {
        // yt-dlp가 남긴 오류의 마지막 줄을 보여주기
        let stderr = errors.join().unwrap_or_default();
        let reason = match stderr.lines().last() {
            Some(line) => line.to_string(),
            None => tr!("원인을 알 수 없어요", "unknown reason"),
        };
        return Err(reason);
    }

    Ok(files)
}

// Windows에서 yt-dlp를 실행할 때 콘솔 창이 뜨지 않게 하기
#[cfg(windows)]
fn hide_console(command: &mut Command) {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

// 다른 운영체제에서는 할 일이 없음
#[cfg(not(windows))]
fn hide_console(_command: &mut Command) {}

// 받은 파일의 경로 줄을 구별하기 위한 표시
const FILE_PREFIX: &str = "[file]";

// 진행률 줄을 다른 출력과 구별하기 위한 표시
const PROGRESS_PREFIX: &str = "[progress]";

// "[progress] 45.3%" 같은 줄에서 비율(0.0 ~ 1.0) 꺼내기
fn parse_progress(line: &str) -> Option<f32> {
    let percent = line
        .strip_prefix(PROGRESS_PREFIX)?
        .trim()
        .strip_suffix('%')?
        .parse::<f32>()
        .ok()?;
    Some((percent / 100.0).clamp(0.0, 1.0))
}

// 외부 프로그램을 찾을 PATH 만들기
fn tool_path() -> OsString {
    let current = std::env::var_os("PATH").unwrap_or_default();
    let mut folders = Vec::new();

    // 1. 프로그램과 함께 설치한 도구.
    //    PC에 다른 버전이 있어도 함께 설치한 것을 먼저 사용
    if let Ok(exe) = std::env::current_exe()
        && let Some(folder) = exe.parent()
    {
        // Windows: 실행 파일 옆의 tools 폴더
        folders.push(folder.join("tools"));
        // Mac: .app 안의 Contents/Resources/tools 폴더
        folders.push(folder.join("../Resources/tools"));
    }

    // 2. 원래의 PATH
    folders.extend(std::env::split_paths(&current));

    // 3. Apple Silicon과 Intel Mac의 Homebrew 위치.
    //    Finder에서 실행한 .app은 PATH에 이 폴더가 없음
    folders.push(PathBuf::from("/opt/homebrew/bin"));
    folders.push(PathBuf::from("/usr/local/bin"));

    std::env::join_paths(folders).unwrap_or(current)
}

// 재생 영역의 틀. 헤더와 같은 바탕색의 둥근 카드
fn player_frame(ui: &egui::Ui) -> egui::Frame {
    egui::Frame::new()
        .fill(theme::palette(ui).edge)
        .corner_radius(12)
        .inner_margin(egui::Margin::symmetric(10, 8))
}

// 헤더의 틀. 본문과 구별되는 바탕색 사용
fn edge_frame(ui: &egui::Ui) -> egui::Frame {
    egui::Frame::side_top_panel(ui.style()).fill(theme::palette(ui).edge)
}

// 뒤의 화면을 어둡게 덮어 누르지 못하게 하는 팝업. 닫아야 하면 true.
// 닫기는 오른쪽 위의 × 또는 Esc. body는 창이 작으면 스크롤되고,
// 제목과 footer의 버튼은 항상 보임
fn dialog(
    ui: &egui::Ui,
    id: &str,
    title: &str,
    body: impl FnOnce(&mut egui::Ui),
    footer: impl FnOnce(&mut egui::Ui),
) -> bool {
    let screen = ui.ctx().content_rect().size();
    let mut closed = false;

    let modal = egui::Modal::new(egui::Id::unique(id)).frame(popup_frame(ui));
    let response = modal.show(ui.ctx(), |ui| {
        ui.set_width((screen.x - 64.0).min(320.0));

        ui.horizontal(|ui| {
            let strong = ui.visuals().strong_text_color();
            ui.label(egui::RichText::new(title).size(16.0).strong().color(strong));
            let right_to_left = egui::Layout::right_to_left(egui::Align::Center);
            ui.with_layout(right_to_left, |ui| {
                let (rect, _) =
                    ui.allocate_exact_size(egui::Vec2::splat(26.0), egui::Sense::hover());
                closed = icons::clear_button(ui, rect, tr!("닫기", "Close")).clicked();
            });
        });

        // 창 높이에서 여백, 제목, 아래 버튼이 차지하는 높이를 뺀 만큼
        let height = (screen.y - 140.0).max(60.0);
        egui::ScrollArea::vertical()
            .max_height(height)
            .min_scrolled_height(0.0)
            .auto_shrink([false, true])
            .show(ui, body);

        ui.with_layout(egui::Layout::top_down(egui::Align::Max), footer);
    });

    // 바깥을 눌러서는 닫지 않음. 색상 선택기 등이 열려 있으면 Esc는 그것을 닫음
    let escaped = response.is_top_modal
        && !response.any_popup_open
        && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
    closed || escaped
}

// 색 조합 하나를 고르는 버튼. 세 색의 견본과 이름을 보여줌
fn preset_button(ui: &mut egui::Ui, colors: theme::Colors, name: &str, selected: bool) -> bool {
    let size = egui::vec2(ui.available_width(), 52.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, name));

    let palette = theme::palette(ui);
    let line = if selected {
        egui::Stroke::new(2.0, palette.point)
    } else if response.hovered() {
        egui::Stroke::new(2.0, palette.shade(0.4))
    } else {
        egui::Stroke::new(1.0, palette.shade(0.2))
    };
    ui.painter()
        .rect_stroke(rect, 10.0, line, egui::StrokeKind::Inside);
    for (offset, color) in [-14.0, 0.0, 14.0].into_iter().zip(colors) {
        let center = egui::pos2(rect.center().x + offset, rect.top() + 18.0);
        ui.painter().circle_filled(center, 9.0, color);
        ui.painter()
            .circle_stroke(center, 9.0, egui::Stroke::new(1.0, egui::Color32::GRAY));
    }
    ui.painter().text(
        egui::pos2(rect.center().x, rect.bottom() - 12.0),
        egui::Align2::CENTER_CENTER,
        name,
        egui::FontId::proportional(12.0),
        palette.ink,
    );
    response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
}

// 안내창의 틀. 뒤의 화면과 구별되도록 강조색 테두리와 조금 밝은 바탕 사용
fn popup_frame(ui: &egui::Ui) -> egui::Frame {
    let colors = theme::palette(ui);
    egui::Frame::popup(ui.style())
        .fill(colors.shade(0.08))
        .stroke(egui::Stroke::new(2.0, colors.point))
        .inner_margin(14)
}

// 줄 전체가 버튼인 다운로드 폴더 줄. 누르면 true
fn folder_button(ui: &mut egui::Ui, folder: &Path) -> bool {
    let builder = egui::UiBuilder::new()
        .id_salt("download-folder")
        .sense(egui::Sense::click());
    let row = ui.scope_builder(builder, |ui| {
        // 글자를 선택하는 기능이 클릭을 가로채지 않게 끄기
        ui.style_mut().interaction.selectable_labels = false;
        let colors = theme::palette(ui);
        let (fill, line) = if ui.response().contains_pointer() && ui.is_enabled() {
            (colors.veil(0.07), colors.point)
        } else {
            (egui::Color32::TRANSPARENT, colors.shade(0.18))
        };

        egui::Frame::new()
            .fill(fill)
            .stroke(egui::Stroke::new(1.0, line))
            .corner_radius(8)
            .inner_margin(egui::Margin::symmetric(10, 4))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.set_width(ui.available_width());
                    icons::symbol(ui, Icon::Folder);

                    // 경로가 길면 "변경"의 자리를 남기고 줄 바꿈
                    let width = ui.available_width() - 60.0;
                    ui.vertical(|ui| {
                        ui.set_width(width);
                        ui.add_space(5.0);
                        let path = egui::RichText::new(folder.display().to_string()).weak();
                        ui.add(egui::Label::new(path).wrap());
                        ui.add_space(2.0);
                    });
                    let right_to_left = egui::Layout::right_to_left(egui::Align::Center);
                    ui.with_layout(right_to_left, |ui| {
                        let change = egui::RichText::new(tr!("변경", "Change"))
                            .size(12.0)
                            .color(colors.point);
                        ui.label(change);
                    });
                });
            });
    });

    row.response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(tr!("다운로드 폴더 변경", "Change download folder"))
        .clicked()
}

// 재생목록이 비었을 때의 안내
fn empty_playlist(ui: &mut egui::Ui) {
    ui.add_space(8.0);
    ui.vertical_centered(|ui| {
        ui.label(egui::RichText::new(tr!("내려받거나", "Download something, or")).weak());
        ui.label(
            egui::RichText::new(tr!(
                "음악, 영상 파일과 폴더를 끌어다 놓으세요",
                "drop music, video files or folders here",
            ))
            .strong(),
        );
    });
}

// 파일을 끌어 오는 동안 창 전체에 보여주는 안내
fn drop_overlay(ui: &egui::Ui) {
    let colors = theme::palette(ui);
    let screen = ui.ctx().content_rect();
    let layer = egui::LayerId::new(egui::Order::Tooltip, egui::Id::unique("drop"));
    let painter = ui.ctx().layer_painter(layer);
    painter.rect_filled(screen, 0.0, colors.base.gamma_multiply(0.85));

    // 점선 테두리
    let inner = screen.shrink(12.0);
    let corners = [
        inner.left_top(),
        inner.right_top(),
        inner.right_bottom(),
        inner.left_bottom(),
        inner.left_top(),
    ];
    let line = egui::Stroke::new(3.0, colors.point);
    painter.extend(egui::Shape::dashed_line(&corners, line, 10.0, 7.0));

    let lines = [
        (
            tr!(
                "여기에 놓으면 재생목록에 추가",
                "Drop to add to the playlist"
            ),
            18.0,
            colors.point,
            -24.0,
        ),
        (
            tr!("음악, 영상 파일 또는 폴더", "Music, video files or folders"),
            13.0,
            colors.ink,
            6.0,
        ),
        (
            tr!(
                "원본은 옮기거나 복사하지 않아요",
                "Files are not moved or copied"
            ),
            13.0,
            colors.ink,
            26.0,
        ),
    ];
    for (text, size, color, offset) in lines {
        painter.text(
            screen.center() + egui::vec2(0.0, offset),
            egui::Align2::CENTER_CENTER,
            text,
            egui::FontId::proportional(size),
            color,
        );
    }
}

// 목록의 줄 사이 간격과, 줄이 비키는 데 걸리는 시간(초)
const ROW_GAP: f32 = 6.0;
const SLIDE_SECONDS: f32 = 0.18;

// 줄을 비켜 그리는 거리를 부드럽게 바꿀 때 쓰는 이름
fn row_shift_id(path: &Path) -> egui::Id {
    egui::Id::unique(("row-shift", path))
}

// 목록의 위쪽에서 offset만큼 떨어진 곳이 몇 번째 자리에 가장 가까운지
fn nearest_slot(offset: f32, step: f32, count: usize) -> usize {
    let slot = (offset / step).round().max(0.0) as usize;
    slot.min(count.saturating_sub(1))
}

// from 번째 줄을 to 번째 자리로 옮길 때, index 번째 줄이 비켜야 하는 칸 수
fn shift_slots(index: usize, from: usize, to: usize) -> i32 {
    if from < index && index <= to {
        -1
    } else if to <= index && index < from {
        1
    } else {
        0
    }
}

// to 번째 자리에 놓으려면 move_item에 알려줄 "몇 번째 앞"
fn to_insert_position(from: usize, to: usize) -> usize {
    if to > from { to + 1 } else { to }
}

// 목록 한 줄의 모양을 정하는 값
#[derive(Clone, Copy)]
struct RowLook {
    // 편집 중이면 Some(이 파일을 골랐는지)
    checked: Option<bool>,
    // 재생 중인 파일인지
    current: bool,
    missing: bool,
}

// 목록 한 줄의 내용: 선택 표시, 확장자, 이름과 위치, 끌기 손잡이
fn row_content(ui: &mut egui::Ui, path: &Path, look: RowLook) {
    ui.set_width(ui.available_width());
    ui.horizontal(|ui| {
        if let Some(checked) = look.checked {
            icons::check_mark(ui, checked);
        }
        extension_badge(ui, path, look.current);

        // 편집 중에는 줄 끝에 끌기 손잡이
        let right_to_left = egui::Layout::right_to_left(egui::Align::Center);
        ui.with_layout(right_to_left, |ui| {
            if look.checked.is_some() {
                icons::drag_handle(ui);
            }

            // 파일 이름은 남은 공간에 왼쪽부터 표시
            let left_to_right = egui::Layout::left_to_right(egui::Align::Center);
            ui.with_layout(left_to_right, |ui| {
                let mut name =
                    egui::RichText::new(file_name(path)).color(ui.visuals().strong_text_color());
                // 재생 중인 곡은 강조색으로 표시
                if look.current {
                    name = name.strong().color(ui.visuals().hyperlink_color);
                }
                // 파일 이름 아래에 파일이 있는 위치
                let mut place = location(path);
                if look.missing {
                    place = tr!(
                        "파일을 찾을 수 없어요 · {place}",
                        "File not found · {place}",
                    );
                }
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 1.0;
                    ui.add(egui::Label::new(name).truncate());
                    let place = egui::RichText::new(place).size(11.0);
                    ui.add(egui::Label::new(place.weak()).truncate());
                });
            });
        });
    });
}

// 끌고 있는 줄을 다른 줄보다 위에, 그림자와 함께 그리기
fn floating_card(ui: &mut egui::Ui, rect: egui::Rect, path: &Path, look: RowLook) {
    let layer = egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("floating-row"));
    let builder = egui::UiBuilder::new()
        .id_salt("floating-row")
        .layer_id(layer)
        .max_rect(rect);
    let mut floating = ui.new_child(builder);
    floating.style_mut().interaction.selectable_labels = false;

    let shadow = egui::Shadow {
        offset: [0, 8],
        blur: 18,
        spread: 2,
        color: egui::Color32::from_black_alpha(120),
    };
    let colors = theme::palette(ui);
    file_card(&floating, true, false)
        .fill(colors.tint(0.18))
        .shadow(shadow)
        .show(&mut floating, |ui| row_content(ui, path, look));
}

// 끄는 중에 목록의 위나 아래 끝에 가까워지면 자동으로 스크롤
fn scroll_near_edge(ui: &egui::Ui, y: f32) {
    const EDGE: f32 = 30.0;
    const STEP: f32 = 8.0;
    let visible = ui.clip_rect();
    let delta = if y < visible.top() + EDGE {
        STEP
    } else if y > visible.bottom() - EDGE {
        -STEP
    } else {
        return;
    };
    ui.scroll_with_delta(egui::vec2(0.0, delta));
    ui.ctx().request_repaint();
}

// 목록의 파일 한 줄을 감싸는 카드. 재생 중이면 강조색으로 구별
fn file_card(ui: &egui::Ui, current: bool, hovered: bool) -> egui::Frame {
    let colors = theme::palette(ui);
    let (fill, line) = if current {
        (colors.tint(0.12), colors.point)
    } else if hovered {
        (colors.shade(0.11), colors.shade(0.28))
    } else {
        (colors.shade(0.05), colors.shade(0.11))
    };

    egui::Frame::new()
        .fill(fill)
        .stroke(egui::Stroke::new(1.0, line))
        .corner_radius(12)
        .inner_margin(egui::Margin::symmetric(10, 6))
}

// 파일 확장자를 글자로 보여주는 칸. 재생 중인 파일은 강조색
fn extension_badge(ui: &mut egui::Ui, path: &Path, current: bool) {
    let colors = theme::palette(ui);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(38.0, 28.0), egui::Sense::hover());
    let radius = egui::CornerRadius::same(8);
    let text_color = if current {
        ui.painter().rect_filled(rect, radius, colors.point);
        colors.on_point
    } else {
        let line = egui::Stroke::new(1.5, colors.shade(0.35));
        ui.painter()
            .rect_stroke(rect, radius, line, egui::StrokeKind::Inside);
        colors.ink
    };
    let extension = path
        .extension()
        .map(|ext| ext.to_string_lossy().to_uppercase())
        .unwrap_or_default();
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        extension,
        egui::FontId::proportional(11.0),
        text_color,
    );
}

// 설치한 프로그램에 들어 있는 license 문서의 위치. 없으면 None
fn notices_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let folder = exe.parent()?;
    // Windows는 실행 파일 옆, Mac은 .app 안의 Contents/Resources
    ["licenses", "../Resources/licenses"]
        .iter()
        .map(|place| folder.join(place).join("THIRD-PARTY-NOTICES.txt"))
        .find(|path| path.is_file())
}

// 개발용: scripts/make_notices.py를 실행해서 target 폴더에 license 문서 만들기.
// 함께 담는 도구(ffmpeg 등)의 license는 설치 파일에만 들어감
#[cfg(debug_assertions)]
fn make_dev_notices() -> Result<PathBuf, String> {
    let project = Path::new(env!("CARGO_MANIFEST_DIR"));
    let path = project.join("target/notices/THIRD-PARTY-NOTICES.txt");
    let python = if cfg!(windows) { "python" } else { "python3" };

    let output = Command::new(python)
        .arg("-I")
        .arg(project.join("scripts/make_notices.py"))
        .arg(&path)
        .output()
        .map_err(|e| tr!("python을 실행할 수 없어요: {e}", "cannot run python: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(stderr.lines().last().unwrap_or_default().to_string());
    }

    Ok(path)
}

// 파일의 확장자가 주어진 것과 같은지 (대소문자 구분 없음)
fn has_extension(path: &Path, extension: &str) -> bool {
    path.extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case(extension))
}

// 앱에서 재생하는 소리 파일의 확장자
const AUDIO_EXTENSIONS: [&str; 8] = ["mp3", "wav", "flac", "ogg", "m4a", "aac", "mid", "midi"];
// 컴퓨터의 기본 앱으로 여는 동영상 파일의 확장자
const VIDEO_EXTENSIONS: [&str; 7] = ["mp4", "mov", "m4v", "mkv", "webm", "avi", "wmv"];

fn is_video(path: &Path) -> bool {
    VIDEO_EXTENSIONS
        .iter()
        .any(|extension| has_extension(path, extension))
}

// 재생목록에 넣을 수 있는 파일인지
fn is_media(path: &Path) -> bool {
    is_video(path)
        || AUDIO_EXTENSIONS
            .iter()
            .any(|extension| has_extension(path, extension))
}

// 폴더 안의 음악, 동영상 파일을 최신순으로 가져오기
fn list_media(folder: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files: Vec<PathBuf> = fs::read_dir(folder)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && is_media(path))
        .collect();

    files.sort_by_key(|path| {
        let modified = fs::metadata(path).and_then(|m| m.modified()).ok();
        std::cmp::Reverse(modified)
    });

    Ok(files)
}

// from 번째 항목을 to 번째 항목의 앞으로 옮기기. 순서가 달라지면 true
fn move_item<T>(items: &mut Vec<T>, from: usize, to: usize) -> bool {
    // 자기 앞이나 바로 뒤에 놓으면 그대로
    if from >= items.len() || to == from || to == from + 1 {
        return false;
    }
    let item = items.remove(from);
    // 앞에서 하나를 빼면 뒤의 위치가 한 칸씩 당겨짐
    let to = if to > from { to - 1 } else { to };
    items.insert(to.min(items.len()), item);
    true
}

// 최신순 파일 목록에 사용자가 정한 순서 적용하기.
// 순서에 없는 새 파일은 맨 위에 최신순으로 둠
fn apply_order(files: Vec<PathBuf>, order: &[String]) -> Vec<PathBuf> {
    let place: HashMap<&str, usize> = order
        .iter()
        .enumerate()
        .map(|(index, name)| (name.as_str(), index))
        .collect();
    let mut files = files;
    // 정렬이 순서를 유지하므로 새 파일끼리는 최신순 그대로
    files.sort_by_key(|path| place.get(file_name(path).as_str()).map_or(0, |&i| i + 1));
    files
}

// 설정에 저장할 때 항목을 나누는 문자 (파일 이름에 쓰이지 않는 제어 문자)
const ENTRY: char = '\u{1e}';
const FIELD: char = '\u{1f}';

// 이전 version에서 저장한 폴더별 순서 읽기: "폴더, 이름, 이름..." 묶음의 나열
fn parse_orders(text: &str) -> HashMap<String, Vec<String>> {
    text.split(ENTRY)
        .filter_map(|entry| {
            let mut fields = entry.split(FIELD).map(str::to_string);
            let folder = fields.next().filter(|folder| !folder.is_empty())?;
            Some((folder, fields.collect()))
        })
        .collect()
}

// 재생목록을 설정에 저장할 글자로 바꾸기
fn format_playlist(files: &[PathBuf]) -> String {
    let paths: Vec<&str> = files.iter().filter_map(|path| path.to_str()).collect();
    paths.join(&ENTRY.to_string())
}

fn parse_playlist(text: &str) -> Vec<PathBuf> {
    text.split(ENTRY)
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .collect()
}

// 목록에 없는 파일만 순서대로 맨 앞에 넣고, 넣은 개수를 돌려주기
fn add_to_front(files: &mut Vec<PathBuf>, paths: Vec<PathBuf>) -> usize {
    let mut known: HashSet<PathBuf> = files.iter().cloned().collect();
    let new: Vec<PathBuf> = paths
        .into_iter()
        .filter(|path| known.insert(path.clone()))
        .collect();
    let count = new.len();
    files.splice(0..0, new);
    count
}

// 파일과 폴더들에서 음악과 동영상 파일 모으기. 폴더는 하위 폴더까지 이름순으로 찾음
fn collect_media(paths: &[PathBuf]) -> Vec<PathBuf> {
    // 너무 깊은 폴더는 찾지 않음
    const MAX_DEPTH: usize = 8;

    fn visit(path: &Path, depth: usize, found: &mut Vec<PathBuf>) {
        if path.is_file() {
            if is_media(path) {
                found.push(path.to_path_buf());
            }
            return;
        }
        let Ok(entries) = fs::read_dir(path) else {
            return;
        };
        let mut children: Vec<PathBuf> = entries
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                // 숨김 파일과, 제자리를 돌 수 있는 폴더 바로 가기는 제외
                let hidden = entry.file_name().to_string_lossy().starts_with('.');
                let linked_folder =
                    entry.file_type().is_ok_and(|t| t.is_symlink()) && entry.path().is_dir();
                !hidden && !linked_folder
            })
            .map(|entry| entry.path())
            .collect();
        children.sort();
        for child in children {
            if depth < MAX_DEPTH || child.is_file() {
                visit(&child, depth + 1, found);
            }
        }
    }

    let mut found = Vec::new();
    for path in paths {
        visit(path, 0, &mut found);
    }
    found
}

// 파일이 있는 폴더. 홈 폴더는 ~ 로 줄여서 보여주기
fn location(path: &Path) -> String {
    let folder = path.parent().unwrap_or(path);
    if let Some(home) = std::env::home_dir()
        && let Ok(rest) = folder.strip_prefix(&home)
    {
        let short = Path::new("~").join(rest);
        return short.display().to_string();
    }
    folder.display().to_string()
}

// 휴지통으로 옮겨도 되는지 확인 창으로 묻기
fn confirm_delete(paths: &[PathBuf]) -> bool {
    let question = match paths {
        [path] => {
            let name = file_name(path);
            tr!(
                "이 파일을 휴지통으로 옮길까요?\n\n{name}",
                "Move this file to the trash?\n\n{name}",
            )
        }
        _ => {
            // 파일이 많으면 앞의 몇 개만 보여주기
            const SHOWN: usize = 5;
            let count = paths.len();
            let mut names: Vec<String> = paths.iter().take(SHOWN).map(|p| file_name(p)).collect();
            if count > SHOWN {
                let more = count - SHOWN;
                names.push(tr!("그 밖에 {more}개", "and {more} more"));
            }
            let names = names.join("\n");
            tr!(
                "{count}개 파일을 휴지통으로 옮길까요?\n\n{names}",
                "Move {count} files to the trash?\n\n{names}",
            )
        }
    };

    let answer = rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Warning)
        .set_title(tr!("파일 삭제", "Delete files"))
        .set_description(question)
        .set_buttons(rfd::MessageButtons::YesNo)
        .show();
    answer == rfd::MessageDialogResult::Yes
}

// 파일을 휴지통으로 옮기기 (완전히 지우지 않으므로 되살릴 수 있음)
fn move_to_trash(path: &Path) -> Result<(), trash::Error> {
    #[allow(unused_mut)] // Mac이 아니면 바꿀 설정이 없음
    let mut context = trash::TrashContext::new();

    // Mac의 기본 방식은 Finder 제어 권한을 요청하므로, 권한이 필요 없는 방식 사용
    #[cfg(target_os = "macos")]
    {
        use trash::macos::{DeleteMethod, TrashContextExtMacos};
        context.set_delete_method(DeleteMethod::NsFileManager);
    }

    context.delete(path)
}

// 소리 출력 장치를 열고 파일 재생 시작하기
fn start_playing(path: PathBuf) -> Result<Playing, String> {
    let cannot_play = |e: String| {
        tr!(
            "재생할 수 없는 파일이에요: {e}",
            "cannot play this file: {e}"
        )
    };

    // MIDI는 소리를 만들어 가며 재생하고, 나머지는 파일의 소리를 풀어서 재생
    if has_extension(&path, "mid") || has_extension(&path, "midi") {
        let source = midi::MidiSource::open(&path).map_err(cannot_play)?;
        return play_source(path, source);
    }
    let file = File::open(&path)
        .map_err(|e| tr!("파일을 열 수 없어요: {e}", "cannot open the file: {e}"))?;
    let source = Decoder::try_from(file).map_err(|e| cannot_play(e.to_string()))?;
    play_source(path, source)
}

fn play_source(path: PathBuf, source: impl Source + Send + 'static) -> Result<Playing, String> {
    let total = source.total_duration();

    let mut device = DeviceSinkBuilder::open_default_sink().map_err(|e| {
        tr!(
            "소리 출력 장치를 열 수 없어요: {e}",
            "cannot open the audio output: {e}",
        )
    })?;
    // 정지할 때 터미널에 안내 문구가 나오지 않도록 끄기
    device.log_on_drop(false);

    let player = Player::connect_new(device.mixer());
    player.append(source);

    Ok(Playing {
        path,
        player,
        total,
        seeking: None,
        _device: device,
    })
}

// 경로에서 파일 이름만 꺼내기
fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

// 곡 목록에서 current의 다음(forward) 또는 이전 곡.
// wrap이면 목록의 끝에서 반대쪽 끝으로 이어짐. current가 목록에 없으면 None
fn step_song(songs: &[PathBuf], current: &Path, forward: bool, wrap: bool) -> Option<PathBuf> {
    let index = songs.iter().position(|path| path == current)?;
    let next = if forward {
        (index + 1 < songs.len())
            .then_some(index + 1)
            .or(wrap.then_some(0))
    } else {
        index.checked_sub(1).or(wrap.then_some(songs.len() - 1))
    };
    next.map(|i| songs[i].clone())
}

// 쌓아 둔 곡을 최근 것부터 꺼내서, 지금도 목록에 있는 첫 번째 곡
fn pop_existing(stack: &mut Vec<PathBuf>, songs: &[PathBuf]) -> Option<PathBuf> {
    std::iter::from_fn(|| stack.pop()).find(|path| songs.contains(path))
}

// 아직 지나오지 않은 곡
fn unplayed(songs: &[PathBuf], history: &[PathBuf]) -> Vec<PathBuf> {
    songs
        .iter()
        .filter(|path| !history.contains(path))
        .cloned()
        .collect()
}

// 옛날 MP3 플레이어처럼 글자를 왼쪽으로 반복해서 흘려 보내기.
// elapsed는 흐르기 시작한 뒤 지난 시간
fn marquee(ui: &mut egui::Ui, text: &str, elapsed: Duration) {
    // 1초에 움직이는 거리와, 글자가 다시 나오기까지의 간격
    const SPEED: f32 = 40.0;
    const GAP: f32 = 60.0;

    let font = egui::TextStyle::Body.resolve(ui.style());
    let color = ui.visuals().strong_text_color();
    let galley = ui.painter().layout_no_wrap(text.to_string(), font, color);

    let size = egui::vec2(ui.available_width(), galley.size().y);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::hover());
    // 양끝에서 글자가 서서히 사라지는 폭
    const FADE: f32 = 24.0;
    // 짧은 제목은 화면에서 다 빠져나간 뒤에 다시 나오게 하기
    let period = (galley.size().x + GAP).max(rect.width());
    let offset = (elapsed.as_secs_f32() * SPEED) % period;

    // 영역 밖으로 나간 부분은 잘라내기
    let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    // 처음에 첫 글자가 흐린 부분에 가리지 않도록 그만큼 안쪽에서 시작
    let first = rect.left() + FADE - offset;
    for start in [first, first + period] {
        painter.galley(egui::pos2(start, rect.top()), galley.clone(), color);
    }

    // 글자가 뚝 잘려 보이지 않게 양끝을 바탕색으로 흐리게 덮기
    let background = theme::palette(ui).edge;
    fade(&painter, rect, FADE, background, true);
    fade(&painter, rect, FADE, background, false);

    // 마우스를 올리면 전체 제목 보여주기
    response.on_hover_text(text);
}

// rect의 왼쪽 또는 오른쪽 끝에, 바깥으로 갈수록 background가 진해지는 띠 그리기
fn fade(
    painter: &egui::Painter,
    rect: egui::Rect,
    width: f32,
    background: egui::Color32,
    left: bool,
) {
    let (outer, inner) = if left {
        (rect.left(), rect.left() + width)
    } else {
        (rect.right(), rect.right() - width)
    };
    let mut mesh = egui::Mesh::default();
    for (x, color) in [(outer, background), (inner, egui::Color32::TRANSPARENT)] {
        mesh.colored_vertex(egui::pos2(x, rect.top()), color);
        mesh.colored_vertex(egui::pos2(x, rect.bottom()), color);
    }
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(1, 2, 3);
    painter.add(mesh);
}

// 시간을 "분:초" 모양으로 바꾸기
fn format_time(time: Duration) -> String {
    let seconds = time.as_secs();
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_progress_lines() {
        assert_eq!(parse_progress("[progress]  50.0%"), Some(0.5));
        assert_eq!(parse_progress("[progress]100%"), Some(1.0));
        assert_eq!(parse_progress("[progress] N/A"), None);
        assert_eq!(parse_progress("[download] Destination: a.webm"), None);
    }

    #[test]
    fn steps_through_songs() {
        let songs = ["a.mp3", "b.mp3", "c.mp3"].map(PathBuf::from);
        let step = |from: usize, forward, wrap| step_song(&songs, &songs[from], forward, wrap);
        assert_eq!(step(0, true, false), Some(songs[1].clone()));
        assert_eq!(step(2, true, false), None);
        assert_eq!(step(2, true, true), Some(songs[0].clone()));
        assert_eq!(step(1, false, false), Some(songs[0].clone()));
        assert_eq!(step(0, false, false), None);
        assert_eq!(step(0, false, true), Some(songs[2].clone()));
        assert_eq!(step_song(&songs, Path::new("x.mp3"), true, true), None);
        assert_eq!(step_song(&[], Path::new("x.mp3"), false, true), None);
    }

    #[test]
    fn picks_only_unplayed_songs() {
        let songs = ["a.mp3", "b.mp3", "c.mp3"].map(PathBuf::from);
        assert_eq!(unplayed(&songs, &songs[..2]), vec![songs[2].clone()]);
        assert!(unplayed(&songs, &songs).is_empty());
    }

    #[test]
    fn moves_items() {
        let moved = |from, to| {
            let mut items = vec!['a', 'b', 'c', 'd'];
            let changed = move_item(&mut items, from, to);
            (changed, items.into_iter().collect::<String>())
        };
        assert_eq!(moved(0, 4), (true, "bcda".into()));
        assert_eq!(moved(3, 0), (true, "dabc".into()));
        assert_eq!(moved(1, 3), (true, "acbd".into()));
        assert_eq!(moved(1, 1), (false, "abcd".into()));
        assert_eq!(moved(1, 2), (false, "abcd".into()));
        assert_eq!(moved(9, 0), (false, "abcd".into()));
    }

    #[test]
    fn keeps_custom_order_and_puts_new_files_first() {
        let files = ["new2", "new1", "a", "b", "c"].map(PathBuf::from).to_vec();
        let order = ["c", "deleted", "a", "b"].map(String::from);
        let result = apply_order(files, &order);
        assert_eq!(result, ["new2", "new1", "c", "a", "b"].map(PathBuf::from));
    }

    #[test]
    fn reads_old_orders() {
        let text = format!("/music{FIELD}b.mp3{FIELD}a.mp3{ENTRY}/other{FIELD}c, d.mp4");
        let orders = parse_orders(&text);
        assert_eq!(orders["/music"], ["b.mp3", "a.mp3"]);
        assert_eq!(orders["/other"], ["c, d.mp4"]);
        assert!(parse_orders("").is_empty());
    }

    #[test]
    fn saves_and_reads_playlist() {
        let files = ["/a b/x.mp3", "/c, d/y.mp4"].map(PathBuf::from).to_vec();
        assert_eq!(parse_playlist(&format_playlist(&files)), files);
        assert!(parse_playlist("").is_empty());
    }

    #[test]
    fn adds_only_new_files_to_front() {
        let mut files = vec![PathBuf::from("old.mp3")];
        let new = ["a.mp3", "old.mp3", "b.mp3", "a.mp3"]
            .map(PathBuf::from)
            .to_vec();
        assert_eq!(add_to_front(&mut files, new), 2);
        assert_eq!(files, ["a.mp3", "b.mp3", "old.mp3"].map(PathBuf::from));
    }

    #[test]
    fn recognizes_media_files() {
        for name in [
            "a.mp3", "b.FLAC", "c.m4a", "d.wav", "e.ogg", "f.aac", "g.mid", "h.MIDI",
        ] {
            let path = Path::new(name);
            assert!(is_media(path) && !is_video(path), "{name}");
        }
        for name in [
            "a.mp4", "b.MOV", "c.mkv", "d.webm", "e.avi", "f.wmv", "g.m4v",
        ] {
            let path = Path::new(name);
            assert!(is_media(path) && is_video(path), "{name}");
        }
        for name in ["a.kar", "b.txt", "c.aiff", "mp3", "d.mp3.part"] {
            assert!(!is_media(Path::new(name)), "{name}");
        }
    }

    #[test]
    fn shows_longer_messages_longer() {
        let seconds = |length: usize| Toast::new("가".repeat(length)).seconds();
        assert_eq!(seconds(5), 2.0);
        assert_eq!(seconds(42), 3.0);
        assert_eq!(seconds(500), 5.0);
    }

    #[test]
    fn rows_make_room_for_the_dragged_row() {
        // 줄 0을 자리 2로: 1, 2번 줄이 한 칸 위로
        let shifts = |from, to| (0..4).map(|i| shift_slots(i, from, to)).collect::<Vec<_>>();
        assert_eq!(shifts(0, 2), [0, -1, -1, 0]);
        assert_eq!(shifts(3, 1), [0, 1, 1, 0]);
        assert_eq!(shifts(2, 2), [0, 0, 0, 0]);

        assert_eq!(nearest_slot(-40.0, 54.0, 4), 0);
        assert_eq!(nearest_slot(80.0, 54.0, 4), 1);
        assert_eq!(nearest_slot(900.0, 54.0, 4), 3);

        // 자리 번호대로 옮겨지는지 확인
        for from in 0..4 {
            for to in 0..4 {
                let mut items = vec![0, 1, 2, 3];
                move_item(&mut items, from, to_insert_position(from, to));
                assert_eq!(items[to], from, "{from} -> {to}");
            }
        }
    }

    #[test]
    fn formats_time() {
        assert_eq!(format_time(Duration::from_secs(0)), "00:00");
        assert_eq!(format_time(Duration::from_secs(125)), "02:05");
    }
}
