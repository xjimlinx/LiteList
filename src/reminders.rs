//! Local calendar input -> UTC instants. No network or background service.
use serde::{Serialize,Deserialize};
use windows_sys::Win32::{Foundation::{SYSTEMTIME,FILETIME},System::{SystemInformation::GetLocalTime,Time::{SystemTimeToFileTime,FileTimeToSystemTime,TzSpecificLocalTimeToSystemTime,SystemTimeToTzSpecificLocalTime}}};

#[derive(Clone,Debug,Serialize,Deserialize,PartialEq)]
pub struct Reminder {
    pub enabled:bool,
    pub due_at:u64,
    pub early_at:Option<u64>,
    #[serde(default)] pub early_note:String,
    #[serde(default)] pub due_sent:bool,
    #[serde(default)] pub early_sent:bool,
}
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct Notification { pub task_id:u64,pub title:String,pub text:String,pub scheduled_at:u64,pub acknowledged:bool }

fn before(s:&str)->Option<u16>{let chars:String=s.chars().rev().take_while(|c|c.is_ascii_digit()).collect();chars.chars().rev().collect::<String>().parse().ok()}
fn after(s:&str)->Option<u16>{s.chars().take_while(|c|c.is_ascii_digit()).collect::<String>().parse().ok()}
pub fn current_year()->u16{unsafe{let mut t=SYSTEMTIME::default();GetLocalTime(&mut t);t.wYear}}
pub fn infer(text:&str,year:u16)->Option<String>{
    let normalized=text.replace('：',":");let s=normalized.as_str();
    let colon=s.find(':')?;let hour=before(&s[..colon])?;let minute=after(&s[colon+1..])?;
    let (year,month,day)=if let (Some(m),Some(d))=(s.find('月'),s.find('日')){
        let y=s.find('年').and_then(|p|before(&s[..p])).unwrap_or(year);
        if d<=m{return None;}(y,before(&s[..m])?,before(&s[..d])?)
    }else{
        let part=s[..colon].split_whitespace().next()?;let nums:Vec<u16>=part.split('-').map(str::parse).collect::<Result<_,_>>().ok()?;
        match nums.as_slice(){[y,m,d]=>(*y,*m,*d),[m,d]=>(year,*m,*d),_=>return None}
    };
    if !valid_date(year,month,day,hour,minute){return None;}
    Some(format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}"))
}
fn valid_date(y:u16,m:u16,d:u16,h:u16,min:u16)->bool{
    if !(1970..=9999).contains(&y)||!(1..=12).contains(&m)||h>23||min>59{return false;}
    let leap=y%4==0 && (y%100!=0||y%400==0);
    let max=[31,if leap{29}else{28},31,30,31,30,31,31,30,31,30,31][m as usize-1];d>0&&d<=max
}
pub fn parse_local(input:&str)->Result<u64,String>{
    let canonical=infer(input.trim(),current_year()).ok_or("请输入有效日期，例如 2026-09-09 19:00（也支持 9月9日 19:00）")?;
    let nums:Vec<u16>=canonical.split(['-',' ',':']).map(|s|s.parse().unwrap()).collect();
    let local=SYSTEMTIME{wYear:nums[0],wMonth:nums[1],wDay:nums[2],wHour:nums[3],wMinute:nums[4],..Default::default()};
    unsafe{
        let mut utc=SYSTEMTIME::default();let mut ft=FILETIME::default();
        if TzSpecificLocalTimeToSystemTime(std::ptr::null(),&local,&mut utc)==0||SystemTimeToFileTime(&utc,&mut ft)==0{return Err("无法转换此日期时间".into());}
        let ticks=((ft.dwHighDateTime as u64)<<32)|ft.dwLowDateTime as u64;
        let ms=ticks.checked_sub(116444736000000000).ok_or("日期超出范围")?/10000;
        if format_local(ms)!=canonical{return Err("该本地时间不存在（可能遇到夏令时切换），请换一个时间".into());}Ok(ms)
    }
}
pub fn format_local(ms:u64)->String{unsafe{
    let Some(ticks)=ms.checked_mul(10000).and_then(|v|v.checked_add(116444736000000000))else{return String::new();};
    let ft=FILETIME{dwLowDateTime:ticks as u32,dwHighDateTime:(ticks>>32)as u32};let mut utc=SYSTEMTIME::default();let mut local=SYSTEMTIME::default();
    if FileTimeToSystemTime(&ft,&mut utc)==0||SystemTimeToTzSpecificLocalTime(std::ptr::null(),&utc,&mut local)==0{return String::new();}
    format!("{:04}-{:02}-{:02} {:02}:{:02}",local.wYear,local.wMonth,local.wDay,local.wHour,local.wMinute)
}}
pub fn local_time(ms:u64)->Option<SYSTEMTIME>{unsafe{
    let ticks=ms.checked_mul(10000)?.checked_add(116444736000000000)?;
    let ft=FILETIME{dwLowDateTime:ticks as u32,dwHighDateTime:(ticks>>32)as u32};let mut utc=SYSTEMTIME::default();let mut local=SYSTEMTIME::default();
    if FileTimeToSystemTime(&ft,&mut utc)==0||SystemTimeToTzSpecificLocalTime(std::ptr::null(),&utc,&mut local)==0{None}else{Some(local)}
}}
pub fn configured_time(enabled:bool,due_local:SYSTEMTIME,early_local:Option<SYSTEMTIME>,note:&str,old:Option<&Reminder>)->Result<Reminder,String>{
    fn convert(local:SYSTEMTIME)->Result<u64,String>{unsafe{let mut utc=SYSTEMTIME::default();let mut ft=FILETIME::default();if TzSpecificLocalTimeToSystemTime(std::ptr::null(),&local,&mut utc)==0||SystemTimeToFileTime(&utc,&mut ft)==0{return Err("无法转换所选日期时间".into());}let ticks=((ft.dwHighDateTime as u64)<<32)|ft.dwLowDateTime as u64;ticks.checked_sub(116444736000000000).map(|t|t/10000).ok_or("日期超出范围".into())}}
    let due_at=convert(due_local)?;let early_at=early_local.map(convert).transpose()?;
    if early_at.is_some_and(|t|t>=due_at){return Err("提前提醒时间必须早于到点时间".into());}
    Ok(Reminder{enabled,due_at,early_at,early_note:note.chars().take(1000).collect(),due_sent:old.is_some_and(|r|r.due_at==due_at&&r.due_sent),early_sent:old.is_some_and(|r|r.early_at==early_at&&r.early_sent)})
}
pub fn configured(enabled:bool,due:&str,early:&str,note:&str,old:Option<&Reminder>)->Result<Option<Reminder>,String>{
    if due.trim().is_empty(){if enabled{return Err("开启提醒前，请先填写到点时间".into());}return Ok(None);}
    let due_at=parse_local(due)?;let early_at=if early.trim().is_empty(){None}else{Some(parse_local(early)?)};
    if early_at.is_some_and(|t|t>=due_at){return Err("提前提醒时间必须早于到点时间".into());}
    Ok(Some(Reminder{enabled,due_at,early_at,early_note:note.chars().take(1000).collect(),
        due_sent:old.is_some_and(|r|r.due_at==due_at&&r.due_sent),
        early_sent:old.is_some_and(|r|r.early_at==early_at&&r.early_sent)}))
}

#[cfg(test)]mod tests{
    use super::*;
    #[test]fn parses_user_example(){assert_eq!(infer("9月9日 19:00 vivo 宣讲会",2026).unwrap(),"2026-09-09 19:00");}
    #[test]fn rejects_invalid_dates(){assert!(infer("2月30日 19:00",2026).is_none());assert!(infer("9月9日 25:00",2026).is_none());assert!(infer("没有日期 19:00",2026).is_none());}
    #[test]fn local_round_trip(){let t=parse_local("2026-09-09 19:00").unwrap();assert_eq!(format_local(t),"2026-09-09 19:00");}
    #[test]fn earlier_must_be_earlier(){assert!(configured(true,"2026-09-09 19:00","2026-09-09 20:00","",None).is_err());}
    #[test]fn preserves_delivery_on_unrelated_edit(){let mut old=configured(true,"2026-09-09 19:00","","",None).unwrap().unwrap();old.due_sent=true;assert!(configured(true,&format_local(old.due_at),"","",Some(&old)).unwrap().unwrap().due_sent);}
}
