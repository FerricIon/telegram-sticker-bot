use crate::{errors::*, types::*};
use image::{
    imageops::FilterType, io::Reader as ImageReader, GenericImage, ImageOutputFormat, Rgba,
    RgbaImage,
};
use std::{io::Cursor, path::Path, process::Stdio};
use teloxide::{
    adaptors::AutoSend, net::Download, prelude::Requester, types::File as TgFile, types::InputFile,
    Bot,
};
use tempfile::NamedTempFile;
use tokio::{fs::File, process::Command};
use ubyte::ToByteUnit;

fn convert_image(path: &Path, config: &mut Option<ConvertConfig>) -> anyhow::Result<Vec<u8>> {
    let img = ImageReader::open(path)?.with_guessed_format()?.decode()?;
    let (width, height) = (img.width(), img.height());

    if config.is_none() {
        config.replace((width, height).into());
    }
    let config = config.unwrap();

    let mut img = img.resize(
        512,
        match config.size {
            ConvertSize::Small => 128,
            ConvertSize::Medium => 256,
            ConvertSize::Large => 512,
        },
        FilterType::CatmullRom,
    );
    let (width, height) = (img.width(), img.height());
    if width < 512 && height < 512 {
        let mut canvas = RgbaImage::from_pixel(512, height, Rgba([0; 4]));
        canvas.copy_from(
            &img,
            match config.position {
                Some(ConvertPosition::Left) | None => 0,
                Some(ConvertPosition::Center) => (512 - width) / 2,
                Some(ConvertPosition::Right) => 512 - width,
            },
            0,
        )?;
        img = canvas.into();
    }
    let mut converted: Vec<u8> = Vec::new();
    img.write_to(&mut Cursor::new(&mut converted), ImageOutputFormat::Png)?;
    Ok(converted)
}

async fn convert_video(path: &Path, config: &mut Option<ConvertConfig>) -> anyhow::Result<Vec<u8>> {
    let probe = Command::new("ffprobe")
        .args([
            "-select_streams",
            "v",
            "-show_entries",
            "stream=width,height:format=duration",
            "-of",
            "default=nokey=1:noprint_wrappers=1",
            path.to_str().expect("path of tempfile"),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await?
        .stdout;
    let probe: Vec<_> = std::str::from_utf8(&probe)?.split('\n').collect();

    let width = *probe.get(0).unwrap_or(&"");
    let width: u32 = width
        .parse()
        .map_err(|_| ConvertError::Format("width".to_string(), width.to_string()))?;
    let height = *probe.get(1).unwrap_or(&"");
    let height: u32 = height
        .parse()
        .map_err(|_| ConvertError::Format("height".to_string(), width.to_string()))?;
    let duration = *probe.get(2).unwrap_or(&"");
    let duration: f32 = duration
        .parse()
        .map_err(|_| ConvertError::Format("duration".to_string(), width.to_string()))?;
    if duration > 3.0 {
        return Err(ConvertError::Duration(duration).into());
    }

    if config.is_none() {
        config.replace((width, height).into());
    }
    let config = config.unwrap();

    let scale = format!(
        ",scale=512:{}:force_original_aspect_ratio=decrease",
        match config.size {
            ConvertSize::Small => 128,
            ConvertSize::Medium => 256,
            ConvertSize::Large => 512,
        }
    );
    let (width, height) = config.size.resize(width, height);
    let pad = if width < 512 && height < 512 {
        match config.position {
            Some(ConvertPosition::Left) | None => format!(",pad=512:{}:0:0:black@0", height),
            Some(ConvertPosition::Center) => format!(",pad=512:{}:(ow-iw)/2:0:black@0", height),
            Some(ConvertPosition::Right) => format!(",pad=512:{}:(ow-iw):0:black@0", height),
        }
    } else {
        String::new()
    };
    let vf = format!("format=yuva420p,fps=30{}{}", scale, pad);

    #[rustfmt::skip]
    let args = vec![
        "-i", path.to_str().expect("path of tempfile"),
        "-c:v", "libvpx-vp9", "-b:v", "0", "-crf", "35",
        "-an", "-vf", &vf,
        "-f", "webm", "-",
    ];
    log::debug!("Calling ffmpeg with {:?}.", args);
    let checksum = Command::new("md5sum")
        .arg(path.to_str().expect("path of tempfile"))
        .stdout(Stdio::piped())
        .output()
        .await?
        .stdout;
    log::debug!("Checksum: {}.", std::str::from_utf8(&checksum).unwrap());
    Ok(Command::new("ffmpeg")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await?
        .stdout)
}

pub async fn convert(
    bot: &AutoSend<Bot>,
    file_id: &str,
    media_type: MediaType,
    config: &mut Option<ConvertConfig>,
) -> Result<InputFile, ConvertError> {
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

    let (file_name, data) = match media_type {
        MediaType::Image => ("sticker.png", convert_image(&tmp_path, config)),
        MediaType::Video => ("sticker.webm", convert_video(&tmp_path, config).await),
    };

    Ok(InputFile::memory(data.map_err(ConvertError::wrap)?).file_name(file_name))
}
