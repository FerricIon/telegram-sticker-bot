use crate::{errors::*, types::*};
use image::{
    imageops::FilterType, io::Reader as ImageReader, GenericImage, ImageOutputFormat, Rgba,
    RgbaImage,
};
use std::{
    io::Cursor,
    path::Path,
    process::{Output, Stdio},
    str::FromStr,
};
use teloxide::{
    adaptors::AutoSend, net::Download, prelude::Requester, types::File as TgFile, types::InputFile,
    Bot,
};
use tempfile::NamedTempFile;
use tokio::{fs::File, process::Command};
use ubyte::ToByteUnit;

fn convert_image(path: &Path, layout: Option<LayoutProp>) -> anyhow::Result<(Vec<u8>, LayoutProp)> {
    let img = ImageReader::open(path)?.with_guessed_format()?.decode()?;
    let (width, height) = (img.width(), img.height());

    let layout = layout.unwrap_or((width, height).into());
    let (b_width, b_height, pad_x) = layout.resize(width, height);

    let img = img.resize(b_width, b_height, FilterType::CatmullRom);
    let img = pad_x
        .and_then(|x| {
            let mut canvas = RgbaImage::from_pixel(b_width, b_height, Rgba([0; 4]));
            canvas.copy_from(&img, x, 0).map(|_| canvas.into()).ok()
        })
        .unwrap_or(img);

    let mut converted: Vec<u8> = Vec::new();
    img.write_to(&mut Cursor::new(&mut converted), ImageOutputFormat::Png)?;
    Ok((converted, layout))
}

async fn convert_video(
    path: &Path,
    layout: Option<LayoutProp>,
    playback: Option<PlaybackProp>,
) -> anyhow::Result<(Vec<u8>, LayoutProp, PlaybackProp)> {
    log::debug!("convert a video with {:?}, {:?}...", layout, playback);
    #[rustfmt::skip]
    let args = [
        "-select_streams", "v", "-show_entries", "stream=width,height:format=duration",
        "-of", "default=nokey=1:noprint_wrappers=1",
        path.to_str().expect("path of tempfile"),
    ];
    let Output { stdout, status, .. } = Command::new("ffprobe")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await?;
    anyhow::ensure!(status.success(), "ffprobe exited with {:?}", status.code());
    let probe: Vec<_> = std::str::from_utf8(&stdout)?.split('\n').collect();

    fn parse<T: FromStr>(s: Option<&&str>, name: &str) -> Result<T, ConvertError> {
        let s = *s.unwrap_or(&"");
        s.parse()
            .map_err(|_| ConvertError::Format(name.to_owned(), s.to_owned()))
    }
    let width: u32 = parse(probe.get(0), "width")?;
    let height: u32 = parse(probe.get(1), "height")?;
    let duration: f32 = parse(probe.get(2), "duration")?;
    log::debug!("video metadata: {}*{}, {:.3}s", width, height, duration);

    let layout = layout.unwrap_or((width, height).into());
    let playback = playback.unwrap_or(PlaybackProp { speed_up: false });
    anyhow::ensure!(
        duration <= 3.0 || playback.speed_up,
        ConvertError::Duration(duration)
    );

    let (b_width, b_height, pad_x) = layout.resize(width, height);
    let scale = format!(
        ",scale={}:{}:force_original_aspect_ratio=decrease",
        b_width, b_height
    );
    let pad = pad_x
        .map(|x| format!(",pad={}:{}:{}:0:black@0", b_width, b_height, x))
        .unwrap_or_default();
    let itsscale = if playback.speed_up {
        3.0 / duration
    } else {
        1.0
    };

    let vf = format!("format=yuva420p,fps=30{}{}", scale, pad);
    log::debug!("ffmpeg vf: {}", vf);

    #[rustfmt::skip]
    let args =  [
        "-itsscale", &itsscale.to_string(),
        "-i", path.to_str().expect("path of tempfile"),
        "-c:v", "libvpx-vp9", "-b:v", "0", "-crf", "35",
        "-an", "-vf", &vf, "-f", "webm", "-",
    ];
    let Output { stdout, status, .. } = Command::new("ffmpeg")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await?;
    anyhow::ensure!(status.success(), "ffmpeg exited with {:?}", status.code());
    log::debug!("output length: {}.", stdout.len());
    Ok((stdout, layout, playback))
}

pub async fn convert(
    bot: &AutoSend<Bot>,
    file_id: &str,
    media_type: MediaType,
    layout: Option<LayoutProp>,
    playback: Option<PlaybackProp>,
) -> Result<(InputFile, LayoutProp, Option<PlaybackProp>), ConvertError> {
    let TgFile {
        file_path,
        file_size,
        ..
    } = bot.get_file(file_id).await.map_err(ConvertError::wrap)?;
    if file_size.bytes() > 5.mebibytes() {
        return Err(ConvertError::FileSize(file_size as u64));
    }

    let (tmp_file, tmp_path) = NamedTempFile::new()
        .expect("tempfile is created")
        .into_parts();
    let mut tmp_file: File = tmp_file.into();
    bot.download_file(&file_path, &mut tmp_file)
        .await
        .map_err(ConvertError::wrap)?;

    let (file_name, data, layout, playback) = match media_type {
        MediaType::Image => {
            let (data, layout) = convert_image(&tmp_path, layout).map_err(ConvertError::wrap)?;
            ("sticker.png", data, layout, None)
        }
        MediaType::Video => {
            let (data, layout, playback) = convert_video(&tmp_path, layout, playback)
                .await
                .map_err(ConvertError::wrap)?;
            ("sticker.webm", data, layout, Some(playback))
        }
    };

    Ok((
        InputFile::memory(data).file_name(file_name),
        layout,
        playback,
    ))
}
