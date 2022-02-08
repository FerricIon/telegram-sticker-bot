use crate::{convert::*, errors::*, types::*};
use enum_iterator::IntoEnumIterator;
use log::error;
use teloxide::{
    adaptors::AutoSend,
    payloads::{AnswerCallbackQuerySetters, SendMessageSetters},
    prelude2::*,
    types::{
        CallbackQuery, InlineKeyboardButton, InlineKeyboardMarkup, InputFile, InputMedia,
        InputMediaDocument, Message,
    },
    Bot, RequestError,
};

fn get_config(m: &Message) -> Option<ConvertConfig> {
    if let Some(markup) = m.reply_markup() {
        let size = ConvertSize::into_enum_iter()
            .zip(markup.inline_keyboard[0].iter())
            .find(|(size, button)| Ok(size.to_owned()) != button.text.parse())
            .map_or(ConvertSize::Large, |x| x.0);
        let position = if size != ConvertSize::Large {
            ConvertPosition::into_enum_iter()
                .zip(markup.inline_keyboard[1].iter())
                .find(|(position, button)| Ok(position.to_owned()) != button.text.parse())
                .map(|x| x.0)
        } else {
            None
        };
        Some(ConvertConfig { size, position })
    } else {
        None
    }
}

fn make_keyboard(config: ConvertConfig) -> InlineKeyboardMarkup {
    let mut keyboard: Vec<Vec<InlineKeyboardButton>> = Vec::new();
    keyboard.push(
        ConvertSize::into_enum_iter()
            .filter_map(|size| {
                if size != config.size {
                    Some(InlineKeyboardButton::callback(
                        size.to_string(),
                        size.to_string(),
                    ))
                } else {
                    None
                }
            })
            .collect(),
    );
    if config.size != ConvertSize::Large {
        keyboard.push(
            ConvertPosition::into_enum_iter()
                .filter_map(|position| {
                    if Some(position) != config.position {
                        Some(InlineKeyboardButton::callback(
                            position.to_string(),
                            position.to_string(),
                        ))
                    } else {
                        None
                    }
                })
                .collect(),
        );
    }

    InlineKeyboardMarkup::new(keyboard)
}

async fn convert_message(
    m: &Message,
    bot: &AutoSend<Bot>,
    config: &mut Option<ConvertConfig>,
) -> Result<InputFile, ConvertError> {
    let media = {
        if let Some(doc) = m.document() {
            let mime = doc.mime_type.clone();
            let mime_type = mime.as_ref().map(|x| x.type_());
            if mime == Some(mime::IMAGE_GIF) || mime_type == Some(mime::VIDEO) {
                Some((&doc.file_id, MediaType::Video))
            } else if mime_type == Some(mime::IMAGE) {
                Some((&doc.file_id, MediaType::Image))
            } else {
                None
            }
        } else if let Some(img) = m.photo().map(|x| x.last()).flatten() {
            Some((&img.file_id, MediaType::Image))
        } else if let Some(vid) = m.video() {
            Some((&vid.file_id, MediaType::Video))
        } else if let Some(anim) = m.animation() {
            Some((&anim.file_id, MediaType::Video))
        } else {
            None
        }
    };
    if let Some((file_id, media_type)) = media {
        convert(bot, file_id, media_type, config).await
    } else {
        Err(ConvertError::MediaType)
    }
}

pub async fn command_handler(
    m: Message,
    bot: AutoSend<Bot>,
    cmd: Command,
) -> Result<(), RequestError> {
    let text = match cmd {
    Command::Start => "Welcome\\! Please send me an image or a video clip\\.",
    Command::Help => "Send me an image or a video clip and I will convert it into the format required by @Stickers\\.
On successful convertion, you may forward the replied document to @Stickers to make your sticker set, or click on the buttons to change the conversion style:

\\- Sticker Size
  *Small* the converted sticker will fit in a box of 512px\\*128px and add transparent paddings
  *Medium* the converted sticker will fit in a box of 512px\\*256px and add transparent paddings
  *Large* the converted sticker will fit in a box of 512px\\*512px
\\- Sticker Positioning \\(for small and medium sized stickers\\)
  *Left* place the sticker on the left
  *Center* place the sticker in the middle
  *Rignt* place the sticker on the right
  
Maintainer: @ferricion
Github Repository: [telegram\\-sticker\\-bot](https://github.com/FerricIon/telegram-sticker-bot)"
    };

    bot.send_message(m.chat_id(), text)
        .parse_mode(teloxide::types::ParseMode::MarkdownV2)
        .await?;

    Ok(())
}

pub async fn message_handler(m: Message, bot: AutoSend<Bot>) -> Result<(), RequestError> {
    let mut config = None;
    match convert_message(&m, &bot, &mut config).await {
        Ok(document) => {
            bot.send_document(m.chat_id(), document)
                .reply_to_message_id(m.id)
                .reply_markup(make_keyboard(config.expect("config is set")))
                .await?;
        }
        Err(e) => {
            error!("{}", e);
            bot.send_message(m.chat_id(), e.to_string())
                .reply_to_message_id(m.id)
                .await?;
        }
    };

    Ok(())
}

pub async fn callback_handler(q: CallbackQuery, bot: AutoSend<Bot>) -> Result<(), RequestError> {
    let result = (|| async {
        let m = q.message.ok_or(ConfigError::Message)?;
        let config = get_config(&m).ok_or(ConfigError::Message)?;
        let config_string = q.data.unwrap_or_default();
        let size = config_string.parse::<ConvertSize>();
        let position = config_string.parse::<ConvertPosition>();
        let mut config = Some(match (config.size, size, position) {
            (_, Ok(ConvertSize::Large), _) => Ok(ConvertConfig {
                size: ConvertSize::Large,
                position: None,
            }),
            (_, Ok(size), _) => Ok(ConvertConfig {
                size,
                position: config.position.or(Some(ConvertPosition::Center)),
            }),
            (ConvertSize::Small | ConvertSize::Medium, _, Ok(position)) => Ok(ConvertConfig {
                size: config.size,
                position: Some(position),
            }),
            _ => Err(ConfigError::Parse(config_string)),
        }?);
        let document = convert_message(
            m.reply_to_message().ok_or(ConfigError::Reply)?,
            &bot,
            &mut config,
        )
        .await?;
        anyhow::Result::<_>::Ok((m, document, config))
    })()
    .await;

    match result {
        Ok((m, document, config)) => {
            bot.edit_message_media(
                m.chat_id(),
                m.id,
                InputMedia::Document(InputMediaDocument::new(document)),
            )
            .await?;
            bot.edit_message_reply_markup(m.chat_id(), m.id)
                .reply_markup(make_keyboard(config.expect("config is set")))
                .await?;
            bot.answer_callback_query(q.id).await?;
        }
        Err(e) => {
            error!("{}", e);
            bot.answer_callback_query(q.id).text(e.to_string()).await?;
        }
    }

    Ok(())
}