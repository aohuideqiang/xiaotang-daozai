//! Today's personal calendar in the original todo list. Completion stays local.
use crate::{app::App,calendar::{self,write::Item},local_calendar::{Draft,Entry}};
use eframe::egui;
use std::{collections::BTreeSet,sync::mpsc,time::{Duration,Instant}};
enum Reply{List(Result<Vec<Item>,String>),Saved(Result<(),String>)}
pub(crate) struct State{
 pub items:Vec<Item>,pub done:BTreeSet<String>,pub error:String,pub draft:Option<Draft>,pub selected:Option<Item>,
 rx:Option<mpsc::Receiver<Reply>>,next:Instant,day:chrono::NaiveDate,configs:Vec<calendar::Config>,account:usize,completion_error:String,
}
impl Default for State{fn default()->Self{
 let (done,completion_error)=match std::fs::read(crate::model::config_dir().join("calendar-completed.json")){
 Ok(raw)=>match serde_json::from_slice(&raw){Ok(v)=>(v,String::new()),Err(_)=>(BTreeSet::new(),"本机完成状态文件无法读取，已暂停修改，请检查文件".into())},
 Err(e)if e.kind()==std::io::ErrorKind::NotFound=>(BTreeSet::new(),String::new()),Err(_)=>(BTreeSet::new(),"无法读取本机完成状态".into())};
 Self{items:vec![],done,error:String::new(),draft:None,selected:None,rx:None,next:Instant::now(),day:chrono::Local::now().date_naive(),configs:vec![],account:0,completion_error}
}}
impl State{
 pub fn key(i:&Item)->String{format!("{}|{}",i.config.credential_name(),calendar::dedupe_key(&i.event))}
 pub fn toggle(&mut self,index:usize){
  if !self.completion_error.is_empty(){self.error=self.completion_error.clone();return;}
  let Some(item)=self.items.get(index)else{return;};let key=Self::key(item);let mut done=self.done.clone();if !done.remove(&key){done.insert(key);}
  let result=serde_json::to_vec(&done).map_err(|e|e.to_string()).and_then(|raw|{let path=crate::model::config_dir().join("calendar-completed.json");crate::model::atomic_write(&path,&raw).map_err(|e|e.to_string())});
  match result{Ok(())=>self.done=done,Err(_)=>self.error="本机完成状态保存失败".into()}
 }
 pub fn edit(&mut self,index:Option<usize>){
  self.selected=index.and_then(|i|self.items.get(i)).cloned();
  if self.selected.as_ref().is_some_and(|i|i.readonly){self.error="重复日程或缺少版本信息的日程只可展示".into();return;}
  self.draft=Some(self.selected.as_ref().map(|i|Draft::edit(&Entry{event:i.event.clone(),reminder:None})).unwrap_or_else(||Draft::new(chrono::Local::now().date_naive())));
  self.error.clear();
 }
 fn poll(&mut self,ctx:&egui::Context){
  let configs:Vec<_>=calendar::load_configs().into_iter().filter(|c|c.provider==calendar::Provider::Custom).collect();
  if configs!=self.configs{self.configs=configs;self.account=0;self.items.clear();self.next=Instant::now();}
  let today=chrono::Local::now().date_naive();if self.day!=today{self.day=today;self.items.clear();self.next=Instant::now();}
  if let Some(reply)=self.rx.as_ref().and_then(|r|r.try_recv().ok()){
   self.rx=None;match reply{
    Reply::List(result)=>match result{Ok(items)=>{self.items=items;self.error.clear();},Err(e)=>self.error=e},
    Reply::Saved(result)=>{match result{Ok(())=>{self.draft=None;self.selected=None;self.error="日历已同步，正在刷新".into();},Err(e)=>self.error=e}self.next=Instant::now();}
   }
  }
  if self.rx.is_none()&&Instant::now()>=self.next{
   self.next=Instant::now()+Duration::from_secs(300);
   if !self.configs.is_empty(){let configs=self.configs.clone();let (tx,rx)=mpsc::channel();self.rx=Some(rx);std::thread::spawn(move||{
    let result=(||{let mut items=vec![];for c in configs{let p=calendar::secrets::load(&c)?;items.extend(calendar::write::list(&c,&p,today).map_err(|e|e.message)?);}Ok(items)})();let _=tx.send(Reply::List(result));
   });}
  }
  ctx.request_repaint_after(Duration::from_millis(500));
 }
 fn save(&mut self,delete:bool){
  if self.rx.is_some(){return;}
  let Some(d)=&self.draft else{return;};
  let event=if delete{match &self.selected{Some(i)=>i.event.clone(),None=>return}}else{match d.entry(){Ok(e)=>e.event,Err(e)=>{self.error=e;return;}}};
  let Some(config)=self.selected.as_ref().map(|i|i.config.clone()).or_else(||self.configs.get(self.account).cloned())else{self.error="请先在设置中连接个人 CalDAV 日历".into();return;};
  let old=self.selected.clone();let (tx,rx)=mpsc::channel();self.rx=Some(rx);self.error="正在同步，请稍候…".into();
  std::thread::spawn(move||{let result=calendar::secrets::load(&config).and_then(|p|calendar::write::save(&config,&p,&event,old.as_ref(),delete).map_err(|e|e.message));let _=tx.send(Reply::Saved(result));});
 }
}
impl App{
 pub fn calendar_tasks_ui(&mut self,ctx:&egui::Context){
  let s=&mut self.calendar_tasks;s.poll(ctx);if s.draft.is_none(){return;}
  let mut open=true;let mut save=false;let mut delete=false;let mut cancel=false;
  ctx.show_viewport_immediate(egui::ViewportId::from_hash_of("calendar-task-form"),egui::ViewportBuilder::default().with_title(if s.selected.is_some(){"编辑个人日历日程"}else{"加一个 · 个人日历"}).with_inner_size([430.0,460.0]).with_min_inner_size([400.0,400.0]),|ctx,_|{
   if ctx.input(|i|i.viewport().close_requested()){open=false;}
   let original=ctx.style();ctx.set_style(crate::settings_theme::island_style(&original,true));
   egui::CentralPanel::default().frame(egui::Frame::new().fill(egui::Color32::from_rgb(25,25,28)).inner_margin(22)).show(ctx,|ui|{
    ui.heading(if s.selected.is_some(){"编辑日程"}else{"加一个"});
    ui.visuals_mut().override_text_color=Some(egui::Color32::from_gray(235));
    ui.spacing_mut().item_spacing=egui::vec2(10.0,10.0);
    ui.add_enabled_ui(s.rx.is_none(),|ui|{
     if s.selected.is_none(){egui::ComboBox::from_id_salt("personal-calendar-choice").selected_text(s.configs.get(s.account).map(|c|c.username.as_str()).unwrap_or("尚未连接个人日历")).show_ui(ui,|ui|{for(i,c)in s.configs.iter().enumerate(){ui.selectable_value(&mut s.account,i,&c.username);}});}
     let d=s.draft.as_mut().unwrap();
     ui.label("标题");ui.add(egui::TextEdit::singleline(&mut d.title).desired_width(330.0));
     ui.label("开始时间 · 年-月-日 / 时:分");ui.horizontal(|ui|{ui.add(egui::TextEdit::singleline(&mut d.start_date).desired_width(150.0));ui.add(egui::TextEdit::singleline(&mut d.start_time).desired_width(80.0));});
     ui.label("结束时间 · 年-月-日 / 时:分");ui.horizontal(|ui|{ui.add(egui::TextEdit::singleline(&mut d.end_date).desired_width(150.0));ui.add(egui::TextEdit::singleline(&mut d.end_time).desired_width(80.0));});
     ui.checkbox(&mut d.all_day,"全天日程");
     ui.weak("保存会同步至已连接日历；勾选完成仅保存在本机。");
     if s.selected.is_some(){if d.confirm_delete{ui.colored_label(egui::Color32::LIGHT_RED,"确认删除？这会同步删除日历中的日程。");ui.horizontal(|ui|{delete=ui.button("确认同步删除").clicked();if ui.button("保留日程").clicked(){d.confirm_delete=false;}});}else if ui.button("删除日程…").clicked(){d.confirm_delete=true;}}
     ui.horizontal(|ui|{save=ui.button("保存到日历").clicked();cancel=ui.button("取消").clicked();});
    });
    if !s.error.is_empty(){ui.colored_label(egui::Color32::from_rgb(245,175,100),&s.error);}
   });ctx.set_style(original);
  });
  if save{s.save(false);}else if delete{s.save(true);}else if (!open||cancel)&&s.rx.is_none(){s.draft=None;s.selected=None;}
 }
}
