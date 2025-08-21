use std::ptr;
use std::mem;
use winapi::um::winuser::*;
use winapi::shared::minwindef::{HKL, LPARAM};

#[derive(Debug, Clone)]
pub struct LayoutInfo {
    pub hkl: usize, // Changed from HKL to usize for Send + Sync
    #[allow(dead_code)]
    pub lang_id: u32,
    pub name: String,
    pub short_code: String,
    pub is_english: bool,
}

impl LayoutInfo {
    pub fn new(hkl: HKL) -> Self {
        let lang_id = (hkl as usize) & 0xFFFF;
        let (name, short_code, is_english) = get_layout_details(lang_id as u32);
        
        LayoutInfo {
            hkl: hkl as usize, // Convert HKL to usize
            lang_id: lang_id as u32,
            name,
            short_code,
            is_english,
        }
    }
    
    pub fn get_hkl(&self) -> HKL {
        self.hkl as HKL // Convert back to HKL when needed
    }
}

pub fn get_all_keyboard_layouts() -> Vec<LayoutInfo> {
    unsafe {
        let mut layouts: [HKL; 20] = mem::zeroed();
        let layout_count = GetKeyboardLayoutList(20, layouts.as_mut_ptr());
        
        let mut layout_infos = Vec::new();
        for i in 0..layout_count as usize {
            layout_infos.push(LayoutInfo::new(layouts[i]));
        }
        
        // Sort layouts: English first, then alphabetically
        layout_infos.sort_by(|a, b| {
            match (a.is_english, b.is_english) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.cmp(&b.name),
            }
        });
        
        layout_infos
    }
}

pub fn get_current_layout() -> Option<LayoutInfo> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return None;
        }
        
        let thread_id = GetWindowThreadProcessId(hwnd, ptr::null_mut());
        let current_layout = GetKeyboardLayout(thread_id);
        
        Some(LayoutInfo::new(current_layout))
    }
}

pub fn find_layouts_by_codes(codes: &[&str]) -> Vec<LayoutInfo> {
    let all_layouts = get_all_keyboard_layouts();
    let mut selected_layouts = Vec::new();
    
    for code in codes {
        if let Some(layout) = all_layouts.iter().find(|l| l.short_code == *code) {
            selected_layouts.push(layout.clone());
        }
    }
    
    selected_layouts
}

pub fn get_english_layout() -> Option<LayoutInfo> {
    let all_layouts = get_all_keyboard_layouts();
    all_layouts.into_iter().find(|l| l.is_english)
}

pub fn switch_to_layout(layout: &LayoutInfo) {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return;
        }
        
        let hkl = layout.get_hkl();
        
        // Activate new layout
        ActivateKeyboardLayout(hkl, 0);
        
        // Send layout change message to all windows
        PostMessageW(
            HWND_BROADCAST,
            WM_INPUTLANGCHANGEREQUEST,
            0,
            hkl as LPARAM,
        );
    }
}

fn get_layout_details(lang_id: u32) -> (String, String, bool) {
    match lang_id {
        // English variants
        0x0409 => ("English (United States)".to_string(), "us".to_string(), true),
        0x0809 => ("English (United Kingdom)".to_string(), "gb".to_string(), true),
        0x0c09 => ("English (Australia)".to_string(), "au".to_string(), true),
        0x1009 => ("English (Canada)".to_string(), "ca".to_string(), true),
        0x1409 => ("English (New Zealand)".to_string(), "nz".to_string(), true),
        0x1809 => ("English (Ireland)".to_string(), "ie".to_string(), true),
        0x1c09 => ("English (South Africa)".to_string(), "za".to_string(), true),
        
        // Cyrillic languages
        0x0419 => ("Russian".to_string(), "ru".to_string(), false),
        0x0422 => ("Ukrainian".to_string(), "ua".to_string(), false),
        0x0423 => ("Belarusian".to_string(), "by".to_string(), false),
        0x0402 => ("Bulgarian".to_string(), "bg".to_string(), false),
        0x041a => ("Croatian".to_string(), "hr".to_string(), false),
        0x0405 => ("Czech".to_string(), "cz".to_string(), false),
        0x081a => ("Serbian (Latin)".to_string(), "rs".to_string(), false),
        0x0c1a => ("Serbian (Cyrillic)".to_string(), "sr".to_string(), false),
        0x041f => ("Turkish".to_string(), "tr".to_string(), false),
        
        // Western European
        0x0407 => ("German".to_string(), "de".to_string(), false),
        0x040c => ("French".to_string(), "fr".to_string(), false),
        0x0410 => ("Italian".to_string(), "it".to_string(), false),
        0x040a => ("Spanish".to_string(), "es".to_string(), false),
        0x0413 => ("Dutch".to_string(), "nl".to_string(), false),
        0x0414 => ("Norwegian".to_string(), "no".to_string(), false),
        0x041d => ("Swedish".to_string(), "se".to_string(), false),
        0x0406 => ("Danish".to_string(), "dk".to_string(), false),
        0x040b => ("Finnish".to_string(), "fi".to_string(), false),
        0x0816 => ("Portuguese".to_string(), "pt".to_string(), false),
        0x0416 => ("Portuguese (Brazil)".to_string(), "br".to_string(), false),
        
        // Eastern European
        0x0415 => ("Polish".to_string(), "pl".to_string(), false),
        0x040e => ("Hungarian".to_string(), "hu".to_string(), false),
        0x0418 => ("Romanian".to_string(), "ro".to_string(), false),
        0x041b => ("Slovak".to_string(), "sk".to_string(), false),
        0x0424 => ("Slovenian".to_string(), "si".to_string(), false),
        0x0425 => ("Estonian".to_string(), "ee".to_string(), false),
        0x0426 => ("Latvian".to_string(), "lv".to_string(), false),
        0x0427 => ("Lithuanian".to_string(), "lt".to_string(), false),
        
        // Asian languages
        0x0411 => ("Japanese".to_string(), "jp".to_string(), false),
        0x0412 => ("Korean".to_string(), "kr".to_string(), false),
        0x0404 => ("Chinese (Traditional)".to_string(), "tw".to_string(), false),
        0x0804 => ("Chinese (Simplified)".to_string(), "cn".to_string(), false),
        0x041e => ("Thai".to_string(), "th".to_string(), false),
        0x042a => ("Vietnamese".to_string(), "vn".to_string(), false),
        
        // Middle Eastern
        0x040d => ("Hebrew".to_string(), "he".to_string(), false),
        0x0401 => ("Arabic".to_string(), "ar".to_string(), false),
        0x0429 => ("Farsi".to_string(), "fa".to_string(), false),
        
        // Other
        0x040f => ("Icelandic".to_string(), "is".to_string(), false),
        0x0408 => ("Greek".to_string(), "gr".to_string(), false),
        0x041c => ("Albanian".to_string(), "al".to_string(), false),
        0x042f => ("Macedonian".to_string(), "mk".to_string(), false),
        
        // Default case
        _ => {
            // Try to determine if it's English based on primary language
            let primary_lang = lang_id & 0x3FF;
            let is_english = primary_lang == 0x09; // LANG_ENGLISH
            let name = format!("Unknown Language (0x{:04X})", lang_id);
            let code = format!("{:02x}", (lang_id & 0xFF) as u8);
            (name, code, is_english)
        }
    }
}

pub fn validate_country_codes(codes: &[&str]) -> Result<Vec<String>, String> {
    let all_layouts = get_all_keyboard_layouts();
    let mut valid_codes = Vec::new();
    let mut invalid_codes = Vec::new();
    
    for code in codes {
        if all_layouts.iter().any(|l| l.short_code == *code) {
            valid_codes.push(code.to_string());
        } else {
            invalid_codes.push(code.to_string());
        }
    }
    
    if !invalid_codes.is_empty() {
        return Err(format!(
            "Unknown country codes: {}. Use 'ccaps -status' to see available codes.",
            invalid_codes.join(", ")
        ));
    }
    
    if valid_codes.is_empty() {
        return Err("No valid country codes provided.".to_string());
    }
    
    Ok(valid_codes)
}