use std::fs;

use ffplayout::{input::playlist::gen_source, utils::prepare_output_cmd};
use ffplayout_lib::{
    utils::{Media, OutputMode::*, PlayoutConfig, ProcessUnit::*},
    vec_strings,
};

#[test]
fn video_audio_input() {
    let mut config = PlayoutConfig::new(Some("../assets/ffplayout.yml".to_string()));
    config.out.mode = Stream;
    config.processing.add_logo = true;
    let logo_path = fs::canonicalize("./assets/logo.png").unwrap();
    config.processing.logo = logo_path.to_string_lossy().to_string();

    let media_obj = Media::new(0, "./assets/with_audio.mp4", true);
    let media = gen_source(&config, media_obj, &None);

    let test_filter_cmd =
        vec_strings![
            "-filter_complex",
            format!("[0:v:0]scale=1024:576,null[v];movie={}:loop=0,setpts=N/(FRAME_RATE*TB),format=rgba,colorchannelmixer=aa=0.7[l];[v][l]overlay=W-w-12:12:shortest=1[vout0];[0:a:0]anull[aout0]", config.processing.logo)
        ];

    let test_filter_map = vec_strings!["-map", "[vout0]", "-map", "[aout0]"];

    assert_eq!(
        media.cmd,
        Some(vec_strings!["-i", "./assets/with_audio.mp4"])
    );
    assert_eq!(media.filter.clone().unwrap().cmd(), test_filter_cmd);
    assert_eq!(media.filter.unwrap().map(), test_filter_map);
}

#[test]
fn dual_audio_aevalsrc_input() {
    let mut config = PlayoutConfig::new(Some("../assets/ffplayout.yml".to_string()));
    config.out.mode = Stream;
    config.processing.audio_tracks = 2;
    config.processing.add_logo = false;

    let media_obj = Media::new(0, "./assets/with_audio.mp4", true);
    let media = gen_source(&config, media_obj, &None);

    let test_filter_cmd =
        vec_strings![
            "-filter_complex",
            "[0:v:0]scale=1024:576[vout0];[0:a:0]anull[aout0];aevalsrc=0:channel_layout=stereo:duration=30:sample_rate=48000,anull[aout1]"
        ];

    let test_filter_map = vec_strings!["-map", "[vout0]", "-map", "[aout0]", "-map", "[aout1]"];

    assert_eq!(
        media.cmd,
        Some(vec_strings!["-i", "./assets/with_audio.mp4"])
    );
    assert_eq!(media.filter.clone().unwrap().cmd(), test_filter_cmd);
    assert_eq!(media.filter.unwrap().map(), test_filter_map);
}

#[test]
fn dual_audio_input() {
    let mut config = PlayoutConfig::new(Some("../assets/ffplayout.yml".to_string()));
    config.out.mode = Stream;
    config.processing.audio_tracks = 2;
    config.processing.add_logo = false;

    let media_obj = Media::new(0, "./assets/dual_audio.mp4", true);
    let media = gen_source(&config, media_obj, &None);

    let test_filter_cmd = vec_strings![
        "-filter_complex",
        "[0:v:0]scale=1024:576[vout0];[0:a:0]anull[aout0];[0:a:1]anull[aout1]"
    ];

    let test_filter_map = vec_strings!["-map", "[vout0]", "-map", "[aout0]", "-map", "[aout1]"];

    assert_eq!(
        media.cmd,
        Some(vec_strings!["-i", "./assets/dual_audio.mp4"])
    );
    assert_eq!(media.filter.clone().unwrap().cmd(), test_filter_cmd);
    assert_eq!(media.filter.unwrap().map(), test_filter_map);
}

#[test]
fn video_separate_audio_input() {
    let mut config = PlayoutConfig::new(Some("../assets/ffplayout.yml".to_string()));
    config.out.mode = Stream;
    config.processing.audio_tracks = 1;
    config.processing.add_logo = false;

    let mut media_obj = Media::new(0, "./assets/no_audio.mp4", true);
    media_obj.audio = "./assets/audio.mp3".to_string();
    let media = gen_source(&config, media_obj, &None);

    let test_filter_cmd = vec_strings![
        "-filter_complex",
        "[0:v:0]scale=1024:576[vout0];[1:a:0]anull[aout0]"
    ];

    let test_filter_map = vec_strings!["-map", "[vout0]", "-map", "[aout0]"];

    assert_eq!(
        media.cmd,
        Some(vec_strings![
            "-i",
            "./assets/no_audio.mp4",
            "-i",
            "./assets/audio.mp3",
            "-t",
            "30"
        ])
    );
    assert_eq!(media.filter.clone().unwrap().cmd(), test_filter_cmd);
    assert_eq!(media.filter.unwrap().map(), test_filter_map);
}

#[test]
fn video_audio_stream() {
    let mut config = PlayoutConfig::new(Some("../assets/ffplayout.yml".to_string()));
    config.out.mode = Stream;
    config.processing.add_logo = false;
    config.out.output_cmd = Some(vec_strings![
        "-c:v",
        "libx264",
        "-c:a",
        "aac",
        "-ar",
        "44100",
        "-b:a",
        "128k",
        "-flags",
        "+global_header",
        "-f",
        "flv",
        "rtmp://localhost/live/stream"
    ]);

    let enc_prefix = vec_strings![
        "-hide_banner",
        "-nostats",
        "-v",
        "level+error",
        "-re",
        "-i",
        "pipe:0"
    ];

    let enc_cmd = prepare_output_cmd(&config, enc_prefix, &None);

    let test_cmd = vec_strings![
        "-hide_banner",
        "-nostats",
        "-v",
        "level+error",
        "-re",
        "-i",
        "pipe:0",
        "-c:v",
        "libx264",
        "-c:a",
        "aac",
        "-ar",
        "44100",
        "-b:a",
        "128k",
        "-flags",
        "+global_header",
        "-f",
        "flv",
        "rtmp://localhost/live/stream"
    ];

    assert_eq!(enc_cmd, test_cmd);
}

#[test]
fn video_audio_filter_stream() {
    let mut config = PlayoutConfig::new(Some("../assets/ffplayout.yml".to_string()));
    config.out.mode = Stream;
    config.processing.add_logo = false;
    config.text.add_text = false;
    config.out.output_cmd = Some(vec_strings![
        "-filter_complex",
        "[0:v]gblur=2[vout0];[0:a]volume=0.2[aout0]",
        "-map",
        "[vout0]",
        "-map",
        "[aout0]",
        "-c:v",
        "libx264",
        "-c:a",
        "aac",
        "-ar",
        "44100",
        "-b:a",
        "128k",
        "-flags",
        "+global_header",
        "-f",
        "flv",
        "rtmp://localhost/live/stream"
    ]);

    let mut media = Media::new(0, "", false);
    media.unit = Encoder;
    media.add_filter(&config, &None);

    let enc_prefix = vec_strings![
        "-hide_banner",
        "-nostats",
        "-v",
        "level+error",
        "-re",
        "-i",
        "pipe:0"
    ];

    let enc_cmd = prepare_output_cmd(&config, enc_prefix, &media.filter);

    let test_cmd = vec_strings![
        "-hide_banner",
        "-nostats",
        "-v",
        "level+error",
        "-re",
        "-i",
        "pipe:0",
        "-filter_complex",
        "[0:v:0]gblur=2[vout0];[0:a:0]volume=0.2[aout0]",
        "-map",
        "[vout0]",
        "-map",
        "[aout0]",
        "-c:v",
        "libx264",
        "-c:a",
        "aac",
        "-ar",
        "44100",
        "-b:a",
        "128k",
        "-flags",
        "+global_header",
        "-f",
        "flv",
        "rtmp://localhost/live/stream"
    ];

    assert_eq!(enc_cmd, test_cmd);
}

#[test]
fn video_audio_filter2_stream() {
    let mut config = PlayoutConfig::new(Some("../assets/ffplayout.yml".to_string()));
    config.out.mode = Stream;
    config.processing.add_logo = false;
    config.text.add_text = true;
    config.text.fontfile = String::new();
    config.out.output_cmd = Some(vec_strings![
        "-filter_complex",
        "[0:v]gblur=2[vout0];[0:a]volume=0.2[aout0]",
        "-map",
        "[vout0]",
        "-map",
        "[aout0]",
        "-c:v",
        "libx264",
        "-c:a",
        "aac",
        "-ar",
        "44100",
        "-b:a",
        "128k",
        "-flags",
        "+global_header",
        "-f",
        "flv",
        "rtmp://localhost/live/stream"
    ]);

    let mut media = Media::new(0, "", false);
    media.unit = Encoder;
    media.add_filter(&config, &None);

    let socket = config
        .text
        .zmq_stream_socket
        .clone()
        .unwrap()
        .replace(':', "\\:");

    let enc_prefix = vec_strings![
        "-hide_banner",
        "-nostats",
        "-v",
        "level+error",
        "-re",
        "-i",
        "pipe:0"
    ];

    let enc_cmd = prepare_output_cmd(&config, enc_prefix, &media.filter);

    let test_cmd = vec_strings![
        "-hide_banner",
        "-nostats",
        "-v",
        "level+error",
        "-re",
        "-i",
        "pipe:0",
        "-filter_complex",
        format!("[0:v:0]zmq=b=tcp\\\\://'{socket}',drawtext@dyntext=text='',gblur=2[vout0];[0:a:0]volume=0.2[aout0]"),
        "-map",
        "[vout0]",
        "-map",
        "[aout0]",
        "-c:v",
        "libx264",
        "-c:a",
        "aac",
        "-ar",
        "44100",
        "-b:a",
        "128k",
        "-flags",
        "+global_header",
        "-f",
        "flv",
        "rtmp://localhost/live/stream"
    ];

    assert_eq!(enc_cmd, test_cmd);
}

#[test]
fn video_audio_filter3_stream() {
    let mut config = PlayoutConfig::new(Some("../assets/ffplayout.yml".to_string()));
    config.out.mode = Stream;
    config.processing.add_logo = false;
    config.text.add_text = true;
    config.text.fontfile = String::new();
    config.out.output_cmd = Some(vec_strings![
        "-filter_complex",
        "[0:v]null[o];movie=/path/to/lower_third.png[l];[o][l]overlay=shortest=1[v_out0]",
        "-map",
        "[v_out0]",
        "-c:v",
        "libx264",
        "-c:a",
        "aac",
        "-ar",
        "44100",
        "-b:a",
        "128k",
        "-flags",
        "+global_header",
        "-f",
        "flv",
        "rtmp://localhost/live/stream"
    ]);

    let mut media = Media::new(0, "", false);
    media.unit = Encoder;
    media.add_filter(&config, &None);

    let socket = config
        .text
        .zmq_stream_socket
        .clone()
        .unwrap()
        .replace(':', "\\:");

    let enc_prefix = vec_strings![
        "-hide_banner",
        "-nostats",
        "-v",
        "level+error",
        "-re",
        "-i",
        "pipe:0"
    ];

    let enc_cmd = prepare_output_cmd(&config, enc_prefix, &media.filter);

    let test_cmd = vec_strings![
        "-hide_banner",
        "-nostats",
        "-v",
        "level+error",
        "-re",
        "-i",
        "pipe:0",
        "-filter_complex",
        format!("[0:v:0]zmq=b=tcp\\\\://'{socket}',drawtext@dyntext=text=''[vout0];[vout0]null[o];movie=/path/to/lower_third.png[l];[o][l]overlay=shortest=1[v_out0]"),
        "-map",
        "[v_out0]",
        "-c:v",
        "libx264",
        "-c:a",
        "aac",
        "-ar",
        "44100",
        "-b:a",
        "128k",
        "-flags",
        "+global_header",
        "-f",
        "flv",
        "rtmp://localhost/live/stream"
    ];

    assert_eq!(enc_cmd, test_cmd);
}

#[test]
fn video_audio_filter4_stream() {
    let mut config = PlayoutConfig::new(Some("../assets/ffplayout.yml".to_string()));
    config.out.mode = Stream;
    config.processing.add_logo = false;
    config.text.add_text = true;
    config.text.fontfile = String::new();
    config.out.output_cmd = Some(vec_strings![
        "-filter_complex",
        "[0:v]null[o];movie=/path/to/lower_third.png[l];[o][l]overlay=shortest=1[v_out0];[0:a:0]volume=0.2[a_out0]",
        "-map",
        "[v_out0]",
        "-map",
        "[a_out0]",
        "-c:v",
        "libx264",
        "-c:a",
        "aac",
        "-ar",
        "44100",
        "-b:a",
        "128k",
        "-flags",
        "+global_header",
        "-f",
        "flv",
        "rtmp://localhost/live/stream"
    ]);

    let mut media = Media::new(0, "", false);
    media.unit = Encoder;
    media.add_filter(&config, &None);

    let socket = config
        .text
        .zmq_stream_socket
        .clone()
        .unwrap()
        .replace(':', "\\:");

    let enc_prefix = vec_strings![
        "-hide_banner",
        "-nostats",
        "-v",
        "level+error",
        "-re",
        "-i",
        "pipe:0"
    ];

    let enc_cmd = prepare_output_cmd(&config, enc_prefix, &media.filter);

    let test_cmd = vec_strings![
        "-hide_banner",
        "-nostats",
        "-v",
        "level+error",
        "-re",
        "-i",
        "pipe:0",
        "-filter_complex",
        format!("[0:v:0]zmq=b=tcp\\\\://'{socket}',drawtext@dyntext=text=''[vout0];[vout0]null[o];movie=/path/to/lower_third.png[l];[o][l]overlay=shortest=1[v_out0];[0:a:0]volume=0.2[a_out0]"),
        "-map",
        "[v_out0]",
        "-map",
        "[a_out0]",
        "-c:v",
        "libx264",
        "-c:a",
        "aac",
        "-ar",
        "44100",
        "-b:a",
        "128k",
        "-flags",
        "+global_header",
        "-f",
        "flv",
        "rtmp://localhost/live/stream"
    ];

    assert_eq!(enc_cmd, test_cmd);
}

#[test]
fn video_dual_audio_stream() {
    let mut config = PlayoutConfig::new(Some("../assets/ffplayout.yml".to_string()));
    config.out.mode = Stream;
    config.processing.add_logo = false;
    config.processing.audio_tracks = 2;
    config.text.add_text = false;
    config.out.output_cmd = Some(vec_strings![
        "-c:v",
        "libx264",
        "-c:a",
        "aac",
        "-ar",
        "44100",
        "-b:a",
        "128k",
        "-flags",
        "+global_header",
        "-f",
        "mpegts",
        "srt://127.0.0.1:40051"
    ]);

    let mut media = Media::new(0, "", false);
    media.unit = Encoder;
    media.add_filter(&config, &None);

    let enc_prefix = vec_strings![
        "-hide_banner",
        "-nostats",
        "-v",
        "level+error",
        "-re",
        "-i",
        "pipe:0"
    ];

    let enc_cmd = prepare_output_cmd(&config, enc_prefix, &media.filter);

    let test_cmd = vec_strings![
        "-hide_banner",
        "-nostats",
        "-v",
        "level+error",
        "-re",
        "-i",
        "pipe:0",
        "-map",
        "0:v",
        "-map",
        "0:a:0",
        "-map",
        "0:a:1",
        "-c:v",
        "libx264",
        "-c:a",
        "aac",
        "-ar",
        "44100",
        "-b:a",
        "128k",
        "-flags",
        "+global_header",
        "-f",
        "mpegts",
        "srt://127.0.0.1:40051"
    ];

    assert_eq!(enc_cmd, test_cmd);
}

#[test]
fn video_dual_audio_filter_stream() {
    let mut config = PlayoutConfig::new(Some("../assets/ffplayout.yml".to_string()));
    config.out.mode = Stream;
    config.processing.add_logo = false;
    config.processing.audio_tracks = 2;
    config.text.fontfile = String::new();
    config.out.output_cmd = Some(vec_strings![
        "-c:v",
        "libx264",
        "-c:a",
        "aac",
        "-ar",
        "44100",
        "-b:a",
        "128k",
        "-flags",
        "+global_header",
        "-f",
        "mpegts",
        "srt://127.0.0.1:40051"
    ]);

    let enc_prefix = vec_strings![
        "-hide_banner",
        "-nostats",
        "-v",
        "level+error",
        "-re",
        "-i",
        "pipe:0"
    ];

    let socket = config
        .text
        .zmq_stream_socket
        .clone()
        .unwrap()
        .replace(':', "\\:");

    let mut media = Media::new(0, "", false);
    media.unit = Encoder;
    media.add_filter(&config, &None);

    let enc_cmd = prepare_output_cmd(&config, enc_prefix, &media.filter);

    let test_cmd = vec_strings![
        "-hide_banner",
        "-nostats",
        "-v",
        "level+error",
        "-re",
        "-i",
        "pipe:0",
        "-filter_complex",
        format!("[0:v:0]zmq=b=tcp\\\\://'{socket}',drawtext@dyntext=text=''[vout0]"),
        "-map",
        "[vout0]",
        "-map",
        "0:a:0",
        "-map",
        "0:a:1",
        "-c:v",
        "libx264",
        "-c:a",
        "aac",
        "-ar",
        "44100",
        "-b:a",
        "128k",
        "-flags",
        "+global_header",
        "-f",
        "mpegts",
        "srt://127.0.0.1:40051"
    ];

    assert_eq!(enc_cmd, test_cmd);
}

#[test]
fn video_audio_multi_stream() {
    let mut config = PlayoutConfig::new(Some("../assets/ffplayout.yml".to_string()));
    config.out.mode = Stream;
    config.processing.add_logo = false;
    config.out.output_cmd = Some(vec_strings![
        "-c:v",
        "libx264",
        "-c:a",
        "aac",
        "-ar",
        "44100",
        "-b:a",
        "128k",
        "-flags",
        "+global_header",
        "-f",
        "flv",
        "rtmp://localhost/live/stream",
        "-s",
        "512x288",
        "-c:v",
        "libx264",
        "-c:a",
        "aac",
        "-ar",
        "44100",
        "-b:a",
        "128k",
        "-flags",
        "+global_header",
        "-f",
        "flv",
        "rtmp://localhost:1936/live/stream"
    ]);

    let enc_prefix = vec_strings![
        "-hide_banner",
        "-nostats",
        "-v",
        "level+error",
        "-re",
        "-i",
        "pipe:0"
    ];

    let enc_cmd = prepare_output_cmd(&config, enc_prefix, &None);

    let test_cmd = vec_strings![
        "-hide_banner",
        "-nostats",
        "-v",
        "level+error",
        "-re",
        "-i",
        "pipe:0",
        "-c:v",
        "libx264",
        "-c:a",
        "aac",
        "-ar",
        "44100",
        "-b:a",
        "128k",
        "-flags",
        "+global_header",
        "-f",
        "flv",
        "rtmp://localhost/live/stream",
        "-s",
        "512x288",
        "-c:v",
        "libx264",
        "-c:a",
        "aac",
        "-ar",
        "44100",
        "-b:a",
        "128k",
        "-flags",
        "+global_header",
        "-f",
        "flv",
        "rtmp://localhost:1936/live/stream"
    ];

    assert_eq!(enc_cmd, test_cmd);
}

#[test]
fn video_dual_audio_multi_stream() {
    let mut config = PlayoutConfig::new(Some("../assets/ffplayout.yml".to_string()));
    config.out.mode = Stream;
    config.processing.add_logo = false;
    config.processing.audio_tracks = 2;
    config.out.output_cmd = Some(vec_strings![
        "-map",
        "0:v",
        "-map",
        "0:a:0",
        "-map",
        "0:a:1",
        "-c:v",
        "libx264",
        "-c:a",
        "aac",
        "-ar",
        "44100",
        "-b:a",
        "128k",
        "-flags",
        "+global_header",
        "-f",
        "mpegts",
        "srt://127.0.0.1:40051",
        "-map",
        "0:v",
        "-map",
        "0:a:0",
        "-map",
        "0:a:1",
        "-s",
        "512x288",
        "-c:v",
        "libx264",
        "-c:a",
        "aac",
        "-ar",
        "44100",
        "-b:a",
        "128k",
        "-flags",
        "+global_header",
        "-f",
        "mpegts",
        "srt://127.0.0.1:40052"
    ]);

    let enc_prefix = vec_strings![
        "-hide_banner",
        "-nostats",
        "-v",
        "level+error",
        "-re",
        "-i",
        "pipe:0"
    ];

    let enc_cmd = prepare_output_cmd(&config, enc_prefix, &None);

    let test_cmd = vec_strings![
        "-hide_banner",
        "-nostats",
        "-v",
        "level+error",
        "-re",
        "-i",
        "pipe:0",
        "-map",
        "0:v",
        "-map",
        "0:a:0",
        "-map",
        "0:a:1",
        "-c:v",
        "libx264",
        "-c:a",
        "aac",
        "-ar",
        "44100",
        "-b:a",
        "128k",
        "-flags",
        "+global_header",
        "-f",
        "mpegts",
        "srt://127.0.0.1:40051",
        "-map",
        "0:v",
        "-map",
        "0:a:0",
        "-map",
        "0:a:1",
        "-s",
        "512x288",
        "-c:v",
        "libx264",
        "-c:a",
        "aac",
        "-ar",
        "44100",
        "-b:a",
        "128k",
        "-flags",
        "+global_header",
        "-f",
        "mpegts",
        "srt://127.0.0.1:40052"
    ];

    assert_eq!(enc_cmd, test_cmd);
}

#[test]
fn video_dual_audio_multi_filter_stream() {
    let mut config = PlayoutConfig::new(Some("../assets/ffplayout.yml".to_string()));
    config.out.mode = Stream;
    config.processing.add_logo = false;
    config.processing.audio_tracks = 2;
    config.text.fontfile = String::new();
    config.out.output_cmd = Some(vec_strings![
        "-map",
        "0:v",
        "-map",
        "0:a:0",
        "-map",
        "0:a:1",
        "-c:v",
        "libx264",
        "-c:a",
        "aac",
        "-ar",
        "44100",
        "-b:a",
        "128k",
        "-flags",
        "+global_header",
        "-f",
        "mpegts",
        "srt://127.0.0.1:40051",
        "-map",
        "0:v",
        "-map",
        "0:a:0",
        "-map",
        "0:a:1",
        "-s",
        "512x288",
        "-c:v",
        "libx264",
        "-c:a",
        "aac",
        "-ar",
        "44100",
        "-b:a",
        "128k",
        "-flags",
        "+global_header",
        "-f",
        "mpegts",
        "srt://127.0.0.1:40052"
    ]);

    let enc_prefix = vec_strings![
        "-hide_banner",
        "-nostats",
        "-v",
        "level+error",
        "-re",
        "-i",
        "pipe:0"
    ];

    let socket = config
        .text
        .zmq_stream_socket
        .clone()
        .unwrap()
        .replace(':', "\\:");

    let mut media = Media::new(0, "", false);
    media.unit = Encoder;
    media.add_filter(&config, &None);

    let enc_cmd = prepare_output_cmd(&config, enc_prefix, &media.filter);

    let test_cmd = vec_strings![
        "-hide_banner",
        "-nostats",
        "-v",
        "level+error",
        "-re",
        "-i",
        "pipe:0",
        "-filter_complex",
        format!("[0:v:0]zmq=b=tcp\\\\://'{socket}',drawtext@dyntext=text=''[vout0]"),
        "-map",
        "[vout0]",
        "-map",
        "0:a:0",
        "-map",
        "0:a:1",
        "-c:v",
        "libx264",
        "-c:a",
        "aac",
        "-ar",
        "44100",
        "-b:a",
        "128k",
        "-flags",
        "+global_header",
        "-f",
        "mpegts",
        "srt://127.0.0.1:40051",
        "-map",
        "[vout0]",
        "-map",
        "0:a:0",
        "-map",
        "0:a:1",
        "-s",
        "512x288",
        "-c:v",
        "libx264",
        "-c:a",
        "aac",
        "-ar",
        "44100",
        "-b:a",
        "128k",
        "-flags",
        "+global_header",
        "-f",
        "mpegts",
        "srt://127.0.0.1:40052"
    ];

    assert_eq!(enc_cmd, test_cmd);
}

#[test]
fn video_audio_hls() {
    let mut config = PlayoutConfig::new(Some("../assets/ffplayout.yml".to_string()));
    config.out.mode = HLS;
    config.processing.add_logo = false;
    config.text.add_text = false;
    config.out.output_cmd = Some(vec_strings![
        "-c:v",
        "libx264",
        "-c:a",
        "aac",
        "-ar",
        "44100",
        "-b:a",
        "128k",
        "-flags",
        "+cgop",
        "-f",
        "hls",
        "-hls_time",
        "6",
        "-hls_list_size",
        "600",
        "-hls_flags",
        "append_list+delete_segments+omit_endlist",
        "-hls_segment_filename",
        "/usr/share/ffplayout/public/live/stream-%d.ts",
        "/usr/share/ffplayout/public/live/stream.m3u8"
    ]);

    let media_obj = Media::new(0, "./assets/with_audio.mp4", true);
    let media = gen_source(&config, media_obj, &None);

    let enc_prefix = vec_strings![
        "-hide_banner",
        "-nostats",
        "-v",
        "level+error",
        "-re",
        "-i",
        "./assets/with_audio.mp4"
    ];

    let enc_cmd = prepare_output_cmd(&config, enc_prefix, &media.filter);

    let test_cmd = vec_strings![
        "-hide_banner",
        "-nostats",
        "-v",
        "level+error",
        "-re",
        "-i",
        "./assets/with_audio.mp4",
        "-filter_complex",
        "[0:v:0]scale=1024:576,realtime=speed=1[vout0];[0:a:0]anull[aout0]",
        "-map",
        "[vout0]",
        "-map",
        "[aout0]",
        "-c:v",
        "libx264",
        "-c:a",
        "aac",
        "-ar",
        "44100",
        "-b:a",
        "128k",
        "-flags",
        "+cgop",
        "-f",
        "hls",
        "-hls_time",
        "6",
        "-hls_list_size",
        "600",
        "-hls_flags",
        "append_list+delete_segments+omit_endlist",
        "-hls_segment_filename",
        "/usr/share/ffplayout/public/live/stream-%d.ts",
        "/usr/share/ffplayout/public/live/stream.m3u8"
    ];

    assert_eq!(enc_cmd, test_cmd);
}

#[test]
fn video_audio_sub_meta_hls() {
    let mut config = PlayoutConfig::new(Some("../assets/ffplayout.yml".to_string()));
    config.out.mode = HLS;
    config.processing.add_logo = false;
    config.text.add_text = false;
    config.out.output_cmd = Some(vec_strings![
        "-map",
        "0:s:0",
        "-map_metadata",
        "0",
        "-c:v",
        "libx264",
        "-c:a",
        "aac",
        "-ar",
        "44100",
        "-b:a",
        "128k",
        "-flags",
        "+cgop",
        "-f",
        "hls",
        "-hls_time",
        "6",
        "-hls_list_size",
        "600",
        "-hls_flags",
        "append_list+delete_segments+omit_endlist",
        "-hls_segment_filename",
        "/usr/share/ffplayout/public/live/stream-%d.ts",
        "/usr/share/ffplayout/public/live/stream.m3u8"
    ]);

    let media_obj = Media::new(0, "./assets/with_audio.mp4", true);
    let media = gen_source(&config, media_obj, &None);

    let enc_prefix = vec_strings![
        "-hide_banner",
        "-nostats",
        "-v",
        "level+error",
        "-re",
        "-i",
        "./assets/with_audio.mp4"
    ];

    let enc_cmd = prepare_output_cmd(&config, enc_prefix, &media.filter);

    let test_cmd = vec_strings![
        "-hide_banner",
        "-nostats",
        "-v",
        "level+error",
        "-re",
        "-i",
        "./assets/with_audio.mp4",
        "-filter_complex",
        "[0:v:0]scale=1024:576,realtime=speed=1[vout0];[0:a:0]anull[aout0]",
        "-map",
        "[vout0]",
        "-map",
        "[aout0]",
        "-map",
        "0:s:0",
        "-map_metadata",
        "0",
        "-c:v",
        "libx264",
        "-c:a",
        "aac",
        "-ar",
        "44100",
        "-b:a",
        "128k",
        "-flags",
        "+cgop",
        "-f",
        "hls",
        "-hls_time",
        "6",
        "-hls_list_size",
        "600",
        "-hls_flags",
        "append_list+delete_segments+omit_endlist",
        "-hls_segment_filename",
        "/usr/share/ffplayout/public/live/stream-%d.ts",
        "/usr/share/ffplayout/public/live/stream.m3u8"
    ];

    assert_eq!(enc_cmd, test_cmd);
}

#[test]
fn video_multi_audio_hls() {
    let mut config = PlayoutConfig::new(Some("../assets/ffplayout.yml".to_string()));
    config.out.mode = HLS;
    config.processing.add_logo = false;
    config.processing.audio_tracks = 2;
    config.text.add_text = false;
    config.out.output_cmd = Some(vec_strings![
        "-c:v",
        "libx264",
        "-c:a",
        "aac",
        "-ar",
        "44100",
        "-b:a",
        "128k",
        "-flags",
        "+cgop",
        "-f",
        "hls",
        "-hls_time",
        "6",
        "-hls_list_size",
        "600",
        "-hls_flags",
        "append_list+delete_segments+omit_endlist",
        "-hls_segment_filename",
        "/usr/share/ffplayout/public/live/stream-%d.ts",
        "/usr/share/ffplayout/public/live/stream.m3u8"
    ]);

    let media_obj = Media::new(0, "./assets/dual_audio.mp4", true);
    let media = gen_source(&config, media_obj, &None);

    let enc_prefix = vec_strings![
        "-hide_banner",
        "-nostats",
        "-v",
        "level+error",
        "-re",
        "-i",
        "./assets/dual_audio.mp4"
    ];

    let enc_cmd = prepare_output_cmd(&config, enc_prefix, &media.filter);

    let test_cmd = vec_strings![
        "-hide_banner",
        "-nostats",
        "-v",
        "level+error",
        "-re",
        "-i",
        "./assets/dual_audio.mp4",
        "-filter_complex",
        "[0:v:0]scale=1024:576,realtime=speed=1[vout0];[0:a:0]anull[aout0];[0:a:1]anull[aout1]",
        "-map",
        "[vout0]",
        "-map",
        "[aout0]",
        "-map",
        "[aout1]",
        "-c:v",
        "libx264",
        "-c:a",
        "aac",
        "-ar",
        "44100",
        "-b:a",
        "128k",
        "-flags",
        "+cgop",
        "-f",
        "hls",
        "-hls_time",
        "6",
        "-hls_list_size",
        "600",
        "-hls_flags",
        "append_list+delete_segments+omit_endlist",
        "-hls_segment_filename",
        "/usr/share/ffplayout/public/live/stream-%d.ts",
        "/usr/share/ffplayout/public/live/stream.m3u8"
    ];

    assert_eq!(enc_cmd, test_cmd);
}

#[test]
fn multi_video_audio_hls() {
    let mut config = PlayoutConfig::new(Some("../assets/ffplayout.yml".to_string()));
    config.out.mode = HLS;
    config.processing.add_logo = false;
    config.text.add_text = false;
    config.out.output_cmd = Some(vec_strings![
        "-filter_complex",
        "[0:v]split=2[v1_out][v2];[v2]scale=w=512:h=288[v2_out];[0:a]asplit=2[a1][a2]",
        "-map",
        "[v1_out]",
        "-map",
        "[a1]",
        "-c:v",
        "libx264",
        "-flags",
        "+cgop",
        "-c:a",
        "aac",
        "-map",
        "[v2_out]",
        "-map",
        "[a2]",
        "-c:v:1",
        "libx264",
        "-flags",
        "+cgop",
        "-c:a:1",
        "aac",
        "-f",
        "hls",
        "-hls_time",
        "6",
        "-hls_list_size",
        "600",
        "-hls_flags",
        "append_list+delete_segments+omit_endlist",
        "-hls_segment_filename",
        "/usr/share/ffplayout/public/live/stream_%v-%d.ts",
        "-master_pl_name",
        "master.m3u8",
        "-var_stream_map",
        "v:0,a:0,name:720p v:1,a:1,name:288p",
        "/usr/share/ffplayout/public/live/stream_%v.m3u8"
    ]);

    let media_obj = Media::new(0, "./assets/with_audio.mp4", true);
    let media = gen_source(&config, media_obj, &None);

    let enc_prefix = vec_strings![
        "-hide_banner",
        "-nostats",
        "-v",
        "level+error",
        "-re",
        "-i",
        "./assets/with_audio.mp4"
    ];

    let enc_cmd = prepare_output_cmd(&config, enc_prefix, &media.filter);

    let test_cmd = vec_strings![
        "-hide_banner",
        "-nostats",
        "-v",
        "level+error",
        "-re",
        "-i",
        "./assets/with_audio.mp4",
        "-filter_complex",
        "[0:v:0]scale=1024:576,realtime=speed=1[vout0];[0:a:0]anull[aout0];[vout0]split=2[v1_out][v2];[v2]scale=w=512:h=288[v2_out];[aout0]asplit=2[a1][a2]",
        "-map",
        "[v1_out]",
        "-map",
        "[a1]",
        "-c:v",
        "libx264",
        "-flags",
        "+cgop",
        "-c:a",
        "aac",
        "-map",
        "[v2_out]",
        "-map",
        "[a2]",
        "-c:v:1",
        "libx264",
        "-flags",
        "+cgop",
        "-c:a:1",
        "aac",
        "-f",
        "hls",
        "-hls_time",
        "6",
        "-hls_list_size",
        "600",
        "-hls_flags",
        "append_list+delete_segments+omit_endlist",
        "-hls_segment_filename",
        "/usr/share/ffplayout/public/live/stream_%v-%d.ts",
        "-master_pl_name",
        "master.m3u8",
        "-var_stream_map",
        "v:0,a:0,name:720p v:1,a:1,name:288p",
        "/usr/share/ffplayout/public/live/stream_%v.m3u8"
    ];

    assert_eq!(enc_cmd, test_cmd);
}

#[test]
fn multi_video_multi_audio_hls() {
    let mut config = PlayoutConfig::new(Some("../assets/ffplayout.yml".to_string()));
    config.out.mode = HLS;
    config.processing.add_logo = false;
    config.processing.audio_tracks = 2;
    config.text.add_text = false;
    config.out.output_cmd = Some(vec_strings![
        "-filter_complex",
        "[0:v]split=2[v1_out][v2];[v2]scale=w=512:h=288[v2_out];[0:a:0]asplit=2[a_0_1][a_0_2];[0:a:1]asplit=2[a_1_1][a_1_2]",
        "-map",
        "[v1_out]",
        "-map",
        "[a_0_1]",
        "-map",
        "[a_1_1]",
        "-c:v",
        "libx264",
        "-flags",
        "+cgop",
        "-c:a",
        "aac",
        "-map",
        "[v2_out]",
        "-map",
        "[a_0_2]",
        "-map",
        "[a_1_2]",
        "-c:v:1",
        "libx264",
        "-flags",
        "+cgop",
        "-c:a:1",
        "aac",
        "-f",
        "hls",
        "-hls_time",
        "6",
        "-hls_list_size",
        "600",
        "-hls_flags",
        "append_list+delete_segments+omit_endlist",
        "-hls_segment_filename",
        "/usr/share/ffplayout/public/live/stream_%v-%d.ts",
        "-master_pl_name",
        "master.m3u8",
        "-var_stream_map",
        "v:0,a:0,a:1,name:720p v:1,a:2,a:3,name:288p",
        "/usr/share/ffplayout/public/live/stream_%v.m3u8"
    ]);

    let media_obj = Media::new(0, "./assets/dual_audio.mp4", true);
    let media = gen_source(&config, media_obj, &None);

    let enc_prefix = vec_strings![
        "-hide_banner",
        "-nostats",
        "-v",
        "level+error",
        "-re",
        "-i",
        "./assets/dual_audio.mp4"
    ];

    let enc_cmd = prepare_output_cmd(&config, enc_prefix, &media.filter);

    let test_cmd = vec_strings![
        "-hide_banner",
        "-nostats",
        "-v",
        "level+error",
        "-re",
        "-i",
        "./assets/dual_audio.mp4",
        "-filter_complex",
        "[0:v:0]scale=1024:576,realtime=speed=1[vout0];[0:a:0]anull[aout0];[0:a:1]anull[aout1];[vout0]split=2[v1_out][v2];[v2]scale=w=512:h=288[v2_out];[aout0]asplit=2[a_0_1][a_0_2];[aout1]asplit=2[a_1_1][a_1_2]",
        "-map",
        "[v1_out]",
        "-map",
        "[a_0_1]",
        "-map",
        "[a_1_1]",
        "-c:v",
        "libx264",
        "-flags",
        "+cgop",
        "-c:a",
        "aac",
        "-map",
        "[v2_out]",
        "-map",
        "[a_0_2]",
        "-map",
        "[a_1_2]",
        "-c:v:1",
        "libx264",
        "-flags",
        "+cgop",
        "-c:a:1",
        "aac",
        "-f",
        "hls",
        "-hls_time",
        "6",
        "-hls_list_size",
        "600",
        "-hls_flags",
        "append_list+delete_segments+omit_endlist",
        "-hls_segment_filename",
        "/usr/share/ffplayout/public/live/stream_%v-%d.ts",
        "-master_pl_name",
        "master.m3u8",
        "-var_stream_map",
        "v:0,a:0,a:1,name:720p v:1,a:2,a:3,name:288p",
        "/usr/share/ffplayout/public/live/stream_%v.m3u8"
    ];

    assert_eq!(enc_cmd, test_cmd);
}
