import json
import sys
from datetime import date, timedelta
from pathlib import Path

import lark_oapi as lark
from flask import Flask, request as flask_request

from api_client import LocalApiClient

app = Flask(__name__)

cli: lark.Client = None
api: LocalApiClient = None


def load_config():
    config_path = Path(__file__).parent / "config.json"
    if not config_path.exists():
        print("请复制 config.example.json 为 config.json 并填写配置")
        sys.exit(1)
    with open(config_path) as f:
        return json.load(f)


def resolve_date(text: str) -> str:
    text = text.strip().lower()
    if text in ("today", "今天"):
        return date.today().isoformat()
    if text in ("yesterday", "昨天"):
        return (date.today() - timedelta(days=1)).isoformat()
    return text


def reply_text(message_id: str, text: str):
    from lark_oapi.api.im.v1 import ReplyMessageRequest, ReplyMessageRequestBody

    req = ReplyMessageRequest.builder().message_id(message_id).request_body(
        ReplyMessageRequestBody.builder().content_type("text").content(
            json.dumps({"text": text})
        ).build()
    ).build()
    cli.im.v1.message.reply(req)


def handle_list_reports(message_id: str):
    try:
        data = api.list_reports()
    except Exception as e:
        reply_text(message_id, f"请求失败: {e}")
        return
    dates = data.get("dates", [])
    if not dates:
        reply_text(message_id, "暂无日报")
        return
    lines = [f"📋 最近 {len(dates)} 天有日报："]
    for i, d in enumerate(dates, 1):
        lines.append(f"  {i}. {d}")
    reply_text(message_id, "\n".join(lines))


def handle_report(message_id: str, date_str: str):
    date_str = resolve_date(date_str)
    try:
        data = api.get_report(date_str)
    except Exception as e:
        reply_text(message_id, f"请求失败: {e}")
        return
    if "error" in data:
        reply_text(message_id, f"❌ {data['error']}")
        return
    content = data.get("content", "")
    if not content:
        reply_text(message_id, f"{date_str} 日报内容为空")
        return
    reply_text(message_id, content[:4000])


def handle_generate(message_id: str, date_str: str):
    date_str = resolve_date(date_str)
    reply_text(message_id, f"正在生成 {date_str} 的日报...")
    try:
        data = api.generate_report(date_str)
    except Exception as e:
        reply_text(message_id, f"请求失败: {e}")
        return
    if "error" in data:
        reply_text(message_id, f"❌ {data['error']}")
        return
    content = data.get("content", "")
    if not content:
        reply_text(message_id, "生成完成但内容为空")
        return
    reply_text(message_id, f"✅ {date_str} 日报已生成\n\n{content[:3900]}")


def handle_device(message_id: str):
    try:
        data = api.device()
    except Exception as e:
        reply_text(message_id, f"请求失败: {e}")
        return
    text = (
        f"🖥 设备信息\n"
        f"  ID: {data.get('deviceId', '-')}\n"
        f"  名称: {data.get('deviceName', '-')}\n"
        f"  平台: {data.get('platform', '-')}\n"
        f"  版本: {data.get('appVersion', '-')}\n"
        f"  录制中: {'是' if data.get('recording') else '否'}\n"
    )
    reply_text(message_id, text)


def on_message(ctx: lark.Context, event: lark.im.v1.P2ImMessageReceiveV1) -> None:
    msg = event.event.message
    msg_type = msg.message_type
    if msg_type != "text":
        return

    content = json.loads(msg.content) if isinstance(msg.content, str) else msg.content
    text = content.get("text", "").strip()

    parts = text.split()
    if not parts:
        return

    cmd = parts[0].lower()
    arg = parts[1] if len(parts) > 1 else None

    if cmd in ("reports", "日报列表", "列表"):
        handle_list_reports(msg.message_id)
    elif cmd in ("report", "日报", "查看"):
        handle_report(msg.message_id, arg or "today")
    elif cmd in ("generate", "生成", "生成日报"):
        handle_generate(msg.message_id, arg or "today")
    elif cmd in ("device", "设备", "设备信息"):
        handle_device(msg.message_id)
    elif cmd in ("help", "帮助", "?", "？"):
        reply_text(
            msg.message_id,
            "📊 Work Review Bot\n\n"
            "日报列表 - 查看最近日报\n"
            "日报 [日期] - 查看日报（默认今天）\n"
            "生成日报 [日期] - 生成日报\n"
            "设备信息 - 查看设备状态\n\n"
            "日期格式：2026-04-25 或 今天/昨天",
        )


def main():
    global cli, api

    config = load_config()
    la = config["local_api"]
    fs = config.get("feishu", {})

    api = LocalApiClient(la["base_url"], la["token"])

    app_id = fs.get("app_id", "")
    app_secret = fs.get("app_secret", "")
    ver_token = fs.get("verification_token", "")
    enc_key = fs.get("encrypt_key", "")

    if not app_id or "替换" in app_id:
        print("请在 config.json 中填写 feishu.app_id 和 feishu.app_secret")
        sys.exit(1)

    try:
        health = api.health()
        print(f"Local API: {health.get('status', 'unknown')}")
    except Exception as e:
        print(f"⚠ Local API 未就绪: {e}")

    cli = lark.Client.builder().app_id(app_id).app_secret(app_secret).log_level(lark.LogLevel.INFO).build()

    event_handler = (
        lark.EventDispatcherHandler.builder("", "")
        .register_p2_im_message_receive_v1(on_message)
        .build()
    )

    @app.route("/feishu/event", methods=["POST"])
    def feishu_event():
        resp = lark.parse.request(
            flask_request.headers,
            flask_request.data,
            event_handler,
            cli,
            ver_token,
            enc_key,
        )
        return resp

    print(f"Feishu Bot 已启动 (端口 9090)，webhook 地址: http://你的域名/feishu/event")
    print("需要内网穿透（ngrok/cloudflare tunnel）将此地址暴露到公网")
    app.run(host="0.0.0.0", port=9090)


if __name__ == "__main__":
    main()
