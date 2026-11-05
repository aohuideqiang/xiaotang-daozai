//! Explicit opt-in live CRUD check. Only deletes the newly generated test UID.
use daozai::{calendar::{self,write},mail::Meeting};
fn main()->Result<(),String>{
 if !std::env::args().any(|a|a=="--live-calendar-crud"){return Err("Pass --live-calendar-crud to test the connected personal calendar".into());}
 let cfg=calendar::load_configs().into_iter().find(|c|c.provider==calendar::Provider::Custom).ok_or("No personal calendar configured")?;
 let password=calendar::secrets::load(&cfg)?;
 let today=chrono::Local::now().date_naive();
 let before=write::list(&cfg,&password,today).map_err(|e|e.message)?;
 let uid=format!("karma-acceptance-{}-{}",std::process::id(),chrono::Utc::now().timestamp_micros());
 let start=today.and_hms_opt(12,0,0).unwrap().and_local_timezone(chrono::Local).single().ok_or("Invalid local date")?.timestamp();
 let mut event=Meeting{uid:uid.clone(),title:"KarMa 岛仔验收临时日程（自动清理）".into(),start,end:Some(start+900),all_day:false,at:String::new()};
 write::save(&cfg,&password,&event,None,false).map_err(|e|e.message)?;println!("CREATE OK");
 let result=(||->Result<(),String>{
  let items=write::list(&cfg,&password,today).map_err(|e|e.message)?;
  let old=items.into_iter().find(|i|i.event.uid==uid).ok_or("Created event not found")?;println!("READ OK");
  event.title.push_str(" · 已修改");event.start+=60;event.end=event.end.map(|t|t+60);
  write::save(&cfg,&password,&event,Some(&old),false).map_err(|e|e.message)?;println!("UPDATE OK");
  let items=write::list(&cfg,&password,today).map_err(|e|e.message)?;
  let updated=items.iter().find(|i|i.event.uid==uid).ok_or("Updated event missing")?;
  if updated.event.title!=event.title||updated.event.start!=event.start{return Err("Update readback mismatch".into());}println!("UPDATE READBACK OK");
  if write::save(&cfg,&password,&event,Some(&old),false).is_ok(){return Err("Stale ETag write was accepted".into());}println!("CONFLICT REJECTED OK");
  Ok(())
 })();
 // Cleanup even if an assertion above failed. Match only the generated test UID.
 let after=write::list(&cfg,&password,today).map_err(|e|e.message)?;
 for item in after.iter().filter(|i|i.event.uid==uid){write::save(&cfg,&password,&item.event,Some(item),true).map_err(|e|e.message)?;}
 let final_items=write::list(&cfg,&password,today).map_err(|e|e.message)?;
 if final_items.iter().any(|i|i.event.uid==uid){return Err("Test event cleanup failed".into());}println!("DELETE AND CLEANUP OK");
 for old in before{
  if !final_items.iter().any(|i|i.href==old.href&&i.etag==old.etag&&i.event==old.event){return Err("Existing calendar item changed during acceptance; investigate concurrent modification".into());}
 }println!("EXISTING TODAY EVENTS PRESERVED OK");result
}
