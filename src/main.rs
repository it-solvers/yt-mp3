use eframe::egui;
use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player};
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::Duration;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([560.0, 480.0]),
        ..Default::default()
    };

    eframe::run_native(
        "YouTube → MP3",
        options,
        Box::new(|_cc| Ok(Box::new(App::new()))),
    )
}

// 화면이 기억해야 하는 값들
struct App {
    url: String,
    folder: PathBuf,
    status: String,
    // 다운로드 중이면 Some, 아니면 None
    job: Option<Receiver<Result<(), String>>>,
    // 저장 폴더에 있는 mp3 목록
    files: Vec<PathBuf>,
    // 재생 중이면 Some, 아니면 None
    playing: Option<Playing>,
}

// 재생에 필요한 값들
struct Playing {
    path: PathBuf,
    player: Player,
    // 소리 출력 장치. 이 값이 사라지면 재생도 멈추므로 함께 보관
    _device: MixerDeviceSink,
}

impl App {
    fn new() -> Self {
        // 기본 저장 폴더는 ~/Music (HOME이 없으면 현재 폴더)
        let folder = match std::env::var("HOME") {
            Ok(home) => PathBuf::from(home).join("Music"),
            Err(_) => PathBuf::from("."),
        };

        let mut app = Self {
            url: String::new(),
            folder,
            status: String::from("URL을 입력해 주세요"),
            job: None,
            files: Vec::new(),
            playing: None,
        };
        app.refresh_files();
        app
    }

    // 저장 폴더의 mp3 목록 다시 읽기
    fn refresh_files(&mut self) {
        match list_mp3(&self.folder) {
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
            Ok(playing) => self.playing = Some(playing),
            Err(message) => self.status = format!("오류: {message}"),
        }
    }

    // 재생 중인 곡의 이름과 일시정지/정지 버튼
    fn player_bar(&mut self, ui: &mut egui::Ui) {
        // 곡이 끝까지 재생됐으면 정리
        if self.playing.as_ref().is_some_and(|p| p.player.empty()) {
            self.playing = None;
        }

        let Some(playing) = &self.playing else {
            return;
        };

        let mut stop = false;
        ui.horizontal(|ui| {
            if playing.player.is_paused() {
                if ui.button("계속 재생").clicked() {
                    playing.player.play();
                }
            } else if ui.button("일시정지").clicked() {
                playing.player.pause();
            }
            stop = ui.button("정지").clicked();

            let seconds = playing.player.get_pos().as_secs();
            ui.label(format!("{:02}:{:02}", seconds / 60, seconds % 60));
            ui.add(egui::Label::new(file_name(&playing.path)).truncate());
        });

        if stop {
            self.playing = None;
        }

        // 재생 시간 표시를 위해 잠시 뒤 화면을 다시 그리기
        ui.ctx().request_repaint_after(Duration::from_millis(500));
    }

    // 저장된 mp3 목록 보여주기
    fn file_list(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(format!("저장된 목록 ({}개)", self.files.len()));
            if ui.button("새로고침").clicked() {
                self.refresh_files();
            }
        });

        // 목록을 그리는 동안에는 클릭한 파일만 기억하고, 재생은 그린 뒤에 하기
        let mut clicked = None;
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                for path in &self.files {
                    let is_current = self.playing.as_ref().is_some_and(|p| &p.path == path);
                    ui.horizontal(|ui| {
                        if ui.button("재생").clicked() {
                            clicked = Some(path.clone());
                        }
                        let mut name = egui::RichText::new(file_name(path));
                        if is_current {
                            name = name.strong();
                        }
                        ui.add(egui::Label::new(name).truncate());
                    });
                }
            });

        if let Some(path) = clicked {
            self.play(path);
        }
    }

    // 별도 thread에서 다운로드 시작하기 (화면이 멈추지 않도록)
    fn start_download(&mut self, ctx: egui::Context) {
        let (sender, receiver) = mpsc::channel();
        let url = self.url.trim().to_string();
        let folder = self.folder.clone();

        thread::spawn(move || {
            let result = download(&url, &folder);
            // 창이 이미 닫혔다면 보낼 곳이 없으므로 결과는 무시
            let _ = sender.send(result);
            // 화면을 다시 그려 달라고 요청
            ctx.request_repaint();
        });

        self.job = Some(receiver);
        self.status = String::from("다운로드 중...");
    }

    // 다운로드가 끝났는지 확인하기 (기다리지 않음)
    fn check_download(&mut self) {
        let Some(receiver) = &self.job else {
            return;
        };

        self.status = match receiver.try_recv() {
            Ok(Ok(())) => format!("저장 완료! 위치: {}", self.folder.display()),
            Ok(Err(message)) => format!("오류: {message}"),
            Err(TryRecvError::Empty) => return, // 아직 진행 중
            Err(TryRecvError::Disconnected) => String::from("오류: 작업이 중단됐어요"),
        };
        self.job = None;
        self.refresh_files();
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.check_download();
        let idle = self.job.is_none();

        egui::CentralPanel::default().show(ui, |ui| {
            // 다운로드 중에는 입력과 버튼을 비활성화
            ui.add_enabled_ui(idle, |ui| {
                ui.label("영상 URL");
                ui.add(egui::TextEdit::singleline(&mut self.url).desired_width(f32::INFINITY));

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
                let has_url = !self.url.trim().is_empty();
                if ui
                    .add_enabled(has_url, egui::Button::new("저장하기"))
                    .clicked()
                {
                    self.start_download(ui.ctx().clone());
                }
            });

            ui.separator();
            ui.horizontal(|ui| {
                if !idle {
                    ui.spinner();
                }
                ui.label(&self.status);
            });

            ui.separator();
            self.player_bar(ui);
            self.file_list(ui);
        });
    }
}

// yt-dlp를 실행해서 mp3로 저장하기
fn download(url: &str, folder: &Path) -> Result<(), String> {
    //   -x                  : 오디오만 추출
    //   --audio-format mp3  : mp3로 변환 (ffmpeg 필요)
    //   --no-playlist       : 재생목록 링크여도 영상 하나만 받기
    //   --no-mtime          : 파일 날짜를 받은 시각으로 (최신순 정렬용)
    //   -P 폴더             : 저장할 폴더 지정
    //   --                  : 이 뒤의 값은 옵션이 아니라고 알려주기
    let output = Command::new("yt-dlp")
        .args(["-x", "--audio-format", "mp3", "--no-playlist", "--no-mtime"])
        .arg("-P")
        .arg(folder)
        .arg("--")
        .arg(url)
        .output() // 실행하고, 출력 내용까지 모아서 가져오기
        .map_err(|e| format!("yt-dlp를 실행할 수 없어요: {e}"))?;

    if !output.status.success() {
        // yt-dlp가 남긴 오류의 마지막 줄을 보여주기
        let stderr = String::from_utf8_lossy(&output.stderr);
        let reason = stderr.lines().last().unwrap_or("원인을 알 수 없어요");
        return Err(format!("다운로드에 실패했어요\n{reason}"));
    }

    Ok(())
}

// 폴더 안의 mp3 파일을 최신순으로 가져오기
fn list_mp3(folder: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files: Vec<PathBuf> = fs::read_dir(folder)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("mp3"))
        })
        .collect();

    files.sort_by_key(|path| {
        let modified = fs::metadata(path).and_then(|m| m.modified()).ok();
        std::cmp::Reverse(modified)
    });

    Ok(files)
}

// 소리 출력 장치를 열고 파일 재생 시작하기
fn start_playing(path: PathBuf) -> Result<Playing, String> {
    let file = File::open(&path).map_err(|e| format!("파일을 열 수 없어요: {e}"))?;
    let source = Decoder::try_from(file).map_err(|e| format!("재생할 수 없는 파일이에요: {e}"))?;

    let mut device = DeviceSinkBuilder::open_default_sink()
        .map_err(|e| format!("소리 출력 장치를 열 수 없어요: {e}"))?;
    // 정지할 때 터미널에 안내 문구가 나오지 않도록 끄기
    device.log_on_drop(false);

    let player = Player::connect_new(device.mixer());
    player.append(source);

    Ok(Playing {
        path,
        player,
        _device: device,
    })
}

// 경로에서 파일 이름만 꺼내기
fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}
