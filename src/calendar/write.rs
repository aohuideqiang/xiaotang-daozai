//! Conditional CalDAV writes. Credentials never leave the configured origin.
use super::*;
#[derive(Clone)]
pub struct Item { pub config:Config, pub event:crate::mail::Meeting, pub href:String, pub etag:String, pub raw:String, pub readonly:bool }
fn property(xml:&str,name:&str)->Option<String>{
    regex::Regex::new(&format!(r"(?is)<(?:[a-z0-9]+:)?{name}\b[^>]*>(.*?)</(?:[a-z0-9]+:)?{name}>" )).ok()?.captures(xml).map(|c|unescape_xml(&c[1]))
}
fn target(cfg:&Config,href:&str)->Result<String,Failure>{
    let base=url::Url::parse(&cfg.base()).map_err(|_|Failure::text("日历地址无效"))?;
    let u=base.join(href).map_err(|_|Failure::text("日程地址无效"))?;
    if u.origin()!=base.origin() || !u.username().is_empty() || u.password().is_some() || u.fragment().is_some(){return Err(Failure::text("拒绝向其他服务器发送日历凭据"));}
    Ok(u.into())
}
pub fn list(cfg:&Config,password:&str,day:NaiveDate)->Result<Vec<Item>,Failure>{
    let utc=|d:NaiveDate|d.and_hms_opt(0,0,0).unwrap().and_local_timezone(chrono::Local).earliest().unwrap().with_timezone(&chrono::Utc);
    let (lo,hi)=(utc(day),utc(day.succ_opt().unwrap()));
    let a=agent();let href=cfg.href.clone().map(Ok).unwrap_or_else(||discover(&a,cfg,password))?;
    let body=report_body(cfg.provider,&lo.format("%Y%m%dT%H%M%SZ").to_string(),&hi.format("%Y%m%dT%H%M%SZ").to_string());
    // Original objects are required to detect recurrence and preserve untouched properties.
    let body=regex::Regex::new(r"<c:calendar-data>.*?</c:calendar-data>").unwrap().replace(&body,"<c:calendar-data/>");
    let xml=dav(&a,"REPORT",&href,"1",&body,cfg,password)?;
    if let Some(code)=report_status_error(&xml){return Err(Failure::status(cfg.provider,code));}
    parse_items(cfg,&href,&xml,lo,hi)
}
fn parse_items(cfg:&Config,href:&str,xml:&str,lo:chrono::DateTime<chrono::Utc>,hi:chrono::DateTime<chrono::Utc>)->Result<Vec<Item>,Failure>{
    let re=regex::Regex::new(r"(?is)<(?:[a-z0-9]+:)?response\b[^>]*>(.*?)</(?:[a-z0-9]+:)?response>").unwrap();
    let mut items=Vec::new();
    for cap in re.captures_iter(xml){
        let block=&cap[1];let Some(raw)=calendar_data(block).into_iter().next() else{continue;};
        let resource=property(block,"href").ok_or_else(||Failure::text("服务器未返回日程地址"))?;
        let resource=target(cfg,&join_url(href,&resource))?;
        let etag=property(block,"getetag").unwrap_or_default();
        let readonly=is_recurring(&raw)||raw.matches("BEGIN:VEVENT").count()!=1||etag.is_empty()||etag.starts_with("W/");
        for event in events_in_range(block,lo,hi)?{items.push(Item{config:cfg.clone(),event,href:resource.clone(),etag:etag.clone(),raw:raw.clone(),readonly});}
    }
    items.sort_by_key(|i|i.event.start);Ok(items)
}
pub fn is_recurring(raw:&str)->bool{raw.replace("\r\n", "\n").lines().any(|l|matches!(l.split([';',':']).next(),Some("RRULE"|"RDATE"|"RECURRENCE-ID")))}
fn escape(s:&str)->String{s.replace('\\',"\\\\").replace('\r',"").replace('\n',"\\n").replace(';',"\\;").replace(',',"\\,")}
pub fn content(event:&crate::mail::Meeting,original:Option<&str>)->Result<String,Failure>{
    if event.title.trim().is_empty()||event.title.chars().any(|c|c=='\0')||event.end.is_none_or(|e|e<=event.start){return Err(Failure::text("标题或时间无效"));}
    let stamp=|t:i64|chrono::DateTime::from_timestamp(t,0).map(|d|d.format("%Y%m%dT%H%M%SZ").to_string()).ok_or_else(||Failure::text("时间无效"));
    let date=|t:i64|chrono::DateTime::from_timestamp(t,0).map(|d|d.with_timezone(&chrono::Local).format("%Y%m%d").to_string()).ok_or_else(||Failure::text("日期无效"));
    let times=if event.all_day{format!("DTSTART;VALUE=DATE:{}\r\nDTEND;VALUE=DATE:{}",date(event.start)?,date(event.end.unwrap())?)}else{format!("DTSTART:{}\r\nDTEND:{}",stamp(event.start)?,stamp(event.end.unwrap())?)};
    let fields=format!("SUMMARY:{}\r\n{times}\r\nDTSTAMP:{}\r\nLAST-MODIFIED:{}\r\n",escape(&event.title),stamp(chrono::Utc::now().timestamp())?,stamp(chrono::Utc::now().timestamp())?);
    if let Some(raw)=original{
        if is_recurring(raw)||raw.matches("BEGIN:VEVENT").count()!=1{return Err(Failure::text("重复日程暂不支持编辑或删除"));}
        let unfolded=raw.replace("\r\n","\n").replace("\n ","").replace("\n\t","");
        let mut out=String::new();let mut depth=0;
        for l in unfolded.lines(){
            if l=="BEGIN:VEVENT"{depth=1;out.push_str(l);out.push_str("\r\n");out.push_str(&fields);continue;}
            if depth>0&&l.starts_with("BEGIN:"){depth+=1;}
            let name=l.split([';',':']).next().unwrap_or("");
            if depth==1&&matches!(name,"SUMMARY"|"DTSTART"|"DTEND"|"DURATION"|"DTSTAMP"|"LAST-MODIFIED"){continue;}
            if depth==1&&name=="SEQUENCE"{let n=l.split_once(':').and_then(|(_,s)|s.parse::<u64>().ok()).unwrap_or(0);out.push_str(&format!("SEQUENCE:{}\r\n",n.saturating_add(1)));continue;}
            if l.starts_with("END:")&&depth>0{depth-=1;}
            out.push_str(l);out.push_str("\r\n");
        }Ok(out)
    }else{Ok(format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//KarMa//Daozai//ZH\r\nBEGIN:VEVENT\r\nUID:{}\r\n{fields}END:VEVENT\r\nEND:VCALENDAR\r\n",escape(&event.uid)))}
}
pub fn save(cfg:&Config,password:&str,event:&crate::mail::Meeting,old:Option<&Item>,delete:bool)->Result<(),Failure>{
    if old.is_some_and(|i|i.readonly){return Err(Failure::text("此日程只可展示，暂不支持编辑或删除"));}
    let a=agent();let href=if let Some(i)=old{target(cfg,&i.href)?}else{
        if delete{return Err(Failure::text("日程不存在"));}
        let collection=cfg.href.clone().map(Ok).unwrap_or_else(||discover(&a,cfg,password))?;
        target(cfg,&join_url(&collection,&format!("karma-{}-{}.ics",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos())))?
    };
    let mut req=a.request(if delete{"DELETE"}else{"PUT"},&href).set("Authorization",&basic_auth(&cfg.username,password));
    req=if let Some(i)=old{if i.etag.is_empty()||i.etag.starts_with("W/"){return Err(Failure::text("服务器未提供可用于冲突检查的版本，请刷新"));}req.set("If-Match",&i.etag)}else{req.set("If-None-Match","*")};
    let result=if delete{req.call()}else{req.set("Content-Type","text/calendar; charset=utf-8").send_string(&content(event,old.map(|i|i.raw.as_str()))?)};
    match result{
        Ok(r)if (200..300).contains(&r.status())=>Ok(()),
        Ok(r)=>Err(Failure::status(cfg.provider,r.status())),
        Err(ureq::Error::Status(409|412,_))=>Err(Failure::text("日程已被其他设备修改或删除，请刷新后重新编辑")),
        Err(ureq::Error::Status(code,_))=>Err(Failure::status(cfg.provider,code)),
        Err(_)=>Err(Failure::text("写入结果未确认，请刷新日历核对，避免重复创建")),
    }
}

#[cfg(test)]
mod tests{
 use super::*;
 fn event()->crate::mail::Meeting{crate::mail::Meeting{uid:"test".into(),title:"标题,一;二\n三".into(),start:1793844000,end:Some(1793847600),all_day:false,at:String::new()}}
 #[test]fn update_preserves_unedited_fields_and_nested_alarm(){
  let raw="BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:test\r\nSUMMARY:old\r\nDTSTART:20261105T100000Z\r\nDURATION:PT1H\r\nLOCATION:保留地点\r\nDESCRIPTION:保留说明\r\nSEQUENCE:4\r\nBEGIN:VALARM\r\nDTSTART:20261105T090000Z\r\nDESCRIPTION:提醒\r\nEND:VALARM\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
  let out=content(&event(),Some(raw)).unwrap();
  assert!(out.contains("LOCATION:保留地点\r\nDESCRIPTION:保留说明"));assert!(out.contains("SEQUENCE:5"));assert!(out.contains("BEGIN:VALARM\r\nDTSTART:20261105T090000Z"));assert!(!out.contains("DURATION:"));assert!(out.contains("SUMMARY:标题\\,一\\;二\\n三"));
  assert!(content(&event(),Some(&raw.replace("SEQUENCE:4","RRULE:FREQ=DAILY"))).is_err());
 }
 #[test]fn write_sends_precondition_and_rejects_conflict_without_redirect(){
  use std::{io::{Read,Write},net::TcpListener};
  let listener=TcpListener::bind("127.0.0.1:0").unwrap();let base=format!("http://{}/",listener.local_addr().unwrap());
  let cfg=Config::from_input(Provider::Custom,"test",&base).unwrap();
  let item=Item{config:cfg.clone(),event:event(),href:format!("{base}test.ics"),etag:"\"v1\"".into(),raw:content(&event(),None).unwrap(),readonly:false};
  let thread=std::thread::spawn(move||{let(mut socket,_)=listener.accept().unwrap();socket.set_read_timeout(Some(Duration::from_secs(3))).unwrap();let mut b=[0;8192];let n=socket.read(&mut b).unwrap();let request=String::from_utf8_lossy(&b[..n]).to_ascii_lowercase();assert!(request.starts_with("delete /test.ics"));assert!(request.contains("if-match: \"v1\""));socket.write_all(b"HTTP/1.1 412 Precondition Failed\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();});
  assert!(save(&cfg,"test-password",&item.event,Some(&item),true).unwrap_err().message.contains("其他设备"));thread.join().unwrap();
  let mut foreign=item;foreign.href="https://other.invalid/stolen".into();assert!(save(&cfg,"test-password",&foreign.event,Some(&foreign),true).is_err());
 }
}
