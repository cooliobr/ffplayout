use std::{
    env, fmt,
    fs::File,
    path::{Path, PathBuf},
    process,
    str::FromStr,
};

use serde::{Deserialize, Serialize};
use shlex::split;

use super::vec_strings;
use crate::utils::{free_tcp_socket, home_dir, time_to_sec, OutputMode::*};

pub const DUMMY_LEN: f64 = 60.0;
pub const IMAGE_FORMAT: [&str; 21] = [
    "bmp", "dds", "dpx", "exr", "gif", "hdr", "j2k", "jpg", "jpeg", "pcx", "pfm", "pgm", "phm",
    "png", "psd", "ppm", "sgi", "svg", "tga", "tif", "webp",
];

// Some well known errors can be safely ignore
pub const FFMPEG_IGNORE_ERRORS: [&str; 4] = [
    "ac-tex damaged",
    "Referenced QT chapter track not found",
    "skipped MB in I-frame at",
    "Warning MVs not available",
];

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum OutputMode {
    Desktop,
    HLS,
    Null,
    Stream,
}

impl FromStr for OutputMode {
    type Err = String;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input {
            "desktop" => Ok(Self::Desktop),
            "hls" => Ok(Self::HLS),
            "null" => Ok(Self::Null),
            "stream" => Ok(Self::Stream),
            _ => Err("Use 'desktop', 'hls', 'null' or 'stream'".to_string()),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ProcessMode {
    Folder,
    Playlist,
}

impl fmt::Display for ProcessMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ProcessMode::Folder => write!(f, "folder"),
            ProcessMode::Playlist => write!(f, "playlist"),
        }
    }
}

impl FromStr for ProcessMode {
    type Err = String;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input {
            "folder" => Ok(Self::Folder),
            "playlist" => Ok(Self::Playlist),
            _ => Err("Use 'folder' or 'playlist'".to_string()),
        }
    }
}

/// Global Config
///
/// This we init ones, when ffplayout is starting and use them globally in the hole program.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PlayoutConfig {
    pub general: General,
    pub rpc_server: RpcServer,
    pub mail: Mail,
    pub logging: Logging,
    pub processing: Processing,
    pub ingest: Ingest,
    pub playlist: Playlist,
    pub storage: Storage,
    pub text: Text,
    pub out: Out,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct General {
    pub help_text: String,
    pub stop_threshold: f64,

    #[serde(skip_serializing, skip_deserializing)]
    pub generate: Option<Vec<String>>,

    #[serde(skip_serializing, skip_deserializing)]
    pub stat_file: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RpcServer {
    pub help_text: String,
    pub enable: bool,
    pub address: String,
    pub authorization: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Mail {
    pub help_text: String,
    pub subject: String,
    pub smtp_server: String,
    pub starttls: bool,
    pub sender_addr: String,
    pub sender_pass: String,
    pub recipient: String,
    pub mail_level: String,
    pub interval: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Logging {
    pub help_text: String,
    pub log_to_file: bool,
    pub backup_count: usize,
    pub local_time: bool,
    pub timestamp: bool,
    pub log_path: String,
    pub log_level: String,
    pub ffmpeg_level: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Processing {
    pub help_text: String,
    pub mode: ProcessMode,
    pub width: i64,
    pub height: i64,
    pub aspect: f64,
    pub fps: f64,
    pub add_logo: bool,
    pub logo: String,
    pub logo_scale: String,
    pub logo_opacity: f32,
    pub logo_filter: String,
    #[serde(default)]
    pub audio_tracks: i32,
    pub add_loudnorm: bool,
    pub loudnorm_ingest: bool,
    pub loud_i: f32,
    pub loud_tp: f32,
    pub loud_lra: f32,
    pub volume: f64,
    #[serde(default)]
    pub custom_filter: String,

    #[serde(skip_serializing, skip_deserializing)]
    pub settings: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Ingest {
    pub help_text: String,
    pub enable: bool,
    input_param: String,

    #[serde(skip_serializing, skip_deserializing)]
    pub input_cmd: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Playlist {
    pub help_text: String,
    pub path: String,
    pub day_start: String,

    #[serde(skip_serializing, skip_deserializing)]
    pub start_sec: Option<f64>,

    pub length: String,

    #[serde(skip_serializing, skip_deserializing)]
    pub length_sec: Option<f64>,

    pub infinit: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Storage {
    pub help_text: String,
    pub path: String,
    pub filler_clip: String,
    pub extensions: Vec<String>,
    pub shuffle: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Text {
    pub help_text: String,
    pub add_text: bool,

    #[serde(skip_serializing, skip_deserializing)]
    pub node_pos: Option<usize>,

    #[serde(skip_serializing, skip_deserializing)]
    pub zmq_stream_socket: Option<String>,

    #[serde(skip_serializing, skip_deserializing)]
    pub zmq_server_socket: Option<String>,

    pub fontfile: String,
    pub text_from_filename: bool,
    pub style: String,
    pub regex: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Out {
    pub help_text: String,
    pub mode: OutputMode,
    pub output_param: String,

    #[serde(skip_serializing, skip_deserializing)]
    pub output_cmd: Option<Vec<String>>,
}

impl PlayoutConfig {
    /// Read config from YAML file, and set some extra config values.
    pub fn new(cfg_path: Option<String>) -> Self {
        let mut config_path = PathBuf::from("/etc/ffplayout/ffplayout.yml");

        if let Some(cfg) = cfg_path {
            config_path = PathBuf::from(cfg);
        }

        if !config_path.is_file() {
            if Path::new("./assets/ffplayout.yml").is_file() {
                config_path = PathBuf::from("./assets/ffplayout.yml")
            } else if let Some(p) = env::current_exe().ok().as_ref().and_then(|op| op.parent()) {
                config_path = p.join("ffplayout.yml")
            };
        }

        let f = match File::open(&config_path) {
            Ok(file) => file,
            Err(_) => {
                println!(
                    "{config_path:?} doesn't exists!\nPut \"ffplayout.yml\" in \"/etc/playout/\" or beside the executable!"
                );
                process::exit(1);
            }
        };

        let mut config: PlayoutConfig =
            serde_yaml::from_reader(f).expect("Could not read config file.");
        config.general.generate = None;
        config.general.stat_file = home_dir()
            .unwrap_or_else(env::temp_dir)
            .join(".ffp_status")
            .display()
            .to_string();

        config.playlist.start_sec = Some(time_to_sec(&config.playlist.day_start));

        if config.playlist.length.contains(':') {
            config.playlist.length_sec = Some(time_to_sec(&config.playlist.length));
        } else {
            config.playlist.length_sec = Some(86400.0);
        }

        if config.processing.add_logo && !Path::new(&config.processing.logo).is_file() {
            config.processing.add_logo = false;
        }

        if config.processing.audio_tracks < 1 {
            config.processing.audio_tracks = 1
        }

        // We set the decoder settings here, so we only define them ones.
        let mut settings = vec_strings![
            "-pix_fmt",
            "yuv420p",
            "-r",
            &config.processing.fps,
            "-c:v",
            "mpeg2video",
            "-g",
            "1",
            "-qscale:v",
            "2"
        ];

        settings.append(&mut pre_audio_codec(config.processing.add_loudnorm));
        settings.append(&mut vec_strings![
            "-ar", "48000", "-ac", "2", "-f", "mpegts", "-"
        ]);

        config.processing.settings = Some(settings);

        config.ingest.input_cmd = split(config.ingest.input_param.as_str());

        if config.out.mode == Null {
            config.out.output_cmd = Some(vec_strings!["-f", "null", "-"]);
        } else {
            config.out.output_cmd = split(config.out.output_param.as_str());
        }

        // when text overlay without text_from_filename is on, turn also the RPC server on,
        // to get text messages from it
        if config.text.add_text && !config.text.text_from_filename {
            config.rpc_server.enable = true;
            config.text.zmq_stream_socket = free_tcp_socket(String::new());
            config.text.zmq_server_socket =
                free_tcp_socket(config.text.zmq_stream_socket.clone().unwrap_or_default());
            config.text.node_pos = Some(2);
        } else {
            config.text.zmq_stream_socket = None;
            config.text.zmq_server_socket = None;
            config.text.node_pos = None;
        }

        config
    }
}

impl Default for PlayoutConfig {
    fn default() -> Self {
        Self::new(None)
    }
}

/// When add_loudnorm is False we use a different audio encoder,
/// s302m has higher quality, but is experimental
/// and works not well together with the loudnorm filter.
fn pre_audio_codec(add_loudnorm: bool) -> Vec<String> {
    let mut codec = vec_strings!["-c:a", "s302m", "-strict", "-2", "-sample_fmt", "s16"];

    if add_loudnorm {
        codec = vec_strings!["-c:a", "mp2", "-b:a", "384k"];
    }

    codec
}
