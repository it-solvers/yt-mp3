// MIDI 파일 재생. MIDI는 소리가 아니라 연주 정보이므로,
// 악기 음색 파일(SoundFont)을 이용해 소리를 만들면서 재생함

use rodio::source::SeekError;
use rodio::{ChannelCount, SampleRate, Source};
use rustysynth::{MidiFile, MidiFileSequencer, SoundFont, Synthesizer, SynthesizerSettings};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

// 함께 설치하는 음색 파일의 이름
const SOUNDFONT_NAME: &str = "GeneralUser-GS.sf2";
const RATE: u32 = 44_100;
// 한 번에 만드는 소리의 길이 (좌우 한 쌍의 개수)
const FRAMES: usize = 512;
// 마지막 음의 여운이 끊기지 않도록 곡 뒤에 두는 시간
const TAIL: f64 = 2.0;
// 이동할 때 연주를 따라가는 배속
const SEEK_SPEED: f64 = 50.0;

pub struct MidiSource {
    sequencer: MidiFileSequencer,
    // 처음으로 되감을 때 다시 사용
    file: Arc<MidiFile>,
    // 꼬리를 포함한 전체 길이(초)
    length: f64,
    // 지금까지 만든 곳의 위치(초)
    position: f64,
    // 만들어 둔 좌우 번갈아 놓인 소리와, 다음에 내보낼 자리
    buffer: Vec<f32>,
    next: usize,
}

// 읽을 수 있는 MIDI 파일인지 확인 (음색 파일은 읽지 않으므로 빠름)
pub fn can_read(path: &Path) -> bool {
    File::open(path).is_ok_and(|file| MidiFile::new(&mut BufReader::new(file)).is_ok())
}

impl MidiSource {
    pub fn open(path: &Path) -> Result<Self, String> {
        let font = soundfont()?;
        let file = File::open(path).map_err(|e| e.to_string())?;
        let file = MidiFile::new(&mut BufReader::new(file)).map_err(|e| e.to_string())?;
        let file = Arc::new(file);
        let synthesizer = Synthesizer::new(&font, &SynthesizerSettings::new(RATE as i32))
            .map_err(|e| e.to_string())?;

        let mut sequencer = MidiFileSequencer::new(synthesizer);
        sequencer.play(&file, false);

        Ok(Self {
            length: file.get_length() + TAIL,
            sequencer,
            file,
            position: 0.0,
            buffer: Vec::new(),
            next: 0,
        })
    }

    // 다음 부분의 소리 만들기. 연주 속도(speed)만큼 위치가 앞으로 감
    fn render(&mut self, frames: usize, speed: f64) {
        let mut left = vec![0.0; frames];
        let mut right = vec![0.0; frames];
        self.sequencer.set_speed(speed);
        self.sequencer.render(&mut left, &mut right);
        self.position += speed * frames as f64 / RATE as f64;

        self.buffer = left
            .into_iter()
            .zip(right)
            .flat_map(|(l, r)| [l, r])
            .collect();
        self.next = 0;
    }
}

impl Iterator for MidiSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if self.next >= self.buffer.len() {
            if self.position >= self.length {
                return None;
            }
            self.render(FRAMES, 1.0);
        }
        let sample = self.buffer[self.next];
        self.next += 1;
        Some(sample)
    }
}

impl Source for MidiSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        ChannelCount::new(2).expect("2는 0이 아님")
    }

    fn sample_rate(&self) -> SampleRate {
        SampleRate::new(RATE).expect("sample rate는 0이 아님")
    }

    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs_f64(self.length))
    }

    // 합성기는 건너뛰기를 지원하지 않으므로, 목표까지 빠른 속도로 연주해서 따라감
    fn try_seek(&mut self, to: Duration) -> Result<(), SeekError> {
        let target = to.as_secs_f64().min(self.length);
        if target < self.position {
            self.sequencer.play(&self.file, false);
            self.position = 0.0;
        }

        // 한 번에 작은 조각(64)씩: 빠른 구간 다음에 보통 속도로 맞추기
        const STEP: usize = 64;
        let fast_step = SEEK_SPEED * STEP as f64 / RATE as f64;
        while self.position + fast_step <= target {
            self.render(STEP, SEEK_SPEED);
        }
        while self.position + (STEP as f64 / RATE as f64) <= target {
            self.render(STEP, 1.0);
        }
        // 따라가며 만든 소리는 내보내지 않기
        self.buffer.clear();
        self.next = 0;
        Ok(())
    }
}

// 음색 파일은 크기가 커서 처음 한 번만 읽고 보관
fn soundfont() -> Result<Arc<SoundFont>, String> {
    static FONT: OnceLock<Arc<SoundFont>> = OnceLock::new();
    if let Some(font) = FONT.get() {
        return Ok(font.clone());
    }

    let path = soundfont_path().ok_or_else(|| {
        tr!(
            "MIDI 음색 파일을 찾을 수 없어요",
            "the MIDI SoundFont was not found",
        )
    })?;
    let file = File::open(path).map_err(|e| e.to_string())?;
    let font = SoundFont::new(&mut BufReader::new(file)).map_err(|e| e.to_string())?;
    Ok(FONT.get_or_init(|| Arc::new(font)).clone())
}

// 음색 파일의 위치. 없으면 None
fn soundfont_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let folder = exe.parent()?;
    // Windows는 실행 파일 옆, Mac은 .app 안의 Contents/Resources
    #[allow(unused_mut)] // 배포용 build에서는 더 넣지 않음
    let mut places = vec![
        folder.join("soundfont"),
        folder.join("../Resources/soundfont"),
    ];
    // 개발 중(cargo run)에는 scripts/fetch-soundfont.sh로 받아 둔 파일
    #[cfg(debug_assertions)]
    places.push(Path::new(env!("CARGO_MANIFEST_DIR")).join("target/soundfont"));

    places
        .into_iter()
        .map(|place| place.join(SOUNDFONT_NAME))
        .find(|path| path.is_file())
}
