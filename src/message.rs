use crate::{convert::*, errors::*, types::*};
use teloxide::{
    adaptors::AutoSend,
    payloads::{
        AnswerCallbackQuerySetters, EditMessageCaptionSetters, SendDocumentSetters,
        SendMessageSetters,
    },
    prelude2::*,
    types::{
        CallbackQuery, InlineKeyboardButton, InlineKeyboardMarkup, InputFile, InputMedia,
        InputMediaDocument, Message, True,
    },
    Bot, RequestError,
};

fn get_props(m: &Message) -> (Option<LayoutProp>, Option<PlaybackProp>) {
    let caption = m.caption().unwrap_or("");
    let arr: Vec<_> = caption.split(';').collect();
    let layout: Option<LayoutProp> = arr.get(0).and_then(|s| s.parse().ok());
    let playback: Option<PlaybackProp> = arr.get(1).and_then(|s| s.parse().ok());
    (layout, playback)
}

fn make_caption(layout: LayoutProp, playback: Option<PlaybackProp>) -> String {
    log::debug!("make_caption: {:?}, {:?}", layout, playback);
    format!(
        "{};{}",
        layout,
        playback.map(|o| o.to_string()).unwrap_or_default()
    )
}

fn make_layout_keyboard(layout: LayoutProp) -> InlineKeyboardMarkup {
    log::debug!("make_layout_keyboard: {:?}", layout);
    use Callback::*;

    fn make_buttons(set: &[Callback], cur: Callback) -> Vec<InlineKeyboardButton> {
        set.iter()
            .filter_map(|&x| if x != cur { Some(x.into()) } else { None })
            .collect()
    }

    let size_callback = [Small, Medium, Large];
    let position_callback = [Left, Center, Right];

    let mut keyboard: Vec<Vec<InlineKeyboardButton>> = Vec::new();

    match layout {
        LayoutProp::Small(p) => {
            keyboard.push(make_buttons(&size_callback, Small));
            keyboard.push(make_buttons(&position_callback, p.into()));
        }
        LayoutProp::Medium(p) => {
            keyboard.push(make_buttons(&size_callback, Medium));
            keyboard.push(make_buttons(&position_callback, p.into()));
        }
        LayoutProp::Large => {
            keyboard.push(make_buttons(&size_callback, Large));
        }
    }

    InlineKeyboardMarkup::new(keyboard)
}

async fn convert_message(
    m: &Message,
    bot: &AutoSend<Bot>,
    layout: Option<LayoutProp>,
    playback: Option<PlaybackProp>,
) -> Result<(InputFile, LayoutProp, Option<PlaybackProp>), ConvertError> {
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
    log::debug!("convert {:?}...", media);
    if let Some((file_id, media_type)) = media {
        convert(bot, file_id, media_type, layout, playback).await
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
        Command::Start => r#"Welcome\! Please send me an image or a video clip\."#,
        Command::Help => {
            r#"Send me an image or a video clip and I will convert it into the format required by @Stickers\.
On successful convertion, you may forward the replied document to @Stickers to make your sticker set, or click on the buttons to change the conversion style:

\- Sticker Size
  *Small* the converted sticker will fit in a box of 512px\*128px and add transparent paddings
  *Medium* the converted sticker will fit in a box of 512px\*256px and add transparent paddings
  *Large* the converted sticker will fit in a box of 512px\*512px
\- Sticker Positioning \(for small and medium sized stickers\)
  *Left* place the sticker on the left
  *Center* place the sticker in the middle
  *Rignt* place the sticker on the right

Notes on translucent GIF:
Telegram will re\-encode all GIFs you send to *mpeg4* which does not have an alpha channel even if you send the GIF "without compression", and thus the bot could never get the original GIF\. If you need translucent video stickers, consider converting the GIF to *WebM* format with online tools and resizing the video clip using this bot\.
Refer to: [GIF Revolution](https://telegram.org/blog/gif-revolution)

Maintainer: @ferricion
Github Repository: [telegram\-sticker\-bot](https://github.com/FerricIon/telegram-sticker-bot)"#
        }
    };

    bot.send_message(m.chat_id(), text)
        .disable_web_page_preview(true)
        .parse_mode(teloxide::types::ParseMode::MarkdownV2)
        .await?;

    Ok(())
}

pub async fn message_handler(m: Message, bot: AutoSend<Bot>) -> Result<(), RequestError> {
    match convert_message(&m, &bot, None, None).await {
        Ok((document, layout, playback)) => {
            bot.send_document(m.chat_id(), document)
                .caption(make_caption(layout, playback))
                .reply_to_message_id(m.id)
                .reply_markup(make_layout_keyboard(layout))
                .await?;
        }
        Err(e) => {
            log::error!("{}", e);
            let mut res = bot
                .send_message(m.chat_id(), e.to_string())
                .reply_to_message_id(m.id);
            if let ConvertError::Duration(_) = e {
                let keyboard = vec![vec![Callback::SpeedUp.into()]];
                res = res.reply_markup(InlineKeyboardMarkup::new(keyboard));
            }
            res.await?;
        }
    };

    Ok(())
}

pub async fn speed_up_handler(q: CallbackQuery, bot: AutoSend<Bot>) -> Result<True, RequestError> {
    let r = (|| async {
        let m = q.message.ok_or(PropsError::Message)?;
        let m_origin = m.reply_to_message().ok_or(PropsError::Origin)?.to_owned();
        let (document, layout, playback) =
            convert_message(&m_origin, &bot, None, Some(PlaybackProp { speed_up: true })).await?;
        anyhow::Ok((m, m_origin, document, layout, playback))
    })()
    .await;

    match r {
        Ok((m, m_origin, document, layout, playback)) => {
            bot.delete_message(m.chat_id(), m.id).await?;
            bot.send_document(m.chat_id(), document)
                .caption(make_caption(layout, playback))
                .reply_to_message_id(m_origin.id)
                .reply_markup(make_layout_keyboard(layout))
                .await?;
            bot.answer_callback_query(q.id).await
        }
        Err(e) => {
            log::error!("{}", e);
            bot.answer_callback_query(q.id).text(e.to_string()).await
        }
    }
}

pub async fn layout_handler(q: CallbackQuery, bot: AutoSend<Bot>) -> Result<True, RequestError> {
    let r = (|| async {
        let m = q.message.ok_or(PropsError::Message)?;
        let m_origin = m.reply_to_message().ok_or(PropsError::Origin)?.to_owned();
        let (layout, playback) = get_props(&m);
        let layout = layout.ok_or(PropsError::Message)?;
        let callback: Callback = q.data.unwrap_or_default().parse()?;
        let layout = match callback.kind() {
            CallbackKind::Size => layout.reset_size(callback),
            CallbackKind::Position => layout.reset_alignment(callback),
            _ => Err(CallbackError::Incompatible),
        }?;

        let (document, layout, playback) =
            convert_message(&m_origin, &bot, Some(layout), playback).await?;
        anyhow::Result::<_>::Ok((m, document, layout, playback))
    })()
    .await;

    match r {
        Ok((m, document, layout, playback)) => {
            bot.edit_message_media(
                m.chat_id(),
                m.id,
                InputMedia::Document(InputMediaDocument::new(document)),
            )
            .await?;
            bot.edit_message_caption(m.chat_id(), m.id)
                .caption(make_caption(layout, playback))
                .await?;
            bot.edit_message_reply_markup(m.chat_id(), m.id)
                .reply_markup(make_layout_keyboard(layout))
                .await?;
            bot.answer_callback_query(q.id).await
        }
        Err(e) => {
            log::error!("{}", e);
            bot.answer_callback_query(q.id).text(e.to_string()).await
        }
    }
}

pub async fn callback_handler(q: CallbackQuery, bot: AutoSend<Bot>) -> Result<(), RequestError> {
    match q.data.to_owned().unwrap_or_default().parse::<Callback>() {
        Ok(callback) => match callback.kind() {
            CallbackKind::Size | CallbackKind::Position => layout_handler(q, bot).await,
            CallbackKind::Time => speed_up_handler(q, bot).await,
        },
        Err(e) => bot.answer_callback_query(q.id).text(e.to_string()).await,
    }
    .map(|_| ())
}
