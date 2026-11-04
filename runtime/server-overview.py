"""Read-only server overview. Remote Python runs via SSH stdin and leaves no script."""
import subprocess
import sys
remote = r"""
import json,subprocess
def call(args):
 try:
  result=subprocess.run(args,capture_output=True,text=True,timeout=12)
  return result.stdout if result.returncode==0 else None
 except (OSError,subprocess.TimeoutExpired): return None
services=[
 ('hindsight-api','共享记忆 API','docker','hindsight-api'),
 ('hindsight-db','记忆数据库','docker','hindsight-db'),
 ('hindsight-ingestion','资料同步与入库','docker','hindsight-ingestion'),
 ('hindsight-control-plane','记忆管理界面','docker','hindsight-control-plane'),
 ('vaultwarden','密码库','docker','vaultwarden'),
 ('radicale','个人日历','docker','radicale'),
 ('uptime-kuma','站点与容器监控','docker','uptime-kuma'),
 ('n8n','自动化工作流','docker','n8n'),
 ('astrbot','消息机器人','docker','astrbot'),
 ('opengym-web-1','健身应用前端','docker','opengym-web-1'),
 ('opengym-api-1','健身应用接口','docker','opengym-api-1'),
 ('cloudflared','云端访问隧道','systemd','cloudflared-restored.service'),
 ('hermes-agent','Hermes 智能助手','docker-prefix','1Panel-hermes-agent-'),
 ('timetable','课程看板','systemd','timetable.service'),
 ('watchpending','消息补发与提醒','systemd','watchpending.service'),
 ('sentinel','监控告警与自愈','systemd','sentinel.service'),
 ('health-check','网站容器健康检查','timer','health-check.timer'),
]
raw=call(['docker','ps','-a','--no-trunc','--format','{{.Names}}\t{{.State}}\t{{.Status}}'])
containers=None if raw is None else [line.split('\t',2) for line in raw.splitlines() if len(line.split('\t',2))==3]
rows=[]
for name,description,kind,ident in services:
 state='unknown'; label='未知'
 if kind.startswith('docker') and containers is not None:
  matches=[c for c in containers if c[0]==ident or (kind=='docker-prefix' and c[0].startswith(ident))]
  if len(matches)==1:
   _,status,detail=matches[0]
   if status=='running' and '(unhealthy)' not in detail and '(health: starting)' not in detail:
    state,label='ok','运行中'
   elif '(unhealthy)' in detail or status in ('exited','dead','restarting'):
    state,label='down','异常'
   else: state,label='unknown','启动中' if 'starting' in detail else '未运行'
  elif not matches: state,label='inactive','未启动'
 elif kind in ('systemd','timer'):
  raw_unit=call(['systemctl','show',ident,'--property=LoadState,ActiveState,SubState,UnitFileState','--no-pager'])
  if raw_unit is not None:
   props=dict(line.split('=',1) for line in raw_unit.splitlines() if '=' in line)
   load=props.get('LoadState'); active=props.get('ActiveState'); enabled=props.get('UnitFileState')
   if load=='not-found': state,label='inactive','未配置'
   elif load!='loaded': state,label='unknown','未知'
   elif active=='active': state,label='ok',('已调度' if kind=='timer' else '运行中')
   elif active=='failed': state,label='down','异常'
   elif active=='inactive' and enabled in ('disabled','masked','static',''): state,label='inactive','未启用'
   elif active=='inactive': state,label='down','已停止'
 rows.append({'name':name,'description':description,'state':state,'label':label})
print(json.dumps({'services':rows},ensure_ascii=False))
"""
from pathlib import Path
connection=Path.home()/'AppData/Roaming/Daozai/server-connection.json'
import json
cmd=json.loads(connection.read_text(encoding='utf-8'))['ssh_command']
r=subprocess.run(cmd,input=remote.encode('utf-8'),capture_output=True,timeout=45,creationflags=0x08000000 if sys.platform=='win32' else 0)
if r.returncode:
 print('服务器总览暂时无法连接'); sys.exit(1)
sys.stdout.reconfigure(encoding='utf-8'); print(r.stdout.decode('utf-8')[:50000])
