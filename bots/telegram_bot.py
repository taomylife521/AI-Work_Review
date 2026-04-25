import json
import sys
from datetime import date, timedelta
from pathlib import Path

from telegram import Update
from telegram.ext import Application, CommandHandler, ContextTypes

from api_client import LocalApiClient

HELP_TEXT = (
    "📊 Work Review Bot\n\n"
    "/reports - 最近日报列表\n"
    "/report [日期] - 查看日报（默认今天）\n"
    "/generate [日期] - 生成日报（默认今天）\n"
    "/device - 设备信息\n"
    "/health - API 健康状态\n\n"
    "日期格式：2026-04-25 或 today/yesterday"
)


def load_config():
    config_path = Path(__file__).parent / "config.json"
    if not config_path.exists():
        print("请复制 config.example.json 为 config.json 并填写配置")
        sys.exit(1)
    with open(config_path) as f:
        return json.load(f)


def resolve_date(text: str | None) -> str:
    if not text:
        return date.today().isoformat()
    text = text.strip().lower()
    if text in ("today", "今天"):
        return date.today().isoformat()
    if text in ("yesterday", "昨天"):
        return (date.today() - timedelta(days=1)).isoformat()
    return text


def make_api(config: dict) -> LocalApiClient:
    la = config["local_api"]
    return LocalApiClient(la["base_url"], la["token"])


async def cmd_help(update: Update, ctx: ContextTypes.DEFAULT_TYPE):
    await update.message.reply_text(HELP_TEXT)


async def cmd_reports(update: Update, ctx: ContextTypes.DEFAULT_TYPE):
    api: LocalApiClient = ctx.bot_data["api"]
    try:
        data = api.list_reports()
    except Exception as e:
        await update.message.reply_text(f"请求失败: {e}")
        return
    dates = data.get("dates", [])
    if not dates:
        await update.message.reply_text("暂无日报")
        return
    lines = [f"📋 最近 {len(dates)} 天有日报："]
    for i, d in enumerate(dates, 1):
        lines.append(f"  {i}. {d}")
    lines.append("\n/report <日期> 查看详情")
    await update.message.reply_text("\n".join(lines))


async def cmd_report(update: Update, ctx: ContextTypes.DEFAULT_TYPE):
    api: LocalApiClient = ctx.bot_data["api"]
    date_str = resolve_date(ctx.args[0] if ctx.args else None)
    try:
        data = api.get_report(date_str)
    except Exception as e:
        await update.message.reply_text(f"请求失败: {e}")
        return
    if "error" in data:
        await update.message.reply_text(f"❌ {data['error']}")
        return
    content = data.get("content", "")
    if not content:
        await update.message.reply_text(f"📄 {date_str} 日报内容为空")
        return
    header = f"📄 {date_str}"
    if data.get("ai_mode"):
        header += f" ({data['ai_mode']})"
    text = f"{header}\n\n{content}"
    if len(text) > 4000:
        text = text[:3990] + "\n...(截断)"
    await update.message.reply_text(text, parse_mode="Markdown")


async def cmd_generate(update: Update, ctx: ContextTypes.DEFAULT_TYPE):
    api: LocalApiClient = ctx.bot_data["api"]
    date_str = resolve_date(ctx.args[0] if ctx.args else None)
    await update.message.reply_text(f"⏳ 正在生成 {date_str} 的日报...")
    try:
        data = api.generate_report(date_str)
    except Exception as e:
        await update.message.reply_text(f"请求失败: {e}")
        return
    if "error" in data:
        await update.message.reply_text(f"❌ {data['error']}")
        return
    content = data.get("content", "")
    if not content:
        await update.message.reply_text("生成完成但内容为空")
        return
    text = f"✅ {date_str} 日报已生成\n\n{content}"
    if len(text) > 4000:
        text = text[:3990] + "\n...(截断)"
    await update.message.reply_text(text, parse_mode="Markdown")


async def cmd_device(update: Update, ctx: ContextTypes.DEFAULT_TYPE):
    api: LocalApiClient = ctx.bot_data["api"]
    try:
        data = api.device()
    except Exception as e:
        await update.message.reply_text(f"请求失败: {e}")
        return
    lines = [
        f"🖥 设备信息",
        f"  ID: {data.get('deviceId', '-')}",
        f"  名称: {data.get('deviceName', '-')}",
        f"  平台: {data.get('platform', '-')}",
        f"  版本: {data.get('appVersion', '-')}",
        f"  录制中: {'是' if data.get('recording') else '否'}",
        f"  已暂停: {'是' if data.get('paused') else '否'}",
    ]
    await update.message.reply_text("\n".join(lines))


async def cmd_health(update: Update, ctx: ContextTypes.DEFAULT_TYPE):
    api: LocalApiClient = ctx.bot_data["api"]
    try:
        data = api.health()
    except Exception as e:
        await update.message.reply_text(f"❌ 连接失败: {e}")
        return
    status = "✅ 正常" if data.get("status") == "ok" else "⚠️ 异常"
    text = f"{status}\n版本: {data.get('version', '-')}"
    await update.message.reply_text(text)


def main():
    config = load_config()
    tg = config.get("telegram", {})
    bot_token = tg.get("bot_token", "")
    if not bot_token or "替换" in bot_token:
        print("请在 config.json 中填写 telegram.bot_token")
        sys.exit(1)

    api = make_api(config)
    try:
        health = api.health()
        print(f"Local API: {health.get('status', 'unknown')}")
    except Exception as e:
        print(f"⚠ Local API 未就绪: {e}")
        print("请确保 Work Review 已启动并开启本地 API")

    app = Application.builder().token(bot_token).build()
    app.bot_data["api"] = api

    app.add_handler(CommandHandler("start", cmd_help))
    app.add_handler(CommandHandler("help", cmd_help))
    app.add_handler(CommandHandler("reports", cmd_reports))
    app.add_handler(CommandHandler("report", cmd_report))
    app.add_handler(CommandHandler("generate", cmd_generate))
    app.add_handler(CommandHandler("device", cmd_device))
    app.add_handler(CommandHandler("health", cmd_health))

    print("Telegram Bot 已启动，按 Ctrl+C 停止")
    app.run_polling(drop_pending_updates=True)


if __name__ == "__main__":
    main()
