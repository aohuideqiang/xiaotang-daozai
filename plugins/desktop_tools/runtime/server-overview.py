"""只读服务器总览：远端代码通过 SSH 标准输入执行，不在服务器留下脚本。

用法：`python server-overview.py [用户数据目录]`；目录缺省为 `%APPDATA%/Daozai`。
输出一行 JSON，字段与插件里的「服务器任务」表格一致。
"""
import json
import subprocess
import sys
from pathlib import Path

# 服务清单；改这里时远端和本地 CATALOG 一起改，连不上服务器也能列出全部条目。
CATALOG = [
    ("hindsight-api", "共享记忆 API"),
    ("hindsight-db", "记忆数据库"),
    ("hindsight-ingestion", "资料同步与入库"),
    ("hindsight-control-plane", "记忆管理界面"),
    ("vaultwarden", "密码库"),
    ("radicale", "个人日历"),
    ("uptime-kuma", "站点与容器监控"),
    ("n8n", "自动化工作流"),
    ("astrbot", "消息机器人"),
    ("opengym-web-1", "健身应用前端"),
    ("opengym-api-1", "健身应用接口"),
    ("cloudflared", "云端访问隧道"),
    ("hermes-agent", "Hermes 智能助手"),
    ("timetable", "课程看板"),
    ("watchpending", "消息补发与提醒"),
    ("sentinel", "监控告警与自愈"),
    ("health-check", "网站容器健康检查"),
]

SERVICES = [
    ("hindsight-api", "共享记忆 API", "docker", "hindsight-api"),
    ("hindsight-db", "记忆数据库", "docker", "hindsight-db"),
    ("hindsight-ingestion", "资料同步与入库", "docker", "hindsight-ingestion"),
    ("hindsight-control-plane", "记忆管理界面", "docker", "hindsight-control-plane"),
    ("vaultwarden", "密码库", "docker", "vaultwarden"),
    ("radicale", "个人日历", "docker", "radicale"),
    ("uptime-kuma", "站点与容器监控", "docker", "uptime-kuma"),
    ("n8n", "自动化工作流", "docker", "n8n"),
    ("astrbot", "消息机器人", "docker", "astrbot"),
    ("opengym-web-1", "健身应用前端", "docker", "opengym-web-1"),
    ("opengym-api-1", "健身应用接口", "docker", "opengym-api-1"),
    ("cloudflared", "云端访问隧道", "systemd", "cloudflared-restored.service"),
    ("hermes-agent", "Hermes 智能助手", "docker-prefix", "1Panel-hermes-agent-"),
    ("timetable", "课程看板", "systemd", "timetable.service"),
    ("watchpending", "消息补发与提醒", "systemd", "watchpending.service"),
    ("sentinel", "监控告警与自愈", "systemd", "sentinel.service"),
    ("health-check", "网站容器健康检查", "systemd", "health-check.service"),
]

# 远端执行体：只用 docker / systemctl 的只读查询，不启动或修改任何服务。
REMOTE = r'''
import json
import subprocess


def call(args):
    try:
        result = subprocess.run(args, capture_output=True, text=True, timeout=12)
        return result.stdout if result.returncode == 0 else None
    except (OSError, subprocess.TimeoutExpired):
        return None


SERVICES = [
    ("hindsight-api", "共享记忆 API", "docker", "hindsight-api"),
    ("hindsight-db", "记忆数据库", "docker", "hindsight-db"),
    ("hindsight-ingestion", "资料同步与入库", "docker", "hindsight-ingestion"),
    ("hindsight-control-plane", "记忆管理界面", "docker", "hindsight-control-plane"),
    ("vaultwarden", "密码库", "docker", "vaultwarden"),
    ("radicale", "个人日历", "docker", "radicale"),
    ("uptime-kuma", "站点与容器监控", "docker", "uptime-kuma"),
    ("n8n", "自动化工作流", "docker", "n8n"),
    ("astrbot", "消息机器人", "docker", "astrbot"),
    ("opengym-web-1", "健身应用前端", "docker", "opengym-web-1"),
    ("opengym-api-1", "健身应用接口", "docker", "opengym-api-1"),
    ("cloudflared", "云端访问隧道", "systemd", "cloudflared-restored.service"),
    ("hermes-agent", "Hermes 智能助手", "docker-prefix", "1Panel-hermes-agent-"),
    ("timetable", "课程看板", "systemd", "timetable.service"),
    ("watchpending", "消息补发与提醒", "systemd", "watchpending.service"),
    ("sentinel", "监控告警与自愈", "systemd", "sentinel.service"),
    ("health-check", "网站容器健康检查", "systemd", "health-check.service"),
]


def containers():
    raw = call(["docker", "ps", "-a", "--no-trunc", "--format", "{{.Names}}\t{{.State}}\t{{.Status}}"])
    if raw is None:
        return None
    return [line.split("\t", 2) for line in raw.splitlines() if len(line.split("\t", 2)) == 3]


def unit(ident):
    raw = call(["systemctl", "show", ident, "--property=LoadState,ActiveState,SubState,UnitFileState", "--no-pager"])
    if raw is None:
        return None
    return dict(line.split("=", 1) for line in raw.splitlines() if "=" in line)


rows = []
running = containers()
for name, description, kind, ident in SERVICES:
    state, label = "unknown", "未知"
    if kind.startswith("docker") and running is not None:
        matches = [item for item in running if item[0] == ident or (kind == "docker-prefix" and item[0].startswith(ident))]
        if len(matches) == 1:
            status, detail = matches[0][1], matches[0][2]
            if status == "running" and "(unhealthy)" not in detail and "(health: starting)" not in detail:
                state, label = "ok", "运行中"
            elif "(unhealthy)" in detail or status in ("exited", "dead", "restarting"):
                state, label = "down", "异常"
            else:
                state, label = "unknown", "启动中" if "starting" in detail else "未运行"
        elif not matches:
            state, label = "inactive", "未启动"
    elif kind in ("systemd", "timer"):
        props = unit(ident)
        if props is not None:
            load = props.get("LoadState")
            active = props.get("ActiveState")
            enabled = props.get("UnitFileState")
            if load == "not-found":
                state, label = "inactive", "未配置"
            elif load != "loaded":
                state, label = "unknown", "未知"
            elif active == "active":
                state, label = "ok", "已调度" if kind == "timer" else "运行中"
            elif active == "failed":
                state, label = "down", "异常"
            elif active == "inactive" and enabled in ("disabled", "masked", "static", ""):
                state, label = "inactive", "未启用"
            elif active == "inactive":
                state, label = "down", "已停止"
    rows.append({"name": name, "description": description, "state": state, "label": label})
print(json.dumps({"connected": True, "services": rows}, ensure_ascii=False))
'''


def offline(reason):
    rows = [{"name": name, "description": description, "state": "unknown", "label": "连接失败"} for name, description in CATALOG]
    print(json.dumps({"connected": False, "reason": reason, "services": rows}, ensure_ascii=False))


def main():
    data_dir = Path(sys.argv[1]) if len(sys.argv) > 1 else Path.home() / "AppData" / "Roaming" / "Daozai"
    connection = data_dir / "server-connection.json"
    if not connection.exists():
        offline("尚未配置服务器连接")
        return
    try:
        command = json.loads(connection.read_text(encoding="utf-8"))["ssh_command"]
    except Exception:
        offline("服务器连接配置无效")
        return
    if not command or not isinstance(command, list):
        offline("服务器连接命令为空")
        return
    creation_flags = 0x08000000 if sys.platform == "win32" else 0
    try:
        result = subprocess.run(command, input=REMOTE.encode("utf-8"), capture_output=True,
                                timeout=45, creationflags=creation_flags)
    except Exception:
        offline("服务器连接超时")
        return
    if result.returncode != 0:
        offline("服务器暂时无法连接")
        return
    try:
        data = json.loads(result.stdout.decode("utf-8")[:50000])
    except Exception:
        offline("服务器返回无法解析")
        return
    data.setdefault("connected", True)
    print(json.dumps(data, ensure_ascii=False))


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    main()
