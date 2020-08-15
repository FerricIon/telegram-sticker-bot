import got from 'got'
import Telegraf from 'telegraf'
import sharp from 'sharp'
import path from 'path'

const MAX_FILE_SIZE = 2 * 1024 * 1024
const bot = new Telegraf(process.env.TELEGRAM_BOT_TOKEN)

bot.start((ctx) => {
  ctx.reply('Welcome')
})

bot.on('document', async (ctx) => {
  try {
    if (!ctx.message.document.mime_type.startsWith('image/')) {
      throw 'unknown file type'
    }
    if (ctx.message.document.file_size > MAX_FILE_SIZE) {
      throw 'file too large'
    }
    const file_link = await ctx.telegram.getFileLink(
      ctx.message.document.file_id,
    )
    const response = await got(file_link)
    const image = sharp(response.rawBody)
    const { width, height } = await image.metadata()
    if (width > 384 || height > 256) image.resize(512, 512, { fit: 'inside' })
    else
      image.resize(512, height > 128 ? 256 : 128, {
        fit: 'contain',
        background: 'rgba(0,0,0,0)',
      })
    ctx.replyWithDocument({
      source: await image.png().toBuffer(),
      filename: path.parse(ctx.message.document.file_name).name + '.png',
    })
  } catch (err) {
    ctx.reply('Sorry but we cannot convert the file at this time: ' + err)
  }
})

bot.launch()
