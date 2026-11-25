// Windows 배포용 실행 파일은 뒤에 콘솔 창이 뜨지 않도록 설정
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use eframe::egui;
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
    // 창과 Dock에 보일 아이콘. 지정하지 않으면 eframe 기본 아이콘이 쓰임
    let icon = eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png"))
        .expect("프로그램에 포함한 아이콘은 올바른 PNG");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            // 설정이 저장되는 폴더 이름으로도 쓰임
            .with_app_id("yt-mp3")
            .with_inner_size([560.0, 560.0])
            .with_icon(icon),
        ..Default::default()
    };

    eframe::run_native(
        "YouTube → MP3",
        options,
        Box::new(|cc| Ok(Box::new(App::new(cc.storage)))),
    )
}

// 화면 하단에 보여줄 저작권 정보
const COPYRIGHT: &str = "© 2026 아이티해결단(이혁). All rights reserved.";

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
        };
        app.refresh_files();
        app
    }

    // 저장 폴더의 파일 목록 다시 읽기
    fn refresh_files(&mut self) {
        match list_media(&self.folder) {
            Ok(files) => self.files = files,
            Err(e) => {
                self.files.clear();
                self.status = format!("오류: 폴더를 읽을 수 없어요: {e}");
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
            Err(message) => self.status = format!("오류: {message}"),
        }
    }

    // 동영상은 PC의 기본 player로 열기
    fn open_video(&mut self, path: &Path) {
        // 소리가 겹치지 않도록 재생 중인 곡은 일시정지
        if let Some(playing) = &self.playing {
            playing.player.pause();
        }
        if let Err(e) = open::that_detached(path) {
            self.status = format!("오류: 동영상을 열 수 없어요: {e}");
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

    // 재생 조작 부분: 음량, 자동 재생, 탐색 bar, 일시정지/정지 버튼
    fn player_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("음량");
            let slider = egui::Slider::new(&mut self.volume, 0.0..=1.0)
                .custom_formatter(|v, _| format!("{:.0}%", v * 100.0))
                .custom_parser(|text| {
                    let number = text.trim().trim_end_matches('%').parse::<f64>().ok()?;
                    Some(number / 100.0)
                });
            if ui.add(slider).changed()
                && let Some(playing) = &self.playing
            {
                playing.player.set_volume(self.volume);
            }
            ui.checkbox(&mut self.auto_next, "다음 곡 자동 재생");
        });

        let Some(playing) = &mut self.playing else {
            return;
        };

        let mut stop = false;
        ui.horizontal(|ui| {
            if playing.player.is_paused() {
                if ui.button(symbol("►")).on_hover_text("계속 재생").clicked() {
                    playing.player.play();
                }
            } else if ui.button(symbol("‖")).on_hover_text("일시정지").clicked() {
                playing.player.pause();
            }
            stop = ui.button(symbol("■")).on_hover_text("정지").clicked();

            ui.add(egui::Label::new(file_name(&playing.path)).truncate());
        });

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

            ui.label(format!(
                "{} / {}",
                format_time(Duration::from_secs_f32(seconds)),
                format_time(total)
            ));

            ui.spacing_mut().slider_width = ui.available_width();
            let slider =
                ui.add(egui::Slider::new(&mut seconds, 0.0..=total_secs).show_value(false));

            if slider.dragged() {
                playing.seeking = Some(seconds);
            } else if slider.drag_stopped() || slider.changed() {
                // 마우스를 놓았을 때 한 번만 이동하기
                playing.seeking = None;
                if let Err(e) = playing.player.try_seek(Duration::from_secs_f32(seconds)) {
                    self.status = format!("오류: 이동할 수 없어요: {e}");
                }
            }
        });

        if stop {
            self.playing = None;
        }
    }

    // 확인을 받은 뒤 파일을 휴지통으로 옮기기
    fn delete(&mut self, path: PathBuf) {
        let name = file_name(&path);
        let answer = rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Warning)
            .set_title("파일 삭제")
            .set_description(format!("이 파일을 휴지통으로 옮길까요?\n\n{name}"))
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
            Ok(()) => format!("휴지통으로 옮겼어요: {name}"),
            Err(e) => format!("오류: 삭제할 수 없어요: {e}"),
        };
        self.refresh_files();
    }

    // 저장된 mp3 목록 보여주기
    fn file_list(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(format!("저장된 목록 ({}개)", self.files.len()));
            if ui.button(symbol("↻")).on_hover_text("새로고침").clicked() {
                self.refresh_files();
            }
        });

        // 목록을 그리는 동안에는 클릭한 파일만 기억하고, 재생과 삭제는 그린 뒤에 하기
        let mut clicked = None;
        let mut to_delete = None;
        // 기본 스크롤 bar는 평소에 숨어 있으므로, 목록이 넘치면 항상 보이는 모양 사용
        ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            // 기본 최소 높이(64)를 없애서, 공간이 좁아도 하단 영역 뒤로 넘치지 않게 하기
            .min_scrolled_height(0.0)
            .show(ui, |ui| {
                for path in &self.files {
                    let video = is_video(path);
                    let is_current = self.playing.as_ref().is_some_and(|p| &p.path == path);
                    ui.horizontal(|ui| {
                        let hint = if video {
                            "기본 player로 열기"
                        } else {
                            "재생"
                        };
                        if ui.button(symbol("►")).on_hover_text(hint).clicked() {
                            clicked = Some(path.clone());
                        }
                        // 삭제 버튼은 재생 버튼과 헷갈리지 않도록 오른쪽 끝에 배치
                        let right_to_left = egui::Layout::right_to_left(egui::Align::Center);
                        ui.with_layout(right_to_left, |ui| {
                            if ui.button(symbol("×")).on_hover_text("삭제").clicked() {
                                to_delete = Some(path.clone());
                            }
                            // 파일 이름은 남은 공간에 왼쪽부터 표시
                            let left_to_right = egui::Layout::left_to_right(egui::Align::Center);
                            ui.with_layout(left_to_right, |ui| {
                                // 파일 이름이 줄어도 동영상인지 알 수 있게 표시
                                if video {
                                    ui.label(egui::RichText::new("[영상]").weak());
                                }
                                let mut name = egui::RichText::new(file_name(path));
                                if is_current {
                                    name = name.strong();
                                }
                                ui.add(egui::Label::new(name).truncate());
                            });
                        });
                    });
                }
            });

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
        self.status = String::from("다운로드 준비 중...");
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
                    self.status = String::from("다운로드 중...");
                }
                Ok(Update::Processing) => {
                    job.progress = None;
                    self.status = String::from(match job.format {
                        Format::Mp3 => "mp3로 변환 중...",
                        Format::Mp4 => "영상과 소리를 mp4로 정리하는 중...",
                    });
                }
                Ok(Update::Done(Ok(()))) => {
                    break format!("저장 완료! 위치: {}", self.folder.display());
                }
                Ok(Update::Done(Err(message))) => break format!("오류: {message}"),
                Err(TryRecvError::Empty) => return, // 아직 진행 중
                Err(TryRecvError::Disconnected) => break String::from("오류: 작업이 중단됐어요"),
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
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let idle = self.job.is_none();

        // 화면 하단의 저작권 정보. 남은 공간을 쓰는 CentralPanel보다 먼저 배치
        egui::Panel::bottom("copyright").show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.label(egui::RichText::new(COPYRIGHT).size(11.0).weak());
            });
        });

        // 삭제 결과는 저작권 정보의 분리선 위에 표시.
        // 아래쪽 영역은 나중에 배치한 것이 위에 놓임
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
            ui.add_enabled_ui(idle, |ui| {
                ui.label("영상 URL");
                let input =
                    ui.add(egui::TextEdit::singleline(&mut self.url).desired_width(f32::INFINITY));
                // 입력창에서 Enter를 눌렀는지
                let entered = input.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("폴더 선택").clicked() {
                        // 취소하면 None이므로 기존 폴더를 그대로 유지
                        if let Some(folder) = rfd::FileDialog::new()
                            .set_directory(&self.folder)
                            .pick_folder()
                        {
                            self.folder = folder;
                            self.refresh_files();
                        }
                    }
                    ui.label(self.folder.display().to_string());
                });

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label("저장 형식");
                    ui.radio_value(&mut self.format, Format::Mp3, "MP3 (소리)");
                    ui.radio_value(&mut self.format, Format::Mp4, "MP4 (동영상)");
                });

                ui.add_space(8.0);
                let has_url = !self.url.trim().is_empty();
                let clicked = ui
                    .add_enabled(has_url, egui::Button::new("저장하기"))
                    .clicked();
                if clicked || (entered && has_url) {
                    self.start_download(ui.ctx().clone());
                }
            });

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
                    ui.add(egui::ProgressBar::new(progress).show_percentage());
                }
            }

            ui.separator();
            self.player_bar(ui);
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
        .map_err(|e| format!("yt-dlp를 실행할 수 없어요: {e}"))?;

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

    let status = child
        .wait()
        .map_err(|e| format!("yt-dlp의 종료를 확인할 수 없어요: {e}"))?;

    if !status.success() {
        // yt-dlp가 남긴 오류의 마지막 줄을 보여주기
        let stderr = errors.join().unwrap_or_default();
        let reason = stderr.lines().last().unwrap_or("원인을 알 수 없어요");
        return Err(format!("다운로드에 실패했어요\n{reason}"));
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
    let file = File::open(&path).map_err(|e| format!("파일을 열 수 없어요: {e}"))?;
    let source = Decoder::try_from(file).map_err(|e| format!("재생할 수 없는 파일이에요: {e}"))?;

    let total = source.total_duration();

    let mut device = DeviceSinkBuilder::open_default_sink()
        .map_err(|e| format!("소리 출력 장치를 열 수 없어요: {e}"))?;
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

// 버튼에 쓸 특수문자.
// 고정폭 글꼴(Hack)로 그려야 이모지 모양이 아닌 글자 모양으로 나옴
fn symbol(text: &str) -> egui::RichText {
    egui::RichText::new(text).monospace()
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
