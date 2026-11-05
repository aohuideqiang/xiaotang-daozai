"""Read only Edge download metadata; never query browsing URLs or credentials."""
import json
import os
import sqlite3
from pathlib import Path

root = Path(os.environ.get('LOCALAPPDATA', '')) / 'Microsoft/Edge/User Data'
rows, errors = [], []
profiles = sorted(root.glob('*/History'))[:20]
for history in profiles:
    try:
        with sqlite3.connect(history.as_uri() + '?mode=ro', uri=True, timeout=0.3) as db:
            db.execute('PRAGMA query_only=ON')
            for current, target, received, total in db.execute(
                'SELECT current_path,target_path,received_bytes,total_bytes '
                'FROM downloads WHERE state=0 ORDER BY start_time DESC LIMIT 50'
            ):
                # Chromium checkpoints metadata; the partial file gives live bytes.
                size = max(0, received)
                try:
                    size = max(size, Path(current).stat().st_size) if current else size
                except OSError:
                    pass
                rows.append({'path': target or current, 'bytes': size, 'total': max(0, total)})
    except (OSError, sqlite3.Error):
        errors.append(history.parent.name)
print(json.dumps({'downloads': rows[:50], 'error': bool(errors)}, ensure_ascii=True))
