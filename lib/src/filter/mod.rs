use std::{
    fmt,
    path::Path,
    sync::{Arc, Mutex},
};

use simplelog::*;

mod a_loudnorm;
mod custom_filter;
pub mod v_drawtext;

// get_delta
use self::custom_filter::custom_filter;
use crate::utils::{
    controller::ProcessUnit::*, fps_calc, get_delta, is_close, Media, MediaProbe, OutputMode::*,
    PlayoutConfig,
};

#[derive(Clone, Debug, Copy, Eq, PartialEq)]
pub enum FilterType {
    Audio,
    Video,
}

impl fmt::Display for FilterType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            FilterType::Audio => write!(f, "a"),
            FilterType::Video => write!(f, "v"),
        }
    }
}

use FilterType::*;

#[derive(Debug, Clone)]
pub struct Filters {
    pub audio_chain: String,
    pub video_chain: String,
    pub audio_map: Vec<String>,
    pub video_map: Vec<String>,
    pub output_map: Vec<String>,
    audio_track_count: i32,
    audio_position: i32,
    video_position: i32,
    audio_last: i32,
    video_last: i32,
}

impl Filters {
    pub fn new(audio_track_count: i32, audio_position: i32) -> Self {
        Self {
            audio_chain: String::new(),
            video_chain: String::new(),
            audio_map: vec![],
            video_map: vec![],
            output_map: vec![],
            audio_track_count,
            audio_position,
            video_position: 0,
            audio_last: -1,
            video_last: -1,
        }
    }

    pub fn add_filter(&mut self, filter: &str, track_nr: i32, filter_type: FilterType) {
        let (map, chain, position, last) = match filter_type {
            Audio => (
                &mut self.audio_map,
                &mut self.audio_chain,
                self.audio_position,
                &mut self.audio_last,
            ),
            Video => (
                &mut self.video_map,
                &mut self.video_chain,
                self.video_position,
                &mut self.video_last,
            ),
        };

        if *last != track_nr {
            // start new filter chain
            let mut selector = String::new();
            let mut sep = String::new();
            if !chain.is_empty() {
                selector = format!("[{}out{}]", filter_type, last);
                sep = ";".to_string()
            }

            chain.push_str(&selector);

            if filter.starts_with("aevalsrc") || filter.starts_with("movie") {
                chain.push_str(&format!("{sep}{filter}"));
            } else {
                chain.push_str(&format!(
                    "{sep}[{}:{}:{track_nr}]{filter}",
                    position, filter_type
                ));
            }

            let m = format!("[{}out{track_nr}]", filter_type);
            map.push(m.clone());
            self.output_map.append(&mut vec!["-map".to_string(), m]);
            *last = track_nr;
        } else if filter.starts_with(';') || filter.starts_with('[') {
            chain.push_str(filter);
        } else {
            chain.push_str(&format!(",{filter}"))
        }
    }

    pub fn cmd(&mut self) -> Vec<String> {
        let mut v_chain = self.video_chain.clone();
        let mut a_chain = self.audio_chain.clone();

        if self.video_last >= 0 {
            v_chain.push_str(&format!("[vout{}]", self.video_last));
        }

        if self.audio_last >= 0 {
            a_chain.push_str(&format!("[aout{}]", self.audio_last));
        }

        let mut f_chain = v_chain;
        let mut cmd = vec![];

        if !a_chain.is_empty() {
            f_chain.push(';');
            f_chain.push_str(&a_chain);
        }

        if !f_chain.is_empty() {
            cmd.push("-filter_complex".to_string());
            cmd.push(f_chain);
        }

        cmd
    }

    pub fn map(&mut self) -> Vec<String> {
        let mut o_map = self.output_map.clone();

        if self.video_last == -1 {
            let v_map = "0:v".to_string();

            if !o_map.contains(&v_map) {
                o_map.append(&mut vec!["-map".to_string(), v_map]);
            };
        }

        if self.audio_last == -1 {
            for i in 0..self.audio_track_count {
                let a_map = format!("{}:a:{i}", self.audio_position);

                if !o_map.contains(&a_map) {
                    o_map.append(&mut vec!["-map".to_string(), a_map]);
                };
            }
        }

        o_map
    }
}

impl Default for Filters {
    fn default() -> Self {
        Self::new(1, 0)
    }
}

fn deinterlace(field_order: &Option<String>, chain: &mut Filters) {
    if let Some(order) = field_order {
        if order != "progressive" {
            chain.add_filter("yadif=0:-1:0", 0, Video)
        }
    }
}

fn pad(aspect: f64, chain: &mut Filters, v_stream: &ffprobe::Stream, config: &PlayoutConfig) {
    if !is_close(aspect, config.processing.aspect, 0.03) {
        let mut scale = String::new();

        if let (Some(w), Some(h)) = (v_stream.width, v_stream.height) {
            if w > config.processing.width && aspect > config.processing.aspect {
                scale = format!("scale={}:-1,", config.processing.width);
            } else if h > config.processing.height && aspect < config.processing.aspect {
                scale = format!("scale=-1:{},", config.processing.height);
            }
        }
        //chain.add_filter(
        //    &format!(
        //        "{scale}pad=max(iw\\,ih*({0}/{1})):ow/({0}/{1}):(ow-iw)/2:(oh-ih)/2",
        //        config.processing.width, config.processing.height
        //   ),
        //    0,
        //    Video,
        //)
    }
}

fn fps(fps: f64, chain: &mut Filters, config: &PlayoutConfig) {
    if fps != config.processing.fps {
        //chain.add_filter(&format!("fps={}", config.processing.fps), 0, Video)
    }
}

fn scale(
    width: Option<i64>,
    height: Option<i64>,
    aspect: f64,
    chain: &mut Filters,
    config: &PlayoutConfig,
) {
    // width: i64, height: i64
    if let (Some(w), Some(h)) = (width, height) {
        if w != config.processing.width || h != config.processing.height {
            chain.add_filter(
                &format!(
                    "scale_npp={}:{}:format=yuv420p",
                    config.processing.width, config.processing.height
                ),
                0,
                Video,
            );
        } else {
            chain.add_filter("scale_npp=1280:720:format=yuv420p", 0, Video);
            );
        }

        if !is_close(aspect, config.processing.aspect, 0.03) {
            //chain.add_filter(                &format!("setdar=dar={}", config.processing.aspect),                0,                Video,            )
        }
    } else {
        chain.add_filter(
            &format!(
                "scale_npp={}:{}:format=yuv420p",
                config.processing.width, config.processing.height
            ),
            0,
            Video,
        );
       // chain.add_filter(
       //     &format!("setdar=dar={}", config.processing.aspect),
       //     0,
       //     Video,
       // )
    }
}

fn fade(node: &mut Media, chain: &mut Filters, nr: i32, filter_type: FilterType) {
    let mut t = "";

    if filter_type == Audio {
        t = "a"
    }

    if node.seek > 0.0 || node.unit == Ingest {
        //chain.add_filter(&format!("{t}fade=in:st=0:d=0.5"), nr, filter_type)
    }

    if node.out != node.duration && node.out - node.seek - 1.0 > 0.0 {
        //chain.add_filter(
        //    &format!("{t}fade=out:st={}:d=1.0", (node.out - node.seek - 1.0)),
        //    nr,
        //    filter_type,
        //)
    }
}

fn overlay(node: &mut Media, chain: &mut Filters, config: &PlayoutConfig) {
    if config.processing.add_logo
        && Path::new(&config.processing.logo).is_file()
        && &node.category != "advertisement"
    {
        let mut logo_chain = format!(
            "null[v];movie={}[l0];[l0]format=rgba,colorchannelmixer=aa=0.4,hwupload_cuda[l];[v][l]overlay_cuda=main_w-overlay_w-50:33",
            config.processing.logo
        );

        if node.last_ad.unwrap_or(false) {
            //logo_chain.push_str(",fade=in:st=0:d=1.0:alpha=1")
        }

        if node.next_ad.unwrap_or(false) {
            //logo_chain.push_str(
            //    format!(",fade=out:st={}:d=1.0:alpha=1", node.out - node.seek - 1.0).as_str(),
            //)
        }

        chain.add_filter(&logo_chain, 0, Video);
    }
}

fn extend_video(node: &mut Media, chain: &mut Filters) {
    if let Some(video_duration) = node
        .probe
        .as_ref()
        .and_then(|p| p.video_streams.get(0))
        .and_then(|v| v.duration.as_ref())
        .and_then(|v| v.parse::<f64>().ok())
    {
        if node.out - node.seek > video_duration - node.seek + 0.1 && node.duration >= node.out {
            //chain.add_filter(
            //    &format!(
            //        "tpad=stop_mode=add:stop_duration={}",
            //        (node.out - node.seek) - (video_duration - node.seek)
            //    ),
            //    0,
            //    Video,
            //)
        }
    }
}

/// add drawtext filter for lower thirds messages
fn add_text(
    node: &mut Media,
    chain: &mut Filters,
    config: &PlayoutConfig,
    filter_chain: &Option<Arc<Mutex<Vec<String>>>>,
) {
    if config.text.add_text
        && (config.text.text_from_filename || config.out.mode == HLS || node.unit == Encoder)
    {
        let filter = v_drawtext::filter_node(config, Some(node), filter_chain);

        chain.add_filter(&filter, 0, Video);
    }
}

fn add_audio(node: &Media, chain: &mut Filters, nr: i32) {
    let audio = format!(
        "aevalsrc=0:channel_layout=stereo:duration={}:sample_rate=48000",
        node.out - node.seek
    );
    chain.add_filter(&audio, nr, Audio);
}

fn extend_audio(node: &mut Media, chain: &mut Filters, nr: i32) {
    let probe = if Path::new(&node.audio).is_file() {
        Some(MediaProbe::new(&node.audio))
    } else {
        node.probe.clone()
    };

    if let Some(audio_duration) = probe
        .as_ref()
        .and_then(|p| p.audio_streams.get(0))
        .and_then(|a| a.duration.clone())
        .and_then(|a| a.parse::<f64>().ok())
    {
        if node.out - node.seek > audio_duration - node.seek + 0.1 && node.duration >= node.out {
            chain.add_filter(
                &format!("apad=whole_dur={}", node.out - node.seek),
                nr,
                Audio,
            )
        }
    }
}

/// Add single pass loudnorm filter to audio line.
fn add_loudnorm(node: &Media, chain: &mut Filters, config: &PlayoutConfig, nr: i32) {
    if config.processing.add_loudnorm || (node.unit == Ingest && config.processing.loudnorm_ingest)
    {
        let loud_filter = a_loudnorm::filter_node(config);
        chain.add_filter(&loud_filter, nr, Audio);
    }
}

fn audio_volume(chain: &mut Filters, config: &PlayoutConfig, nr: i32) {
    if config.processing.volume != 1.0 {
        chain.add_filter(&format!("volume={}", config.processing.volume), nr, Audio)
    }
}

fn aspect_calc(aspect_string: &Option<String>, config: &PlayoutConfig) -> f64 {
    let mut source_aspect = config.processing.aspect;

    if let Some(aspect) = aspect_string {
        let aspect_vec: Vec<&str> = aspect.split(':').collect();
        let w: f64 = aspect_vec[0].parse().unwrap();
        let h: f64 = aspect_vec[1].parse().unwrap();
        source_aspect = w as f64 / h as f64;
    }

    source_aspect
}

/// This realtime filter is important for HLS output to stay in sync.
fn realtime(node: &mut Media, chain: &mut Filters, config: &PlayoutConfig) {
    if config.general.generate.is_none() && config.out.mode == HLS {
        let mut speed_filter = "realtime=speed=1".to_string();

        if let Some(begin) = &node.begin {
            let (delta, _) = get_delta(config, begin);

            if delta < 0.0 && node.seek == 0.0 {
                let duration = node.out - node.seek;
                let speed = duration / (duration + delta);

                if speed > 0.0 && speed < 1.1 && delta < config.general.stop_threshold {
                    speed_filter = format!("realtime=speed={speed}");
                }
            }
        }

        chain.add_filter(&speed_filter, 0, Video);
    }
}

fn custom(filter: &str, chain: &mut Filters, nr: i32, filter_type: FilterType) {
    if !filter.is_empty() {
        chain.add_filter(filter, nr, filter_type);
    }
}

pub fn filter_chains(
    config: &PlayoutConfig,
    node: &mut Media,
    filter_chain: &Option<Arc<Mutex<Vec<String>>>>,
) -> Filters {
    let mut filters = Filters::new(config.processing.audio_tracks, 0);

    if node.unit == Encoder {
        add_text(node, &mut filters, config, filter_chain);

        return filters;
    }

    if let Some(probe) = node.probe.as_ref() {
        if Path::new(&node.audio).is_file() {
            filters.audio_position = 1;
        }

        if let Some(v_stream) = &probe.video_streams.get(0) {
            let aspect = aspect_calc(&v_stream.display_aspect_ratio, config);
            let frame_per_sec = fps_calc(&v_stream.r_frame_rate, 1.0);

            deinterlace(&v_stream.field_order, &mut filters);
            pad(aspect, &mut filters, v_stream, config);
            fps(frame_per_sec, &mut filters, config);
            scale(
                v_stream.width,
                v_stream.height,
                aspect,
                &mut filters,
                config,
            );
        }

        extend_video(node, &mut filters);
    } else {
        fps(0.0, &mut filters, config);
        scale(None, None, 1.0, &mut filters, config);
    }

    add_text(node, &mut filters, config, filter_chain);
    fade(node, &mut filters, 0, Video);
    overlay(node, &mut filters, config);
    realtime(node, &mut filters, config);

    let (proc_vf, proc_af) = custom_filter(&config.processing.custom_filter);
    let (list_vf, list_af) = custom_filter(&node.custom_filter);

    custom(&proc_vf, &mut filters, 0, Video);
    custom(&list_vf, &mut filters, 0, Video);

    for i in 0..config.processing.audio_tracks {
        if node
            .probe
            .as_ref()
            .and_then(|p| p.audio_streams.get(i as usize))
            .is_some()
            || Path::new(&node.audio).is_file()
        {
            extend_audio(node, &mut filters, i);
        } else if node.unit == Decoder {
            warn!(
                "Missing audio track (id {i}) from <b><magenta>{}</></b>",
                node.source
            );
            add_audio(node, &mut filters, i);
        }
        // add at least anull filter, for correct filter construction,
        // is important for split filter in HLS mode
        filters.add_filter("anull", i, Audio);

        add_loudnorm(node, &mut filters, config, i);
        fade(node, &mut filters, i, Audio);
        audio_volume(&mut filters, config, i);

        custom(&proc_af, &mut filters, i, Audio);
        custom(&list_af, &mut filters, i, Audio);
    }

    filters
}
