//! Lightweight desktop tools. Clipboard contents remain in memory; all caches are bounded.
use eframe::egui::{self,Color32};
use chrono::{Datelike,NaiveDate};
use std::{sync::{Arc,Mutex,mpsc},time::{Duration,Instant},io::Read};
use serde_json::{Value,json};
#[derive(Default,Clone)]
struct Snapshot { clips:Vec<String>, cpu:f64, memory:f64, tasks:String, updated:String, edge:Vec<Download>, edge_error:String }
#[derive(Clone)]
struct Config { enabled:[bool;5], shortcuts:Vec<(String,String)>, bridge:String, courses:String }
impl Default for Config {fn default()->Self {Self{enabled:[true;5],shortcuts:vec![],bridge:String::new(),courses:String::new()}}}
fn config_path()->std::path::PathBuf{crate::model::config_dir().join("desktop-tools.json")}
fn single_line(value:&str)->String{value.split_whitespace().collect::<Vec<_>>().join(" ")}
fn download_progress(d:&Download)->String{
 if d.total>0{format!("{} {:.0}% · {:.1}/{:.1} MB",d.status,(d.bytes as f64/d.total as f64*100.0).min(100.0),d.bytes as f64/1048576.0,d.total as f64/1048576.0)}
 else{format!("{} · 已下载 {:.1} MB",d.status,d.bytes as f64/1048576.0)}
}
fn edge_downloads()->(Vec<Download>,String){
 let mut cmd=std::process::Command::new("python");cmd.args(["-I","-c",include_str!("../runtime/edge-downloads.py")]);
 #[cfg(windows)]{use std::os::windows::process::CommandExt;cmd.creation_flags(0x08000000);}
 let data=cmd.output().ok().filter(|o|o.status.success()).and_then(|o|serde_json::from_slice::<Value>(&o.stdout).ok());
 match data{Some(v)=>{
  let rows=v["downloads"].as_array().map(|rows|rows.iter().take(50).filter_map(|r|Some(Download{path:r["path"].as_str()?.into(),bytes:r["bytes"].as_u64()?,total:r["total"].as_u64()?,status:"Edge".into(),..Default::default()})).collect()).unwrap_or_default();
  (rows,if v["error"].as_bool()==Some(true){"部分 Edge 下载暂时无法读取，将自动重试".into()}else{String::new()})
 },None=>(vec![],"无法读取 Edge 下载，请确认本机 Python 可用；稍后自动重试".into())}
}
impl Config {
 fn load()->Self {let mut c=Self::default();if let Ok(raw)=std::fs::read(config_path()){if let Ok(v)=serde_json::from_slice::<Value>(&raw){
 if let Some(a)=v["enabled"].as_array(){for(i,b)in a.iter().take(5).enumerate(){c.enabled[i]=b.as_bool().unwrap_or(true);}}
 c.bridge=v["bridge"].as_str().unwrap_or_default().into();c.courses=v["courses"].as_str().unwrap_or_default().into();
 if let Some(rows)=v["shortcuts"].as_array(){c.shortcuts=rows.iter().take(50).filter_map(|r|Some((r[0].as_str()?.into(),r[1].as_str()?.into()))).collect();}
 }}c}
 fn save(&self)->Result<(),String>{let raw=serde_json::to_vec(&json!({"enabled":self.enabled,"shortcuts":self.shortcuts,"bridge":self.bridge,"courses":self.courses})).unwrap();std::fs::create_dir_all(crate::model::config_dir()).map_err(|e|e.to_string())?;crate::model::atomic_write(&config_path(),&raw).map_err(|e|e.to_string())}
}
#[derive(Clone,Default)]
struct Download {url:String, path:String, bytes:u64,total:u64,status:String}
pub(crate) struct Tools {
 pub open:bool,pub tab:usize, pub mini:bool, pub(crate) week:bool, offset:i64, config:Config, shared:Arc<Mutex<Snapshot>>, tx:Option<mpsc::Sender<Config>>,
 downloads:Vec<Arc<Mutex<Download>>>,url:String,label:String,target:String,filter:String,note:String,courses:Value,server_offset:usize,
}
impl Default for Tools {fn default()->Self{Self{open:false,tab:0,mini:false,week:false,offset:0,config:Config::load(),shared:Default::default(),tx:None,downloads:vec![],url:String::new(),label:String::new(),target:String::new(),filter:String::new(),note:String::new(),courses:Value::Null,server_offset:0}}}
fn bounded_clip(clips:&mut Vec<String>,text:String){if text.trim().is_empty(){return;}clips.retain(|s|s!=&text);if text.len()>2*1024*1024{return;}clips.insert(0,text);clips.truncate(100);}
impl Tools {
 #[cfg(test)]
 pub(crate) fn set_server_test_snapshot(&mut self, value:&str){self.tx=None;self.shared.lock().unwrap().tasks=value.into();}
 pub(crate) fn scroll_server(&mut self,next:bool)->bool {
  let rows=self.shared.lock().ok().and_then(|state|serde_json::from_str::<Value>(&state.tasks).ok())
   .and_then(|value|value["services"].as_array().cloned()).unwrap_or_default();
  if rows.is_empty(){return false;}
  let max=rows.len().saturating_sub(1)/3*3;
  let target=if next{(self.server_offset+3).min(max)}else{self.server_offset.saturating_sub(3)};
  let changed=target!=self.server_offset;
  self.server_offset=target;
  changed
 }
 fn start(&mut self){if self.tx.is_some(){return;}let (tx,rx)=mpsc::channel();let shared=self.shared.clone();let mut config=self.config.clone();self.tx=Some(tx);
 std::thread::spawn(move||{let pending=Arc::new(std::sync::atomic::AtomicBool::new(false));let mut seq=0;let mut prior=(0,0,0);let mut tick=Instant::now()-Duration::from_secs(60);let mut server=Instant::now()-Duration::from_secs(60);let mut edge=Instant::now()-Duration::from_secs(60);let edge_pending=Arc::new(std::sync::atomic::AtomicBool::new(false));
 loop{match rx.recv_timeout(Duration::from_millis(500)){Ok(c)=>{config=c;server=Instant::now()-Duration::from_secs(60);},Err(mpsc::RecvTimeoutError::Disconnected)=>break,Err(_)=>{}}
 #[cfg(windows)] if config.enabled[0]{if let Some((s,text))=native_clip(seq){seq=s;if let Some(text)=text{if let Ok(mut state)=shared.lock(){bounded_clip(&mut state.clips,text);}}}}else if let Ok(mut state)=shared.lock(){state.clips.clear();}
 if !config.enabled[1]{if let Ok(mut s)=shared.lock(){s.edge.clear();s.edge_error.clear();}}
 if config.enabled[1]&&edge.elapsed()>=Duration::from_secs(2)&&!edge_pending.swap(true,std::sync::atomic::Ordering::Relaxed){edge=Instant::now();let shared=shared.clone();let pending=edge_pending.clone();std::thread::spawn(move||{let(rows,error)=edge_downloads();if let Ok(mut s)=shared.lock(){s.edge=rows;s.edge_error=error;}pending.store(false,std::sync::atomic::Ordering::Relaxed);});}
 if config.enabled[3]&&tick.elapsed()>=Duration::from_secs(2){tick=Instant::now();#[cfg(windows)]{let(cpu,memory)=native_perf(&mut prior);if let Ok(mut state)=shared.lock(){state.cpu=cpu;state.memory=memory;}}}
 if config.enabled[4]&&server.elapsed()>=Duration::from_secs(30)&&!pending.swap(true,std::sync::atomic::Ordering::Relaxed){server=Instant::now();let bridge=config.bridge.clone();let state=shared.clone();let pending=pending.clone();std::thread::spawn(move||{let message=if bridge.is_empty(){"尚未配置服务器总览来源".into()}else{run_bridge(&bridge)};if let Ok(mut state)=state.lock(){state.tasks=message;state.updated=chrono::Local::now().format("%H:%M:%S").to_string();}pending.store(false,std::sync::atomic::Ordering::Relaxed);});}
 }});}
 fn save(&mut self){self.note=match self.config.save(){Ok(())=>String::new(),Err(e)=>format!("保存失败：{e}")};if let Some(tx)=&self.tx{let _=tx.send(self.config.clone());}}
 fn read_courses(&mut self){self.courses=std::fs::read(&self.config.courses).ok().filter(|v|v.len()<2*1024*1024).and_then(|raw|serde_json::from_slice(&raw).ok()).unwrap_or(Value::Null);}
 fn course_day(&self,day:NaiveDate)->Vec<String>{course_day(&self.courses,day)}
 fn start_download(&mut self){if self.downloads.iter().filter(|d|d.lock().is_ok_and(|d|d.status=="下载中")).count()>=3{self.note="最多同时下载 3 个任务".into();return;}
 let Ok(url)=url::Url::parse(self.url.trim())else{self.note="请输入完整下载地址".into();return;};if !matches!(url.scheme(),"https"|"http")||!url.username().is_empty()||url.password().is_some(){self.note="下载地址需要 HTTP/HTTPS，且不能包含密码".into();return;}
 let name=url.path_segments().and_then(|mut s|s.next_back()).filter(|s|!s.is_empty()).unwrap_or("download.bin");let name:String=name.chars().filter(|c|c.is_ascii_alphanumeric()||".-_".contains(*c)).take(100).collect();
 let root=std::env::var_os("USERPROFILE").map(std::path::PathBuf::from).unwrap_or_else(crate::model::config_dir).join("Downloads");
 let path=root.join(format!("daozai-{}-{}",chrono::Local::now().timestamp_millis(),if name.is_empty(){"download.bin"}else{&name}));
 let task=Arc::new(Mutex::new(Download{url:url.to_string(),path:path.to_string_lossy().into(),status:"下载中".into(),..Default::default()}));self.downloads.retain(|d|d.lock().is_ok_and(|d|d.status=="下载中"));self.downloads.push(task.clone());
 std::thread::spawn(move||{let d=task.lock().unwrap().clone();let partial=format!("{}.part",d.path);let result=(||->Result<(),String>{
 std::fs::create_dir_all(&root).map_err(|e|e.to_string())?;let response=ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(10)).timeout_read(Duration::from_secs(30)).build().get(&d.url).call().map_err(|_|"下载连接失败".to_string())?;
 task.lock().unwrap().total=response.header("Content-Length").and_then(|s|s.parse().ok()).unwrap_or(0);let mut input=response.into_reader();let mut out=std::fs::OpenOptions::new().write(true).create_new(true).open(&partial).map_err(|e|e.to_string())?;let mut buf=[0;65536];loop{let n=input.read(&mut buf).map_err(|_|"下载中断".to_string())?;if n==0{break;}std::io::Write::write_all(&mut out,&buf[..n]).map_err(|e|e.to_string())?;task.lock().unwrap().bytes+=n as u64;}out.sync_all().map_err(|e|e.to_string())?;drop(out);let completed=task.lock().unwrap();if completed.total>0&&completed.bytes!=completed.total{return Err("下载文件不完整".into());}drop(completed);std::fs::rename(&partial,&d.path).map_err(|e|e.to_string())?;Ok(())})();task.lock().unwrap().status=match result{Ok(())=>"已完成".into(),Err(e)=>format!("失败：{e}")};});self.url.clear();}
}
fn course_day(data:&Value,day:NaiveDate)->Vec<String>{let Some(start)=data["term_start"].as_str().and_then(|s|NaiveDate::parse_from_str(s,"%Y-%m-%d").ok())else{return vec![];};let key=day.to_string();if data["holidays"].as_array().is_some_and(|a|a.iter().any(|v|v.as_str()==Some(&key))){return vec![];}let days=(day-start).num_days();if days<0||data["term_end"].as_str().is_some_and(|end|key.as_str()>end){return vec![];}let week=days/7+1;data["rules"].as_array().map(|rules|rules.iter().filter(|r|r["weekday"].as_u64()==Some(day.weekday().number_from_monday() as u64)&&r["weeks"].as_array().is_some_and(|w|w.iter().any(|n|n.as_i64()==Some(week)))).map(|r|format!("{} · {}节\n{}\n{}\n{}",r["start"].as_str().unwrap_or(""),r["periods"].as_str().unwrap_or(""),r["course"].as_str().unwrap_or(""),r["location"].as_str().unwrap_or(""),r["teacher"].as_str().unwrap_or(""))).collect()).unwrap_or_default()}
fn run_bridge(path:&str)->String{let mut cmd=std::process::Command::new("python");cmd.arg(path);#[cfg(windows)]{use std::os::windows::process::CommandExt;cmd.creation_flags(0x08000000);}match cmd.output(){Ok(o)if o.status.success()=>String::from_utf8_lossy(&o.stdout[..o.stdout.len().min(65536)]).into(),_=>"服务器总览暂时不可用（稍后自动重试）".into()}}
impl crate::app::App {
 pub fn accept_download_url(&mut self,url:String){
  self.show_page(10);self.desktop_tools.tab=2;self.desktop_tools.open=true;
  if self.desktop_tools.config.enabled[1]{self.desktop_tools.url=url;self.desktop_tools.start_download();}else{self.desktop_tools.note="下载功能已关闭，请先在工具设置中开启".into();}
 }

 pub(crate) fn desktop_native_page(&mut self,page:usize,c:&crate::paint::Card) {
 use crate::{paint::H,theme::{self,W}};
 let t=&mut self.desktop_tools;
 t.start();
 let state=t.shared.lock().map(|s|s.clone()).unwrap_or_default();
 let card=Color32::from_rgb(29,29,33);
 match page {
 0=>{
  if t.courses.is_null(){t.read_courses();}
  let today=chrono::Local::now().date_naive();
  if t.week {
   let start=today-chrono::Duration::days(today.weekday().num_days_from_monday() as i64)+chrono::Duration::days(t.offset);
   for i in 0..7 {let day=start+chrono::Duration::days(i);let y=33.0+i as f32*27.0;
    c.fill(38.0,y,342.0,24.0,7.0,card);
    c.text(47.0,y+16.0,&format!("周{} {}",["一","二","三","四","五","六","日"][i as usize],day.format("%m/%d")),9.0,W::Semibold,theme::WHITE,H::Left);
    let rows=t.course_day(day);let line=if rows.is_empty(){"无课".into()}else{rows.iter().map(|row|format!("{} {}",row.lines().next().unwrap_or(""),row.lines().nth(1).unwrap_or("课程"))).collect::<Vec<_>>().join(" · ")};
    c.text(128.0,y+16.0,&c.fit(&line,9.0,W::Regular,240.0),9.0,W::Regular,theme::GRAY,H::Left);
    if c.hit(38.0,y,342.0,24.0,("week-details",i)).clicked(){t.open=true;t.tab=0;}
   }
   if c.hit(315.0,3.0,55.0,27.0,"course-collapse").clicked(){t.week=false;}
   c.text(340.0,21.0,"两日",10.0,W::Regular,theme::GRAY,H::Center);
  } else {
   for i in 0..2 {let day=today+chrono::Duration::days(i);let x=42.0+i as f32*171.0;
    c.fill(x,36.0,164.0,76.0,10.0,card);
    c.text(x+9.0,51.0,&format!("{} {}",if i==0{"今天"}else{"明天"},day.format("%m/%d")),10.0,W::Semibold,theme::WHITE,H::Left);
    let rows=t.course_day(day);if rows.is_empty(){c.text(x+9.0,76.0,if t.courses.is_null(){"尚未导入独立课表"}else{"当天无课"},10.0,W::Regular,theme::GRAY,H::Left);}
    for(j,row)in rows.iter().take(2).enumerate(){let label=format!("{} {}",row.lines().next().unwrap_or(""),row.lines().nth(1).unwrap_or(""));c.text(x+9.0,72.0+j as f32*20.0,&c.fit(&label,9.0,W::Regular,145.0),9.0,W::Regular,theme::GRAY,H::Left);}
    if c.hit(x,36.0,164.0,76.0,("native-course",i)).clicked(){t.week=true;t.offset=0;}
   }
  }
 }
 1=>{
  if !t.config.enabled[0]{c.text(45.0,67.0,"剪贴板记录已关闭",11.0,W::Regular,theme::GRAY,H::Left);}
  else if state.clips.is_empty(){c.text(45.0,67.0,"复制文字后将在这里显示",11.0,W::Regular,theme::GRAY,H::Left);}
  else {for(i,value) in state.clips.iter().take(3).enumerate(){let y=36.0+i as f32*25.0;c.fill(40.0,y,338.0,22.0,7.0,card);c.text(50.0,y+15.0,&c.fit(&single_line(value),10.0,W::Regular,310.0),10.0,W::Regular,theme::WHITE,H::Left);if c.hit(40.0,y,338.0,22.0,("native-clip",i)).clicked(){c.ui.ctx().copy_text(value.clone());}}}
 }
 2=>{
  if c.hit(40.0,5.0,335.0,25.0,"download-input-open").on_hover_text("输入或拖入 HTTP/HTTPS 下载地址").clicked(){t.open=true;t.tab=2;}
  if t.downloads.is_empty()&&state.edge.is_empty(){c.text(45.0,65.0,"暂无岛仔下载任务",11.0,W::Regular,theme::GRAY,H::Left);}
  for(i,d) in state.edge.iter().cloned().chain(t.downloads.iter().filter_map(|d|d.lock().ok().map(|d|d.clone()))).take(3).enumerate(){let y=40.0+i as f32*25.0;let name=std::path::Path::new(&d.path).file_name().and_then(|n|n.to_str()).unwrap_or("下载");c.text(45.0,y+12.0,&c.fit(name,10.0,W::Regular,210.0),10.0,W::Regular,theme::WHITE,H::Left);c.text(370.0,y+12.0,&download_progress(&d),9.0,W::Regular,theme::GRAY,H::Right);}
 }
 3=>{
  if t.config.shortcuts.is_empty(){c.text(45.0,65.0,"还没有快捷入口，可在设置中添加",10.0,W::Regular,theme::GRAY,H::Left);}
  for(i,(name,target)) in t.config.shortcuts.iter().take(6).enumerate(){let x=42.0+(i%3) as f32*114.0;let y=37.0+(i/3) as f32*35.0;c.fill(x,y,108.0,29.0,9.0,card);c.text(x+54.0,y+19.0,&c.fit(name,10.0,W::Regular,95.0),10.0,W::Regular,theme::WHITE,H::Center);if c.hit(x,y,108.0,29.0,("native-shortcut",i)).clicked(){if target.starts_with("http://")||target.starts_with("https://"){c.ui.ctx().open_url(egui::OpenUrl::new_tab(target));}else{self.pending.push(crate::app::Cmd::Open(target.into()));}}}
 }
 4=>{for(i,(label,value))in [("CPU",state.cpu),("内存",state.memory)].iter().enumerate(){let x=45.0+i as f32*170.0;c.fill(x,37.0,155.0,70.0,10.0,card);c.text(x+12.0,55.0,label,10.0,W::Semibold,theme::GRAY,H::Left);c.text(x+12.0,84.0,&format!("{value:.1}%"),22.0,W::Bold,theme::WHITE,H::Left);c.fill(x+12.0,94.0,130.0,4.0,2.0,Color32::from_gray(60));c.fill(x+12.0,94.0,130.0*(*value as f32/100.0).clamp(0.0,1.0),4.0,2.0,theme::BLUE);}}
 5=>{
  if !t.config.enabled[4] {c.text(45.0,62.0,"服务器读取已关闭",11.0,W::Regular,theme::GRAY,H::Left);}
  else if state.tasks.is_empty(){c.text(45.0,62.0,"正在读取服务器状态…",11.0,W::Regular,theme::GRAY,H::Left);}
  else {
   let parsed=serde_json::from_str::<Value>(&state.tasks).ok();
   if let Some(rows)=parsed.as_ref().and_then(|v|v["services"].as_array()) {
    t.server_offset=t.server_offset.min(rows.len().saturating_sub(1)/3*3);
    c.text(45.0,43.0,"名称",9.0,W::Semibold,theme::GRAY,H::Left);
    c.text(155.0,43.0,"用途",9.0,W::Semibold,theme::GRAY,H::Left);
    c.text(325.0,43.0,"状态",9.0,W::Semibold,theme::GRAY,H::Right);
    for(i,row) in rows.iter().skip(t.server_offset).take(3).enumerate(){
     let y=48.0+i as f32*22.0;
     c.fill(39.0,y,296.0,20.0,6.0,card);
     c.text(45.0,y+14.0,&c.fit(row["name"].as_str().unwrap_or("—"),9.0,W::Regular,103.0),9.0,W::Regular,theme::WHITE,H::Left);
     c.text(155.0,y+14.0,&c.fit(row["description"].as_str().unwrap_or("—"),9.0,W::Regular,115.0),9.0,W::Regular,theme::GRAY,H::Left);
     let color=match row["state"].as_str(){Some("ok")=>theme::GREEN,Some("down")=>theme::RED_TOP,_=>theme::GRAY};
     c.text(325.0,y+14.0,row["label"].as_str().unwrap_or("未知"),9.0,W::Semibold,color,H::Right);
    }
   }else{c.text(45.0,65.0,"服务器连接失败或数据不可用",10.0,W::Regular,theme::GRAY,H::Left);}
  }
 }
 _=>{}
 }
 }
 pub(crate) fn desktop_course_card(&mut self,c:&crate::paint::Card) {
 use crate::{theme::{self,W},paint::H};let t=&mut self.desktop_tools;
 if t.courses.is_null(){t.read_courses();}
 c.centered_text(56.0,18.0,"‹ 返回",10.0,W::Regular,theme::GRAY,H::Center);
 if c.hit(34.0,5.0,48.0,25.0,"course-back").clicked(){t.mini=false;}
 c.centered_text(200.0,18.0,"今明课表",11.0,W::Semibold,theme::WHITE,H::Center);
 let today=chrono::Local::now().date_naive();
 for i in 0..2 {let day=today+chrono::Duration::days(i);let x=42.0+i as f32*171.0;
 c.fill(x,36.0,164.0,76.0,10.0,Color32::from_rgb(28,28,31));
 c.text(x+9.0,51.0,&format!("{} {}",if i==0{"今天"}else{"明天"},day.format("%m/%d")),10.0,W::Semibold,theme::WHITE,H::Left);
 let rows=t.course_day(day);if rows.is_empty(){c.text(x+9.0,76.0,if t.courses.is_null(){"尚未导入独立课表"}else{"当天无课"},10.0,W::Regular,theme::GRAY,H::Left);}
 for(j,row)in rows.iter().take(2).enumerate(){let parts:Vec<_>=row.lines().collect();let label=format!("{} {}",parts.first().unwrap_or(&""),parts.get(1).unwrap_or(&""));c.text(x+9.0,72.0+j as f32*20.0,&c.fit(&label,9.0,W::Regular,145.0),9.0,W::Regular,theme::GRAY,H::Left);}
 if c.hit(x,36.0,164.0,76.0,("course-expand",i)).clicked(){t.open=true;t.tab=0;t.week=true;}
 }
 }
 pub fn desktop_tools_ui(&mut self,ctx:&egui::Context){self.desktop_tools.start();if !self.desktop_tools.open{return;}let t=&mut self.desktop_tools;let mut open=t.open;
 let id=egui::ViewportId::from_hash_of("desktop-tools-window");let builder=egui::ViewportBuilder::default().with_title("岛仔 · 桌面工具").with_inner_size([if t.week{980.0}else{700.0},540.0]).with_min_inner_size([620.0,400.0]);
 ctx.show_viewport_immediate(id,builder,|ctx,_class|{let original=ctx.style();ctx.set_style(crate::settings_theme::island_style(&original,true));if ctx.input(|i|i.viewport().close_requested()){open=false;}
 egui::CentralPanel::default().frame(egui::Frame::new().fill(Color32::from_rgb(25,25,28)).inner_margin(20)).show(ctx,|ui|{ui.heading("桌面工具");ui.add_space(8.0);ui.horizontal_wrapped(|ui|{for(i,name)in [if t.week{"一周课表"}else{"两日课表"},"剪贴板","下载","快捷启动","系统性能","服务器任务","设置"].iter().enumerate(){if ui.selectable_label(t.tab==i,*name).clicked(){t.tab=i;}}});ui.separator();
 let state=t.shared.lock().map(|s|s.clone()).unwrap_or_default();egui::ScrollArea::vertical().show(ui,|ui|{match t.tab{
 0=>{if t.courses.is_null(){t.read_courses();}ui.horizontal(|ui|{if ui.button(if t.week{"‹ 两日预览"}else{"展开完整一周 ›"}).clicked(){t.week=!t.week;t.offset=0;ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(if t.week{980.0}else{700.0},540.0)));}if ui.small_button("上一周").clicked(){t.offset-=7;}if ui.small_button("今天").clicked(){t.offset=0;}if ui.small_button("下一周").clicked(){t.offset+=7;}if ui.small_button("刷新课表").clicked(){t.read_courses();}});
 if t.courses.is_null(){ui.label("请在设置中选择独立课表数据文件。");}else{let today=chrono::Local::now().date_naive();let anchor=today+chrono::Duration::days(t.offset);let start=if t.week{anchor-chrono::Duration::days(anchor.weekday().num_days_from_monday() as i64)}else{anchor};let count=if t.week{7}else{2};ui.add_space(10.0);ui.columns(count,|cols|{for(i,col)in cols.iter_mut().enumerate(){let date=start+chrono::Duration::days(i as i64);egui::Frame::group(col.style()).corner_radius(12.0).show(col,|ui|{let label=format!("{} {}",["周一","周二","周三","周四","周五","周六","周日"][date.weekday().num_days_from_monday()as usize],date.format("%m/%d"));if ui.selectable_label(date==today,label).clicked(){t.week=true;}let rows=t.course_day(date);if rows.is_empty(){ui.weak("当天无课");}for row in rows{ui.add_space(8.0);ui.label(row);ui.separator();}});}});}},
 1=>{if !t.config.enabled[0]{ui.label("剪贴板记录已关闭");}else{ui.weak("仅文本 · 最近 100 条 · 只保存在内存中");ui.text_edit_singleline(&mut t.filter);if ui.button("清空历史").clicked(){t.shared.lock().unwrap().clips.clear();}for (i,text)in state.clips.iter().filter(|s|s.contains(&t.filter)).enumerate(){ui.horizontal(|ui|{if ui.small_button("复制").clicked(){ctx.copy_text(text.clone());}if ui.add(egui::Label::new(format!("{}  {}",i+1,single_line(text))).truncate().sense(egui::Sense::click())).clicked(){ctx.copy_text(text.clone());}});}}},
 2=>{ui.weak("Edge 正在下载的文件与岛仔任务 · 可拖入 HTTP/HTTPS 地址");
 ui.add_enabled_ui(t.config.enabled[1],|ui|{ui.add(egui::TextEdit::singleline(&mut t.url).hint_text("https://…").desired_width(f32::INFINITY));if ui.button("开始下载到 Downloads").clicked(){t.start_download();}});
 if !state.edge_error.is_empty(){ui.colored_label(Color32::LIGHT_RED,&state.edge_error);}
 for d in state.edge.iter().cloned().chain(t.downloads.iter().filter_map(|d|d.lock().ok().map(|d|d.clone()))){
 egui::Frame::group(ui.style()).corner_radius(10.0).show(ui,|ui|{ui.add(egui::Label::new(format!("{} · {}",if d.status=="Edge"{"Edge"}else{"岛仔"},std::path::Path::new(&d.path).file_name().and_then(|n|n.to_str()).unwrap_or("下载文件"))).truncate());
 if d.total>0{ui.add(egui::ProgressBar::new((d.bytes as f32/d.total as f32).clamp(0.0,1.0)).show_percentage());}ui.label(download_progress(&d));
 if d.status=="已完成"&&ui.small_button("打开所在目录").clicked(){if let Some(parent)=std::path::Path::new(&d.path).parent(){self.pending.push(crate::app::Cmd::Open(parent.to_owned()));}}
 });}},
 3=>{ui.add_enabled_ui(t.config.enabled[2],|ui|{ui.label("名称");ui.text_edit_singleline(&mut t.label);ui.label("应用/文件夹路径或 HTTPS 网址");ui.text_edit_singleline(&mut t.target);if ui.button("添加快捷入口").clicked()&&!t.label.trim().is_empty()&&!t.target.trim().is_empty()&&t.config.shortcuts.len()<50{t.config.shortcuts.push((t.label.trim().into(),t.target.trim().into()));t.label.clear();t.target.clear();t.save();}let mut remove=None;for(i,(name,target))in t.config.shortcuts.iter().enumerate(){ui.horizontal(|ui|{if ui.button(name).clicked(){if target.starts_with("https://")||target.starts_with("http://"){ctx.open_url(egui::OpenUrl::new_tab(target));}else{self.pending.push(crate::app::Cmd::Open(target.into()));}}if ui.small_button("移除").clicked(){remove=Some(i);}});}if let Some(i)=remove{t.config.shortcuts.remove(i);t.save();}});},
 4=>{if !t.config.enabled[3]{ui.label("系统采样已关闭");}else{ui.heading(format!("CPU  {:.1}%",state.cpu));ui.add(egui::ProgressBar::new(state.cpu as f32/100.0));ui.heading(format!("内存  {:.1}%",state.memory));ui.add(egui::ProgressBar::new(state.memory as f32/100.0));ui.weak("每 2 秒采样；首次 CPU 采样需等待下一次更新。");}},
 5=>{ui.horizontal(|ui|{ui.label(format!("最近更新 {} · 每 30 秒同步",state.updated));if ui.button("刷新").clicked(){t.save();}});if !t.config.enabled[4]{ui.label("服务器同步已关闭");}else if let Ok(data)=serde_json::from_str::<Value>(&state.tasks){egui::Grid::new("server-overview-table").striped(true).show(ui,|ui|{ui.strong("名称");ui.strong("用途");ui.strong("状态");ui.end_row();if let Some(rows)=data["services"].as_array(){for row in rows{ui.label(row["name"].as_str().unwrap_or("—"));ui.label(row["description"].as_str().unwrap_or("—"));let color=match row["state"].as_str(){Some("ok")=>Color32::from_rgb(71,200,120),Some("down")=>Color32::from_rgb(240,95,95),_=>Color32::GRAY};ui.colored_label(color,row["label"].as_str().unwrap_or("未知"));ui.end_row();}}});}else{ui.label(if state.tasks.is_empty(){"正在读取服务器状态…"}else{&state.tasks});}},
 _=>{let mut changed=false;for(i,label)in ["剪贴板历史","下载任务","快捷启动","系统性能","服务器任务总览"].iter().enumerate(){changed|=ui.checkbox(&mut t.config.enabled[i],*label).changed();}ui.separator();ui.label("独立课表 JSON 文件");changed|=ui.text_edit_singleline(&mut t.config.courses).changed();ui.label("服务器总览读取脚本（只运行你配置的本机 Python 文件）");changed|=ui.text_edit_singleline(&mut t.config.bridge).changed();if changed{t.courses=Value::Null;t.save();}ui.weak("剪贴板历史不写入磁盘；关闭记录时清空内存缓存。");}
 }if !t.note.is_empty(){ui.colored_label(Color32::from_rgb(240,170,90),&t.note);}});});ctx.set_style(original);});t.open=open;
 }
}
#[cfg(windows)]
fn native_clip(previous:u32)->Option<(u32,Option<String>)>{use std::ffi::c_void;#[link(name="user32")]extern "system"{fn GetClipboardSequenceNumber()->u32;fn OpenClipboard(w:*mut c_void)->i32;fn CloseClipboard()->i32;fn GetClipboardData(f:u32)->*mut c_void;fn RegisterClipboardFormatW(n:*const u16)->u32;fn IsClipboardFormatAvailable(f:u32)->i32;}#[link(name="kernel32")]extern "system"{fn GlobalLock(h:*mut c_void)->*mut c_void;fn GlobalUnlock(h:*mut c_void)->i32;fn GlobalSize(h:*mut c_void)->usize;}
 unsafe{let seq=GetClipboardSequenceNumber();if seq==previous{return None;}if OpenClipboard(std::ptr::null_mut())==0{return None;}let excluded:Vec<u16>="ExcludeClipboardContentFromMonitorProcessing\0".encode_utf16().collect();let format=RegisterClipboardFormatW(excluded.as_ptr());if format!=0&&IsClipboardFormatAvailable(format)!=0{CloseClipboard();return Some((seq,None));}let h=GetClipboardData(13);let text=if h.is_null(){None}else{let size=GlobalSize(h)/2;if size>1024*1024{CloseClipboard();return Some((seq,None));}let p=GlobalLock(h)as *const u16;if p.is_null(){None}else{let raw=std::slice::from_raw_parts(p,size);let n=raw.iter().position(|v|*v==0).unwrap_or(size);let text=String::from_utf16_lossy(&raw[..n]);GlobalUnlock(h);Some(text)}};CloseClipboard();Some((seq,text))}}
#[cfg(windows)]
fn native_perf(prior:&mut(u64,u64,u64))->(f64,f64){#[repr(C)]struct Memory{length:u32,load:u32,total:u64,available:u64,page:u64,page_available:u64,virtual_total:u64,virtual_available:u64,extended:u64}#[link(name="kernel32")]extern "system"{fn GetSystemTimes(i:*mut u64,k:*mut u64,u:*mut u64)->i32;fn GlobalMemoryStatusEx(m:*mut Memory)->i32;}
 unsafe{let(mut i,mut k,mut u)=(0,0,0);let valid=GetSystemTimes(&mut i,&mut k,&mut u)!=0;let total=k.saturating_sub(prior.1)+u.saturating_sub(prior.2);let cpu=if valid&&prior.1!=0&&total>0{100.0*(total.saturating_sub(i.saturating_sub(prior.0)))as f64/total as f64}else{0.0};if valid{*prior=(i,k,u);}let mut m:Memory=std::mem::zeroed();m.length=std::mem::size_of::<Memory>()as u32;let memory=if GlobalMemoryStatusEx(&mut m)!=0{m.load as f64}else{0.0};(cpu.clamp(0.0,100.0),memory)}}
#[cfg(test)]mod tests{use super::*;#[test]fn clipboard_is_bounded_and_deduplicated(){let mut rows=vec![];for n in 0..110{bounded_clip(&mut rows,n.to_string());}assert_eq!(rows.len(),100);bounded_clip(&mut rows,"109".into());assert_eq!(rows.len(),100);assert_eq!(rows[0],"109");}#[test]fn independent_course_rules_respect_weeks_and_holidays(){let data=json!({"term_start":"2026-09-07","term_end":"2027-01-17","holidays":["2026-09-14"],"rules":[{"weekday":1,"weeks":[1,2],"course":"课程","start":"08:10"}]});assert_eq!(course_day(&data,NaiveDate::from_ymd_opt(2026,9,7).unwrap()).len(),1);assert!(course_day(&data,NaiveDate::from_ymd_opt(2026,9,14).unwrap()).is_empty());assert!(course_day(&data,NaiveDate::from_ymd_opt(2026,9,21).unwrap()).is_empty());}}
