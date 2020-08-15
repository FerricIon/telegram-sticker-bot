import got from 'got'
import Telegraf from 'telegraf'
import sharp from 'sharp'
import path from 'path'
import moment from 'moment'
import LocalSession from 'telegraf-session-local'
import TT from 'telegraf/typings/telegram-types'
import { TelegrafContext } from 'telegraf/typings/context'

const MAX_FILE_SIZE = 2 * 1024 * 1024
const bot = new Telegraf(process.env.TELEGRAM_BOT_TOKEN)
const local_session = new LocalSession<ConvertSettings>()

class ConvertSettings {
  sticker_placement?: 'center' | 'left'
  unconverted?: TT.Document & { date: Date }
}

async function convertImage(
  ctx: TelegrafContext & { session: ConvertSettings },
  document: TT.Document & { date: Date },
) {
  if (!document.mime_type.startsWith('image/')) {
    throw 'unknown file type'
  }
  if (document.file_size > MAX_FILE_SIZE) {
    throw 'file too large'
  }
  if (moment(document.date).isBefore(moment().subtract(1, 'minute'))) {
    throw 'file expired'
  }
  const file_link = await ctx.telegram.getFileLink(document.file_id)
  const response = await got(file_link)
  const image = sharp(response.rawBody)
  const { width, height } = await image.metadata()
  if (width > 384 || height > 256) image.resize(512, 512, { fit: 'inside' })
  else {
    if (!ctx.session.sticker_placement) throw 'no placement'

    image.resize(512, height > 128 ? 256 : 128, {
      position: ctx.session.sticker_placement,
      fit: 'contain',
      background: 'rgba(0,0,0,0)',
    })
  }
  return await image.png().toBuffer()
}

bot.use(local_session.middleware())

bot.on(
  'document',
  async (ctx: TelegrafContext & { session: ConvertSettings }) => {
    try {
      if (ctx.message.document.mime_type.startsWith('image/')) {
        const converted = await convertImage(ctx, {
          ...ctx.message.document,
          date: moment().toDate(),
        })
        ctx.replyWithDocument({
          source: converted,
          filename: path.parse(ctx.message.document.file_name).name + '.png',
        })
      }
    } catch (err) {
      if (err == 'no placement') {
        ctx.session.unconverted = {
          ...ctx.message.document,
          date: moment().toDate(),
        }
        ctx.replyWithMarkdown(
          "This seems to be a small sticker so I'd rather keep it small by letterboxing, " +
            'but please tell me how to place the sticker with `/setplacement centre | left`',
        )
      } else
        ctx.reply('Sorry but we cannot convert the file at this time: ' + err)
    }
  },
)

bot.command(
  'setplacement',
  async (ctx: TelegrafContext & { session: ConvertSettings }) => {
    const placement = ctx.message.text.split(' ')
    if (placement.length < 2 || !['centre', 'left'].includes(placement[1])) {
      ctx.replyWithMarkdown(
        "Sorry but I don't know how to place the sticker. Please tell me `/setplacement centre | left`",
      )
    } else {
      ctx.session.sticker_placement = placement[1] as 'center' | 'left'
      ctx.replyWithMarkdown(
        `Set sticker placement to ${placement[1]}. ` +
          'Remember you can always change this by telling me `/setplacement centre | left`',
      )
    }
    if (ctx.session.unconverted) {
      try {
        const converted = await convertImage(ctx, ctx.session.unconverted)
        ctx.replyWithDocument({
          source: converted,
          filename: path.parse(ctx.session.unconverted.file_name).name + '.png',
        })
        delete ctx.session.unconverted
      } catch (err) {
        // pass
      }
    }
  },
)

bot.launch()
