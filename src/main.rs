// Windows 배포용 실행 파일은 뒤에 콘솔 창이 뜨지 않도록 설정
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

// tr! 매크로를 아래 코드에서 쓰려면 가장 먼저 선언해야 함
#[macro_use]
mod lang;

mod icons;
mod theme;

use eframe::egui;
use icons::Icon;
use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player, Source};
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
            .with_icon(icon),
        ..Default::default()
    };

    eframe::run_native(
        &app_name(),
        options,
        Box::new(|cc| {
            theme::apply(&cc.egui_ctx);
            let app = App::new(cc.storage);
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
    tr!("걍담아", "JustGrab")
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

fn supported_sites(ui: &mut egui::Ui) {
    ui.set_max_width(340.0);
    let strong = ui.visuals().strong_text_color();

    ui.label(
        egui::RichText::new(tr!("지원 사이트", "Supported sites"))
            .size(15.0)
            .strong()
            .color(ui.visuals().hyperlink_color),
    );
    ui.add_space(4.0);
    ui.label(egui::RichText::new(SITES).color(strong));
    ui.separator();
    ui.label(
        egui::RichText::new(tr!(
            "• SoundCloud는 소리만 받을 수 있어요\n\
             • 게시물에 따라 받지 못할 수 있어요\n\
             • 로그인이 필요한 콘텐츠는 받을 수 없어요\n\
             • 비공개, DRM 콘텐츠는 받을 수 없어요",
            "• SoundCloud provides audio only\n\
             • Some posts may fail to download\n\
             • Login-only content is not supported\n\
             • Private or DRM content is not supported",
        ))
        .color(strong),
    );
}

// 화면이 기억해야 하는 값들
struct App {
    url: String,
    folder: PathBuf,
    status: String,
    // 화면 하단에 보여줄 삭제 결과
    notice: String,
    // 다운로드 중이면 Some, 아니면 None
    job: Option<Job>,
    // 저장 폴더에 있는 mp3, mp4 목록
    files: Vec<PathBuf>,
    // 재생 중이면 Some, 아니면 None
    playing: Option<Playing>,
    // 음량 (0.0 ~ 1.0)
    volume: f32,
    // 곡이 끝나면 목록의 다음 곡을 이어서 재생할지
    auto_next: bool,
    // 다운로드할 때 저장할 형식
    format: Format,
    // 정보창에 보여줄 앱 아이콘. 처음 열 때 만듦
    icon_texture: Option<egui::TextureHandle>,
    // 사용자가 언어를 직접 골랐는지. 고르지 않았으면 시스템 언어를 계속 따름
    language_chosen: bool,
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
    Done(Result<(), String>),
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
        let auto_next = saved("auto_next").is_none_or(|v| v != "false");
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
            notice: String::new(),
            job: None,
            files: Vec::new(),
            playing: None,
            volume,
            auto_next,
            format,
            language_chosen,
            icon_texture: None,
        };
        app.refresh_files();
        app
    }

    // 화면 상단: 왼쪽에 앱 이름, 오른쪽에 언어 선택 버튼
    fn title_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(app_name()).heading().strong());

            // 정보 버튼: 누르면 열리고, 바깥을 누르면 닫힘
            let info = icons::button(ui, Icon::Info, tr!("프로그램 정보", "About"));
            let frame = popup_frame(ui);
            egui::Popup::from_toggle_button_response(&info)
                .frame(frame)
                .show(|ui| self.about(ui));

            let right_to_left = egui::Layout::right_to_left(egui::Align::Center);
            ui.with_layout(right_to_left, |ui| {
                let korean = lang::is_korean();
                // 오른쪽부터 배치하므로 English를 먼저 추가
                if ui.selectable_label(!korean, "English").clicked() {
                    self.set_language(ui.ctx(), false);
                }
                if ui.selectable_label(korean, "한국어").clicked() {
                    self.set_language(ui.ctx(), true);
                }
            });
        });
    }

    // 프로그램 정보: 아이콘, 이름, version, 저작권, 오픈소스 license
    fn about(&mut self, ui: &mut egui::Ui) {
        ui.set_width(260.0);
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
            ui.add_space(4.0);
            ui.label(egui::RichText::new(copyright()).size(11.0).color(strong));
            ui.separator();
            let licenses = tr!("오픈소스 라이선스", "Open source licenses");
            if ui.link(licenses).clicked() {
                self.open_licenses();
            }
        });
    }

    fn set_language(&mut self, ctx: &egui::Context, korean: bool) {
        lang::set_korean(korean);
        self.language_chosen = true;
        // 이미 만들어 둔 안내 문구는 이전 언어이므로 지우기
        // (다운로드 중의 상태는 다음 소식을 받을 때 새 언어로 바뀜)
        if self.job.is_none() {
            self.status.clear();
        }
        self.notice.clear();
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(app_name()));
    }

    // 저장 폴더의 파일 목록 다시 읽기
    fn refresh_files(&mut self) {
        match list_media(&self.folder) {
            Ok(files) => self.files = files,
            Err(e) => {
                self.files.clear();
                self.status = tr!(
                    "오류: 폴더를 읽을 수 없어요: {e}",
                    "Error: cannot read the folder: {e}"
                );
            }
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
            Err(message) => self.status = tr!("오류: {message}", "Error: {message}"),
        }
    }

    // 동영상은 PC의 기본 player로 열기
    fn open_video(&mut self, path: &Path) {
        // 소리가 겹치지 않도록 재생 중인 곡은 일시정지
        if let Some(playing) = &self.playing {
            playing.player.pause();
        }
        if let Err(e) = open::that_detached(path) {
            self.status = tr!(
                "오류: 동영상을 열 수 없어요: {e}",
                "Error: cannot open the video: {e}"
            );
        }
    }

    // 곡이 끝까지 재생됐으면 정리하고, 설정에 따라 다음 곡 재생하기
    fn check_playing(&mut self) {
        let Some(playing) = &self.playing else {
            return;
        };
        if !playing.player.empty() {
            return;
        }

        let next = next_file(&self.files, &playing.path);
        self.playing = None;

        if let Some(path) = next.filter(|_| self.auto_next) {
            self.play(path);
        }
    }

    // 화면 아래의 재생 bar: 일시정지/정지, 곡 이름, 음량, 탐색 bar
    fn player_bar(&mut self, ui: &mut egui::Ui) {
        let Some(playing) = &mut self.playing else {
            return;
        };

        // slider의 채워진 부분은 진한 강조색, 빈 부분은 바탕과 구별되는 색
        let colors = theme::palette(ui);
        ui.visuals_mut().selection.bg_fill = colors.point;
        // 손잡이에도 쓰이는 색이므로 불투명하게
        ui.visuals_mut().widgets.inactive.bg_fill = colors.edge.lerp_to_gamma(colors.ink, 0.2);

        let mut stop = false;
        ui.horizontal(|ui| {
            if playing.player.is_paused() {
                if icons::button(ui, Icon::Play, tr!("계속 재생", "Resume")).clicked() {
                    playing.player.play();
                }
            } else if icons::button(ui, Icon::Pause, tr!("일시정지", "Pause")).clicked() {
                playing.player.pause();
            }
            stop = icons::button(ui, Icon::Stop, tr!("정지", "Stop")).clicked();

            // 음량은 오른쪽 끝에 두고, 곡 이름은 남은 공간에 표시
            let right_to_left = egui::Layout::right_to_left(egui::Align::Center);
            ui.with_layout(right_to_left, |ui| {
                ui.spacing_mut().slider_width = 80.0;
                // 손잡이가 너무 커지지 않도록 slider는 낮게
                ui.spacing_mut().interact_size.y = 18.0;
                let percent = self.volume * 100.0;
                let slider = egui::Slider::new(&mut self.volume, 0.0..=1.0)
                    .show_value(false)
                    .trailing_fill(true);
                if ui
                    .add(slider)
                    .on_hover_text(format!("{percent:.0}%"))
                    .changed()
                {
                    playing.player.set_volume(self.volume);
                }
                icons::picture(ui, Icon::Speaker, tr!("음량", "Volume"));

                let left_to_right = egui::Layout::left_to_right(egui::Align::Center);
                ui.with_layout(left_to_right, |ui| {
                    let name = egui::RichText::new(file_name(&playing.path)).strong();
                    ui.add(egui::Label::new(name).truncate());
                });
            });
        });

        ui.spacing_mut().interact_size.y = 18.0;
        ui.horizontal(|ui| {
            let position = playing.player.get_pos();

            // 전체 길이를 모르면 탐색할 수 없으므로 재생 시간만 표시
            let Some(total) = playing.total else {
                ui.label(format_time(position));
                return;
            };

            let total_secs = total.as_secs_f32();
            // 끄는 중에는 실제 재생 위치 대신 끌고 있는 위치를 보여주기
            let mut seconds = playing
                .seeking
                .unwrap_or_else(|| position.as_secs_f32().min(total_secs));

            ui.label(format_time(Duration::from_secs_f32(seconds)));

            let right_to_left = egui::Layout::right_to_left(egui::Align::Center);
            ui.with_layout(right_to_left, |ui| {
                ui.label(format_time(total));

                // 두 시간 표시 사이를 탐색 bar로 채우기
                ui.spacing_mut().slider_width = ui.available_width();
                let slider = ui.add(
                    egui::Slider::new(&mut seconds, 0.0..=total_secs)
                        .show_value(false)
                        .trailing_fill(true),
                );

                if slider.dragged() {
                    playing.seeking = Some(seconds);
                } else if slider.drag_stopped() || slider.changed() {
                    // 마우스를 놓았을 때 한 번만 이동하기
                    playing.seeking = None;
                    if let Err(e) = playing.player.try_seek(Duration::from_secs_f32(seconds)) {
                        self.status = tr!("오류: 이동할 수 없어요: {e}", "Error: cannot seek: {e}");
                    }
                }
            });
        });

        if stop {
            self.playing = None;
        }
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
                self.notice = tr!(
                    "오류: 라이선스 문서를 열 수 없어요: {e}",
                    "Error: cannot open the license document: {e}",
                );
                return;
            }
        };
        if let Err(e) = open::that_detached(path) {
            self.notice = tr!(
                "오류: 라이선스 문서를 열 수 없어요: {e}",
                "Error: cannot open the license document: {e}",
            );
        }
    }

    // 화면 위쪽: URL 입력, 저장 버튼, 저장 형식과 위치
    fn download_form(&mut self, ui: &mut egui::Ui) {
        let has_url = !self.url.trim().is_empty();
        let mut clicked = false;
        let mut entered = false;

        // 저장 버튼과 도움말을 오른쪽에 두고, 입력창이 남은 폭을 채우기
        let row = egui::vec2(ui.available_width(), 32.0);
        let right_to_left = egui::Layout::right_to_left(egui::Align::Center);
        ui.allocate_ui_with_layout(row, right_to_left, |ui| {
            let label = tr!("저장하기", "Save");
            clicked = icons::primary_button(ui, Icon::Download, label, has_url).clicked();

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

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.scope(|ui| {
                accent_marks(ui);
                ui.radio_value(
                    &mut self.format,
                    Format::Mp3,
                    tr!("MP3 (소리)", "MP3 (audio)"),
                );
                ui.radio_value(
                    &mut self.format,
                    Format::Mp4,
                    tr!("MP4 (동영상)", "MP4 (video)"),
                );
            });

            // 지원 사이트 안내: 누르면 열리고, 바깥을 누르면 닫힘
            let sites = tr!("지원 사이트", "Supported sites");
            let link = ui.link(egui::RichText::new(sites).size(11.0));

            let frame = popup_frame(ui);
            egui::Popup::from_toggle_button_response(&link)
                .frame(frame)
                .show(supported_sites);

            let right_to_left = egui::Layout::right_to_left(egui::Align::Center);
            ui.with_layout(right_to_left, |ui| {
                let change = tr!("저장 폴더 변경", "Change folder");
                if icons::button(ui, Icon::Folder, change).clicked() {
                    // 취소하면 None이므로 기존 폴더를 그대로 유지
                    if let Some(folder) = rfd::FileDialog::new()
                        .set_directory(&self.folder)
                        .pick_folder()
                    {
                        self.folder = folder;
                        self.refresh_files();
                    }
                }
                // 경로가 길면 줄이고, 마우스를 올리면 전체 경로 보여주기
                let path = self.folder.display().to_string();
                ui.add(egui::Label::new(egui::RichText::new(&path).weak()).truncate())
                    .on_hover_text(path);
            });
        });

        if clicked || (entered && has_url) {
            self.start_download(ui.ctx().clone());
        }
    }

    // 확인을 받은 뒤 파일을 휴지통으로 옮기기
    fn delete(&mut self, path: PathBuf) {
        let name = file_name(&path);
        let answer = rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Warning)
            .set_title(tr!("파일 삭제", "Delete file"))
            .set_description(tr!(
                "이 파일을 휴지통으로 옮길까요?\n\n{name}",
                "Move this file to the trash?\n\n{name}"
            ))
            .set_buttons(rfd::MessageButtons::YesNo)
            .show();
        if answer != rfd::MessageDialogResult::Yes {
            return;
        }

        // 지우려는 곡이 재생 중이면 먼저 정지 (Windows는 열린 파일을 옮길 수 없음)
        if self.playing.as_ref().is_some_and(|p| p.path == path) {
            self.playing = None;
        }

        self.notice = match move_to_trash(&path) {
            Ok(()) => tr!("휴지통으로 옮겼어요: {name}", "Moved to trash: {name}"),
            Err(e) => tr!("오류: 삭제할 수 없어요: {e}", "Error: cannot delete: {e}"),
        };
        self.refresh_files();
    }

    // 저장된 mp3 목록 보여주기
    fn file_list(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let count = self.files.len();
            let title = tr!("저장된 목록 ({count})", "Saved files ({count})");
            ui.label(egui::RichText::new(title).strong());

            let right_to_left = egui::Layout::right_to_left(egui::Align::Center);
            ui.with_layout(right_to_left, |ui| {
                if icons::button(ui, Icon::Refresh, tr!("새로고침", "Refresh")).clicked() {
                    self.refresh_files();
                }
                ui.scope(|ui| {
                    accent_marks(ui);
                    ui.checkbox(
                        &mut self.auto_next,
                        tr!("다음 곡 자동 재생", "Play next automatically"),
                    );
                });
            });
        });

        // 목록을 그리는 동안에는 클릭한 파일만 기억하고, 재생과 삭제는 그린 뒤에 하기
        let mut clicked = None;
        let mut to_delete = None;
        let mut toggle_pause = false;
        // 기본 스크롤 bar는 평소에 숨어 있으므로, 목록이 넘치면 항상 보이는 모양 사용
        ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            // 기본 최소 높이(64)를 없애서, 공간이 좁아도 하단 영역 뒤로 넘치지 않게 하기
            .min_scrolled_height(0.0)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 6.0;
                for path in &self.files {
                    let video = is_video(path);
                    // 이 파일이 재생 중이면 Some(일시정지 여부)
                    let current = self
                        .playing
                        .as_ref()
                        .filter(|p| &p.path == path)
                        .map(|p| p.player.is_paused());

                    file_card(ui, current.is_some()).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            // 앞의 아이콘이 재생 버튼
                            let kind = if video { Icon::Video } else { Icon::Music };
                            let hint = match current {
                                Some(false) => tr!("일시정지", "Pause"),
                                Some(true) => tr!("계속 재생", "Resume"),
                                None if video => {
                                    tr!("기본 player로 열기", "Open in default player")
                                }
                                None => tr!("재생", "Play"),
                            };
                            if icons::file_button(ui, kind, current, hint).clicked() {
                                if current.is_some() {
                                    toggle_pause = true;
                                } else {
                                    clicked = Some(path.clone());
                                }
                            }

                            // 삭제 버튼은 오른쪽 끝에 배치
                            let right_to_left = egui::Layout::right_to_left(egui::Align::Center);
                            ui.with_layout(right_to_left, |ui| {
                                let delete = tr!("삭제", "Delete");
                                if icons::button(ui, Icon::Trash, delete).clicked() {
                                    to_delete = Some(path.clone());
                                }

                                // 파일 이름은 남은 공간에 왼쪽부터 표시
                                let left_to_right =
                                    egui::Layout::left_to_right(egui::Align::Center);
                                ui.with_layout(left_to_right, |ui| {
                                    let mut name = egui::RichText::new(file_name(path))
                                        .color(ui.visuals().strong_text_color());
                                    // 재생 중인 곡은 강조색으로 표시
                                    if current.is_some() {
                                        name = name.strong().color(ui.visuals().hyperlink_color);
                                    }
                                    ui.add(egui::Label::new(name).truncate());
                                });
                            });
                        });
                    });
                }
            });

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
                self.play(path);
            }
        }
        if let Some(path) = to_delete {
            self.delete(path);
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
        self.notice.clear();
    }

    // 다운로드 thread가 보낸 소식 확인하기 (기다리지 않음)
    fn check_download(&mut self) {
        let Some(job) = &mut self.job else {
            return;
        };

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
                Ok(Update::Done(Ok(()))) => {
                    let folder = self.folder.display();
                    break tr!("저장 완료! 위치: {folder}", "Saved to: {folder}");
                }
                Ok(Update::Done(Err(message))) => break tr!("오류: {message}", "Error: {message}"),
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
        self.refresh_files();
    }
}

impl eframe::App for App {
    // 화면 그리기 전에 처리할 일. 창이 가려져 있어도 호출됨
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.check_download();
        self.check_playing();

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
        storage.set_string("auto_next", self.auto_next.to_string());
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

        egui::Panel::top("title")
            .frame(edge_frame(ui))
            .show(ui, |ui| self.title_bar(ui));

        // 아래쪽 영역은 나중에 배치한 것이 위에 놓임.
        // 남은 공간을 쓰는 CentralPanel보다 먼저 배치해야 함

        // 재생 중일 때만 보이는 재생 bar
        if self.playing.is_some() {
            egui::Panel::bottom("player")
                .default_size(56.0)
                .frame(edge_frame(ui))
                .show(ui, |ui| self.player_bar(ui));
        }

        // 삭제 결과 등의 안내는 분리선 위에 표시
        if !self.notice.is_empty() {
            egui::Panel::bottom("notice")
                .show_separator_line(false)
                .show(ui, |ui| {
                    // 파일 이름이 길면 한 줄에 맞게 줄이기
                    ui.add(egui::Label::new(&self.notice).truncate());
                });
        }

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
) -> Result<(), String> {
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
        .args(["--no-playlist", "--no-mtime"])
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
    for line in BufReader::new(stdout).split(b'\n') {
        let Ok(line) = line else {
            break;
        };
        let line = String::from_utf8_lossy(&line);

        if let Some(progress) = parse_progress(&line) {
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
        return Err(tr!(
            "다운로드에 실패했어요\n{reason}",
            "Download failed\n{reason}"
        ));
    }

    Ok(())
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

// 헤더와 재생 bar의 틀. 본문과 구별되는 바탕색 사용
fn edge_frame(ui: &egui::Ui) -> egui::Frame {
    egui::Frame::side_top_panel(ui.style()).fill(theme::palette(ui).edge)
}

// 안내창의 틀. 뒤의 화면과 구별되도록 강조색 테두리와 조금 밝은 바탕 사용
fn popup_frame(ui: &egui::Ui) -> egui::Frame {
    let colors = theme::palette(ui);
    egui::Frame::popup(ui.style())
        .fill(colors.shade(0.08))
        .stroke(egui::Stroke::new(2.0, colors.point))
        .inner_margin(14)
}

// radio와 checkbox의 선택 표시는 강조색, 글자는 원래 색
fn accent_marks(ui: &mut egui::Ui) {
    let colors = theme::palette(ui);
    let visuals = ui.visuals_mut();
    visuals.override_text_color = Some(colors.ink);
    for widget in [
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
    ] {
        widget.fg_stroke.color = colors.point;
    }
}

// 목록의 파일 한 줄을 감싸는 카드. 재생 중이면 강조색으로 구별
fn file_card(ui: &egui::Ui, current: bool) -> egui::Frame {
    let colors = theme::palette(ui);
    let (fill, line) = if current {
        (colors.tint(0.12), colors.point)
    } else {
        (colors.shade(0.05), colors.shade(0.11))
    };

    egui::Frame::new()
        .fill(fill)
        .stroke(egui::Stroke::new(1.0, line))
        .corner_radius(12)
        .inner_margin(egui::Margin::symmetric(10, 6))
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

fn is_video(path: &Path) -> bool {
    has_extension(path, "mp4")
}

// 폴더 안의 mp3, mp4 파일을 최신순으로 가져오기
fn list_media(folder: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files: Vec<PathBuf> = fs::read_dir(folder)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && (has_extension(path, "mp3") || is_video(path)))
        .collect();

    files.sort_by_key(|path| {
        let modified = fs::metadata(path).and_then(|m| m.modified()).ok();
        std::cmp::Reverse(modified)
    });

    Ok(files)
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
    let file = File::open(&path)
        .map_err(|e| tr!("파일을 열 수 없어요: {e}", "cannot open the file: {e}"))?;
    let source = Decoder::try_from(file).map_err(|e| {
        tr!(
            "재생할 수 없는 파일이에요: {e}",
            "cannot play this file: {e}"
        )
    })?;

    let total = source.total_duration();

    let mut device = DeviceSinkBuilder::open_default_sink().map_err(|e| {
        tr!(
            "소리 출력 장치를 열 수 없어요: {e}",
            "cannot open the audio output: {e}"
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

// 목록에서 current 다음에 있는 곡(동영상은 건너뜀). 마지막 곡이거나 목록에 없으면 None
fn next_file(files: &[PathBuf], current: &Path) -> Option<PathBuf> {
    let index = files.iter().position(|path| path == current)?;
    files[index + 1..]
        .iter()
        .find(|path| !is_video(path))
        .cloned()
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
    fn finds_next_file() {
        let files = [PathBuf::from("a.mp3"), PathBuf::from("b.mp3")];
        assert_eq!(next_file(&files, &files[0]), Some(files[1].clone()));
        assert_eq!(next_file(&files, &files[1]), None);
        assert_eq!(next_file(&files, Path::new("c.mp3")), None);
    }

    #[test]
    fn skips_videos_when_finding_next_file() {
        let files = ["a.mp3", "b.MP4", "c.mp3", "d.mp4"].map(PathBuf::from);
        assert_eq!(next_file(&files, &files[0]), Some(files[2].clone()));
        assert_eq!(next_file(&files, &files[2]), None);
    }

    #[test]
    fn formats_time() {
        assert_eq!(format_time(Duration::from_secs(0)), "00:00");
        assert_eq!(format_time(Duration::from_secs(125)), "02:05");
    }
}
