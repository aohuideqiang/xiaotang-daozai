"""只读 Edge 下载记录：不查询浏览地址，也不读取账号和密码。"""
import json
import os
import sqlite3
from pathlib import Path

root = Path(os.environ.get("LOCALAPPDATA", "")) / "Microsoft" / "Edge" / "User Data"
rows, errors = [], []
profiles = sorted(root.glob("*/History"))[:20]
for history in profiles:
    try:
        # 以只读方式打开；Edge 正在运行也能读，超时很短，失败就让界面稍后重试。
        with sqlite3.connect(history.as_uri() + "?mode=ro", uri=True, timeout=0.3) as db:
            db.execute("PRAGMA query_only=ON")
            for current, target, received, total in db.execute(
                "SELECT current_path,target_path,received_bytes,total_bytes "
                "FROM downloads WHERE state=0 ORDER BY start_time DESC LIMIT 50"
            ):
                # Chromium 会把进度写进元数据；半成品文件本身能给出更及时的字节数。
                size = max(0, received)
                try:
                    if current:
                        size = max(size, Path(current).stat().st_size)
                except OSError:
                    pass
                rows.append({"path": target or current, "bytes": size, "total": max(0, total)})
    except (OSError, sqlite3.Error):
        errors.append(history.parent.name)
print(json.dumps({"downloads": rows[:50], "error": bool(errors)}, ensure_ascii=True))
