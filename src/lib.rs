use std::{
    borrow::Cow,
    sync::{Arc, Mutex},
};

#[macro_use]
extern crate lazy_static;
use regex::Regex;

pub mod bindings {
    windows::include_bindings!();
}

use bindings::Windows::Win32::{
    Foundation::PWSTR,
    System::Com::CoInitialize,
    UI::Shell::PathCchCanonicalizeEx,
    Storage::FileSystem::{MoveFileExW,MOVE_FILE_FLAGS},
};

/*
 * For Initializing win32
 *
 */

lazy_static! {
    static ref INIT: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
    static ref WIN_ESCAPED_CHAR: Regex = Regex::new(r#"\u{005E}(.)"#).unwrap();
    static ref ROOTED_MING_W64_COMPAT: Regex = Regex::new(r#"^/([a-zA-Z])/(.*)$"#).unwrap();
    static ref ROOTED_TILDE_COMPAT: Regex = Regex::new(r#"^(~)(.*)$"#).unwrap();
    static ref NORMALIZE_SLASH: Regex = Regex::new(r#"([\u{005C}\u{002F}]{1,})"#).unwrap();
}

fn co_initialize() -> Result<(), Box<dyn std::error::Error>> {
    let mut flag = INIT.lock()?;
    if !*flag {
        unsafe { CoInitialize(std::ptr::null_mut())? };
        *flag = true;
    }
    Ok(())
}

/*
 * Boilerplate so I don't need to think about
 * types or borrowing
 *
 */
trait ToCow<'a> {
    fn to_cow(self) -> Cow<'a, str>;
}
impl<'a> ToCow<'a> for &'a str {
    fn to_cow(self) -> Cow<'a, str> {
        Cow::Borrowed(self)
    }
}
impl<'a> ToCow<'a> for String {
    fn to_cow(self) -> Cow<'a, str> {
        Cow::Owned(self)
    }
}
impl<'a> ToCow<'a> for &'a String {
    fn to_cow(self) -> Cow<'a, str> {
        Cow::Borrowed(self.as_str())
    }
}
impl<'a> ToCow<'a> for Cow<'a, str> {
    fn to_cow(self) -> Cow<'a, str> {
        self
    }
}
impl<'a> ToCow<'a> for &'a Cow<'_, str> {
    fn to_cow(self) -> Cow<'a, str> {
        Cow::Borrowed(self.as_ref())
    }
}

fn win_escape_char<'a,T>(arg: T) -> Result<Cow<'a,str>,Box<dyn std::error::Error>>
where
    T: ToCow<'a>,
{
    let cow = <T as ToCow>::to_cow(arg);
    if WIN_ESCAPED_CHAR.is_match(cow.as_ref()) {
        Ok(WIN_ESCAPED_CHAR.replace_all(cow.as_ref(), "$1")
            .to_string()
            .to_cow())
    } else {
        Ok(cow)
    }
}

#[test]
fn test_win_escape_char() {
    assert_eq!(
        win_escape_char(r#"F:\^^Users^\Valarauca"#).unwrap(),
        r#"F:\^Users\Valarauca"#
    );
}

fn fix_root<'a, T>(arg: T) -> Result<Cow<'a, str>, Box<dyn std::error::Error>>
where
    T: ToCow<'a>,
{
    let cow = <T as ToCow>::to_cow(arg);
    match ROOTED_MING_W64_COMPAT.captures(&cow) {
        Option::None => Ok(cow),
        Option::Some(caps) => {
            let drive_letter = caps.get(1).unwrap().as_str().to_uppercase();
            let rest = caps.get(2).unwrap().as_str();
            Ok(Cow::Owned(format!(r#"{}:\{}"#, drive_letter, rest)))
        }
    }
}

#[test]
fn test_fix_root() {
    // sanity check
    assert_eq!(
        fix_root(r#"F:\Users\Valarauca"#).unwrap(),
        r#"F:\Users\Valarauca"#
    );
    assert_eq!(
        fix_root(r#"/f/Users/Valarauca"#).unwrap(),
        r#"F:\Users/Valarauca"#
    );

    // terminating slash
    assert_eq!(
        fix_root(r#"F:\Users\Valarauca\"#).unwrap(),
        r#"F:\Users\Valarauca\"#
    );
    assert_eq!(
        fix_root(r#"/f/Users/Valarauca\"#).unwrap(),
        r#"F:\Users/Valarauca\"#
    );

    // opposite slash
    assert_eq!(
        fix_root(r#"F:\Users\Valarauca/"#).unwrap(),
        r#"F:\Users\Valarauca/"#
    );
    assert_eq!(
        fix_root(r#"/f/Users/Valarauca/"#).unwrap(),
        r#"F:\Users/Valarauca/"#
    );

    // double terminating slash
    assert_eq!(
        fix_root(r#"F:\Users\Valarauca\\"#).unwrap(),
        r#"F:\Users\Valarauca\\"#
    );
    assert_eq!(
        fix_root(r#"/f/Users/Valarauca\\"#).unwrap(),
        r#"F:\Users/Valarauca\\"#
    );

    // double opposite slash
    assert_eq!(
        fix_root(r#"F:\Users\Valarauca//"#).unwrap(),
        r#"F:\Users\Valarauca//"#
    );
    assert_eq!(
        fix_root(r#"/f/Users/Valarauca//"#).unwrap(),
        r#"F:\Users/Valarauca//"#
    );
}

fn fix_tilde<'a, T>(arg: T) -> Result<Cow<'a, str>, Box<dyn std::error::Error>>
where
    T: ToCow<'a>,
{
    let cow = <T as ToCow>::to_cow(arg);
    if ROOTED_TILDE_COMPAT.is_match(cow.as_ref()) {
        let home = std::env::var("HOME")?;
        Ok(ROOTED_TILDE_COMPAT
            .replace_all(&cow, format!("{}$2", home))
            .to_string()
            .to_cow())
    } else {
        Ok(cow)
    }
}

#[test]
fn test_fix_tilde() {
    // test cases which should be uneffected
    assert_eq!(
        fix_tilde(r#"C:\Users\Valarauca\Documents\"#).unwrap(),
        r#"C:\Users\Valarauca\Documents\"#
    );
    assert_eq!(
        fix_tilde(r#"C:\Users\\\Valarauca\Documents\"#).unwrap(),
        r#"C:\Users\\\Valarauca\Documents\"#
    );
    assert_eq!(
        fix_tilde(r#"~/Documents/"#).unwrap(),
        r#"C:\Users\valarauca/Documents/"#
    );

    // trivial cases
    assert_eq!(fix_tilde(r#"~/"#).unwrap(), r#"C:\Users\valarauca/"#);
    assert_eq!(fix_tilde(r#"~\"#).unwrap(), r#"C:\Users\valarauca\"#);
    assert_eq!(fix_tilde(r#"~///"#).unwrap(), r#"C:\Users\valarauca///"#);
    assert_eq!(
        fix_tilde(r#"~\\\lol\"#).unwrap(),
        r#"C:\Users\valarauca\\\lol\"#
    );
}

fn normalize_slash<'a, T>(arg: T) -> Result<Cow<'a, str>, Box<dyn std::error::Error>>
where
    T: ToCow<'a>,
{
    let cow = <T as ToCow>::to_cow(arg);
    if NORMALIZE_SLASH.is_match(cow.as_ref()) {
        Ok(NORMALIZE_SLASH
            .replace_all(&cow, r#"\"#)
            .to_string()
            .to_cow())
    } else {
        Ok(cow)
    }
}

#[test]
fn test_normalize_slash() {
    // test cases which should be uneffected
    assert_eq!(
        normalize_slash(r#"C:\Users\Valarauca\Documents\"#).unwrap(),
        r#"C:\Users\Valarauca\Documents\"#
    );

    // simple test cases
    assert_eq!(
        normalize_slash(r#"C:/Users/Valarauca/Documents/"#).unwrap(),
        r#"C:\Users\Valarauca\Documents\"#
    );
    assert_eq!(
        normalize_slash(r#"C:\Users\\\\Valarauca\\Documents\\/\"#).unwrap(),
        r#"C:\Users\Valarauca\Documents\"#
    );
    assert_eq!(
        normalize_slash(r#"C:\Users/Valarauca\/\Documents\\/\"#).unwrap(),
        r#"C:\Users\Valarauca\Documents\"#
    );
}

fn path_cch_canonicalize_ex<'a, T>(arg: T) -> Result<Cow<'a, str>, Box<dyn std::error::Error>>
where
    T: ToCow<'a>,
{
    co_initialize()?;

    let cow = <T as ToCow>::to_cow(arg);

    // 32KiB is totally unreasonable for a path length
    #[allow(non_snake_case)]
    let KiB32 = 32768usize;
    let mut v = Vec::<u16>::with_capacity(KiB32);
    for _ in 0..KiB32 {
        v.push(0u16);
    }

    unsafe { PathCchCanonicalizeEx(PWSTR(v.as_mut_ptr()), KiB32, cow.as_ref(), 1)? };

    let mut length = 0usize;
    for index in 0..KiB32 {
        if v[index] == 0 {
            break;
        }
        length += 1;
    }
    Ok(String::from_utf16(&v.as_slice()[0..length])?.to_cow())
}

#[test]
fn test_path_cch_canonicalize_ex() {
    assert_eq!(
        path_cch_canonicalize_ex(r#"C:\Users\Valarauca\Documents\"#).unwrap(),
        r#"C:\Users\Valarauca\Documents\"#
    );
    assert_eq!(
        path_cch_canonicalize_ex(r#"C:\Users\Valarauca\Documents\..\..\"#).unwrap(),
        r#"C:\Users\"#
    );
}

/// This canonicalizes a path, if the path in question exists or not
///
/// Will handle some -oddities- of cygwin, mingw, and windows shell
pub fn canonicalize(path: &str) -> Result<String, Box<dyn std::error::Error>> {
    let a = fix_root(path)?;
    let b = fix_tilde(a)?;
    let c = normalize_slash(b)?;
    let d = path_cch_canonicalize_ex(c)?;
    Ok(d.to_string())
}

#[test]
fn assert_matches() {
    assert_eq!(
        canonicalize("~/Documents/").unwrap(),
        r#"C:\Users\valarauca\Documents\"#
    );
    assert_eq!(canonicalize("/f/Downloads/").unwrap(), r#"F:\Downloads\"#);
    assert_eq!(canonicalize("/f/Downloads/../").unwrap(), r#"F:\"#);
}

/// moves file
fn priv_move_file<'a,A,B>(
    src: A,
    dst: B,
    overwrite_okay: bool) -> Result<(),Box<dyn std::error::Error>>
where
    A: ToCow<'a>,
    B: ToCow<'a>,
{
    co_initialize()?;

    let src_value = <A as ToCow>::to_cow(src);
    let dst_value = <B as ToCow>::to_cow(dst);

    // see: https://docs.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-movefileexa
    let mut flags = 0u32;
    if overwrite_okay {
        flags += 1u32;
    }
    // ensure copy occurs before flushing
    flags += 0u32;
    // allow for copy + delete when needed
    flags += 2u32;
    unsafe {
        MoveFileExW(
            src_value.as_ref(),
            dst_value.as_ref(),
            MOVE_FILE_FLAGS(flags)).ok()?;
    }
    Ok(())
}


pub fn move_file(src: &str, dst: &str, overwrite: bool) -> Result<(),Box<dyn std::error::Error>> {
    priv_move_file(src, dst, overwrite)
}
